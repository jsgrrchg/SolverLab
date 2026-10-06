//! Batched parallel simulation runtime, ported from
//! `SimulationViewModel+Runtime.swift` and the auto-parallel heuristics in
//! `SimulationViewModel.swift`.
//!
//! A runner thread plays games in batches of `workers` threads, streams each
//! finished game over a channel and, in auto mode, resizes the next batch
//! from the timeout and win rates of the previous one.

use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use crate::deck::ShuffleSource;
use crate::games::{GameContext, run_single_game};
use crate::model::{BatchStats, SimConfig, SimResult, StopReason};

/// Name prefix of the threads that run a single game (see the panic hook in `main`).
pub const GAME_THREAD_PREFIX: &str = "solverlab-game";

/// Hands out game ids `1..=total`.
pub struct Coordinator {
    next: AtomicU64,
    total: u64,
}

impl Coordinator {
    pub fn new(total: u32) -> Self {
        Coordinator {
            next: AtomicU64::new(1),
            total: u64::from(total),
        }
    }

    pub fn take_next_game_id(&self) -> Option<u32> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        if id <= self.total {
            u32::try_from(id).ok()
        } else {
            None
        }
    }
}

/// Start times of the games currently being solved; drives the active-timeout countdown.
#[derive(Clone, Default)]
pub struct ActiveGamesClock(Arc<Mutex<HashMap<u32, Instant>>>);

impl ActiveGamesClock {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Instant>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn begin(&self, game_id: u32) {
        self.lock().insert(game_id, Instant::now());
    }

    pub fn end(&self, game_id: u32) {
        self.lock().remove(&game_id);
    }

    pub fn clear(&self) {
        self.lock().clear();
    }

    pub fn oldest_start(&self) -> Option<Instant> {
        self.lock().values().min().copied()
    }
}

#[derive(Debug)]
pub enum RunEvent {
    WorkersChanged { run_id: u64, workers: usize },
    GameFinished { run_id: u64, result: SimResult },
    Finished { run_id: u64 },
}

pub struct RunHandle {
    pub run_id: u64,
    cancel: Arc<AtomicBool>,
}

impl RunHandle {
    /// Stops handing out new games. In-flight solves cannot be interrupted;
    /// their results are dropped.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Handle for a run that has no runner thread (tests).
    #[cfg(test)]
    pub(crate) fn idle(run_id: u64) -> Self {
        RunHandle {
            run_id,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub fn cpu_count() -> usize {
    thread::available_parallelism().map_or(1, |n| n.get())
}

fn clamp_total(total_games: i64) -> usize {
    usize::try_from(total_games.max(0)).unwrap_or(usize::MAX)
}

/// Half the CPUs, capped by the number of games (0 games → 0 workers).
pub fn auto_parallel_initial(total_games: i64, cpu: usize) -> usize {
    clamp_total(total_games).min((cpu / 2).max(1))
}

pub fn auto_parallel_next(
    current: usize,
    completed: u64,
    total_games: i64,
    stats: BatchStats,
    cpu: usize,
) -> usize {
    let total = clamp_total(total_games);
    let hard_cap = total.min(cpu.max(1));
    if stats.completed == 0 {
        return current.min(hard_cap).max(1);
    }

    let timeout_rate = stats.timeouts as f64 / stats.completed as f64;
    let win_rate = stats.wins as f64 / stats.completed as f64;

    let mut next = current;
    if timeout_rate > 0.55 {
        next = current.saturating_sub(1).max(1);
    } else if timeout_rate < 0.25 && win_rate >= 0.25 {
        next = hard_cap.min(current + 1);
    }

    let completed = usize::try_from(completed).unwrap_or(usize::MAX);
    let remaining = total.saturating_sub(completed).max(1);
    next.min(remaining).min(hard_cap).max(1)
}

fn initial_workers(config: &SimConfig, cpu: usize) -> usize {
    if config.auto_parallel {
        auto_parallel_initial(config.simulations, cpu)
    } else {
        let parallel = clamp_total(config.parallel_games);
        parallel
            .min(clamp_total(config.simulations))
            .min(cpu)
            .max(1)
    }
}

fn panicked_result(game_id: u32) -> SimResult {
    SimResult {
        id: game_id,
        move_count: 0,
        undo_count: 0,
        checkpoint_count: None,
        won: false,
        stop_reason: StopReason::Stalled,
        duration: 0.0,
        score: 0,
        moves_detail: "panic".into(),
    }
}

fn play_game(
    game_id: u32,
    config: &SimConfig,
    shuffle: ShuffleSource,
    clock: &ActiveGamesClock,
) -> SimResult {
    clock.begin(game_id);
    let ctx = GameContext {
        game_id,
        config,
        shuffle,
    };
    let result = panic::catch_unwind(AssertUnwindSafe(|| run_single_game(&ctx)))
        .unwrap_or_else(|_| panicked_result(game_id));
    clock.end(game_id);
    result
}

/// Port of `runSimulation`: runs on its own thread and reports through `tx`.
pub fn spawn_simulation(
    config: SimConfig,
    shuffle: ShuffleSource,
    run_id: u64,
    clock: ActiveGamesClock,
    tx: Sender<RunEvent>,
) -> RunHandle {
    let cancel = Arc::new(AtomicBool::new(false));
    let handle = RunHandle {
        run_id,
        cancel: Arc::clone(&cancel),
    };
    thread::Builder::new()
        .name("solverlab-runner".into())
        .spawn(move || run_simulation(&config, shuffle, run_id, &clock, &tx, &cancel))
        .expect("failed to spawn the simulation runner thread");
    handle
}

fn run_simulation(
    config: &SimConfig,
    shuffle: ShuffleSource,
    run_id: u64,
    clock: &ActiveGamesClock,
    tx: &Sender<RunEvent>,
    cancel: &AtomicBool,
) {
    let cpu = cpu_count();
    let total = u32::try_from(config.simulations.max(0)).unwrap_or(u32::MAX);
    let coordinator = Coordinator::new(total);
    let mut workers = initial_workers(config, cpu);
    let mut completed: u64 = 0;
    let _ = tx.send(RunEvent::WorkersChanged { run_id, workers });

    while !cancel.load(Ordering::Relaxed) {
        let batch: Vec<SimResult> = thread::scope(|scope| {
            let handles: Vec<_> = (0..workers)
                .map(|worker| {
                    let coordinator = &coordinator;
                    thread::Builder::new()
                        .name(format!("{GAME_THREAD_PREFIX}-{worker}"))
                        .spawn_scoped(scope, move || {
                            if cancel.load(Ordering::Relaxed) {
                                return None;
                            }
                            let game_id = coordinator.take_next_game_id()?;
                            let result = play_game(game_id, config, shuffle, clock);
                            if !cancel.load(Ordering::Relaxed) {
                                let _ = tx.send(RunEvent::GameFinished {
                                    run_id,
                                    result: result.clone(),
                                });
                            }
                            Some(result)
                        })
                        .expect("failed to spawn a game thread")
                })
                .collect();
            handles
                .into_iter()
                .filter_map(|handle| handle.join().ok().flatten())
                .collect()
        });

        if batch.is_empty() {
            break;
        }
        completed += batch.len() as u64;

        if config.auto_parallel {
            let stats = BatchStats {
                completed: batch.len(),
                wins: batch.iter().filter(|r| r.won).count(),
                timeouts: batch
                    .iter()
                    .filter(|r| r.stop_reason == StopReason::Timeout)
                    .count(),
            };
            workers = auto_parallel_next(workers, completed, config.simulations, stats, cpu);
            let _ = tx.send(RunEvent::WorkersChanged { run_id, workers });
        }
    }

    clock.clear();
    let _ = tx.send(RunEvent::Finished { run_id });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::GameType;
    use std::collections::HashSet;
    use std::sync::mpsc;
    use std::time::Duration;

    fn stats(completed: usize, wins: usize, timeouts: usize) -> BatchStats {
        BatchStats {
            completed,
            wins,
            timeouts,
        }
    }

    #[test]
    fn auto_parallel_initial_uses_half_the_cpus() {
        assert_eq!(auto_parallel_initial(0, 8), 0);
        assert_eq!(auto_parallel_initial(-3, 8), 0);
        assert_eq!(auto_parallel_initial(1, 8), 1);
        assert_eq!(auto_parallel_initial(3, 8), 3);
        assert_eq!(auto_parallel_initial(100, 8), 4);
        assert_eq!(auto_parallel_initial(100, 1), 1);
    }

    #[test]
    fn auto_parallel_next_adjusts_by_batch_rates() {
        // Many timeouts → shrink.
        assert_eq!(auto_parallel_next(4, 10, 100, stats(10, 1, 8), 8), 3);
        assert_eq!(auto_parallel_next(1, 10, 100, stats(10, 1, 8), 8), 1);
        // Few timeouts and decent win rate → grow, capped by CPUs.
        assert_eq!(auto_parallel_next(2, 10, 100, stats(10, 5, 1), 8), 3);
        assert_eq!(auto_parallel_next(8, 10, 100, stats(10, 5, 1), 8), 8);
        assert_eq!(auto_parallel_next(2, 10, 100, stats(10, 5, 1), 2), 2);
        // In between → unchanged.
        assert_eq!(auto_parallel_next(4, 10, 100, stats(10, 1, 4), 8), 4);
        // Never more than the remaining games.
        assert_eq!(auto_parallel_next(16, 98, 100, stats(10, 9, 0), 8), 2);
        // Empty batch keeps current within the cap.
        assert_eq!(auto_parallel_next(16, 0, 100, stats(0, 0, 0), 8), 8);
    }

    #[test]
    fn manual_workers_are_clamped() {
        let config = |simulations, parallel_games| SimConfig {
            simulations,
            parallel_games,
            ..SimConfig::default()
        };
        assert_eq!(initial_workers(&config(0, 0), 8), 1);
        assert_eq!(initial_workers(&config(100, 4), 8), 4);
        assert_eq!(initial_workers(&config(100, 64), 8), 8);
        assert_eq!(initial_workers(&config(2, 4), 8), 2);
    }

    #[test]
    fn coordinator_returns_sequential_ids_then_none() {
        let coordinator = Coordinator::new(2);
        assert_eq!(coordinator.take_next_game_id(), Some(1));
        assert_eq!(coordinator.take_next_game_id(), Some(2));
        assert_eq!(coordinator.take_next_game_id(), None);
        assert_eq!(coordinator.take_next_game_id(), None);
    }

    #[test]
    fn coordinator_ids_are_unique_across_threads() {
        let coordinator = Coordinator::new(1000);
        let ids: Vec<u32> = thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    scope.spawn(|| {
                        let mut ids = Vec::new();
                        while let Some(id) = coordinator.take_next_game_id() {
                            ids.push(id);
                        }
                        ids
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap())
                .collect()
        });
        let unique: HashSet<u32> = ids.iter().copied().collect();
        assert_eq!(ids.len(), 1000);
        assert_eq!(unique, (1..=1000).collect());
    }

    #[test]
    fn active_games_clock_tracks_oldest_start() {
        let clock = ActiveGamesClock::default();
        assert!(clock.oldest_start().is_none());

        clock.begin(1);
        let first = clock.oldest_start().unwrap();
        thread::sleep(Duration::from_millis(5));
        clock.begin(2);
        assert_eq!(clock.oldest_start(), Some(first));

        clock.end(1);
        assert!(clock.oldest_start().unwrap() > first);

        clock.clear();
        assert!(clock.oldest_start().is_none());
    }

    fn collect_until_finished(rx: &mpsc::Receiver<RunEvent>) -> Vec<RunEvent> {
        let mut events = Vec::new();
        loop {
            let event = rx.recv_timeout(Duration::from_secs(60)).unwrap();
            let done = matches!(event, RunEvent::Finished { .. });
            events.push(event);
            if done {
                return events;
            }
        }
    }

    #[test]
    fn zero_simulations_finish_without_games() {
        let (tx, rx) = mpsc::channel();
        let config = SimConfig::default();
        spawn_simulation(
            config,
            ShuffleSource::Seeded(1),
            7,
            ActiveGamesClock::default(),
            tx,
        );
        let events = collect_until_finished(&rx);
        assert!(matches!(
            events.as_slice(),
            [
                RunEvent::WorkersChanged {
                    run_id: 7,
                    workers: 1
                },
                RunEvent::Finished { run_id: 7 }
            ]
        ));
    }

    #[test]
    fn runs_every_game_once() {
        let (tx, rx) = mpsc::channel();
        let config = SimConfig {
            game_type: GameType::TriPeaks,
            simulations: 3,
            parallel_games: 2,
            ..SimConfig::default()
        };
        let clock = ActiveGamesClock::default();
        spawn_simulation(config, ShuffleSource::Seeded(1), 1, clock.clone(), tx);
        let events = collect_until_finished(&rx);
        let ids: HashSet<u32> = events
            .iter()
            .filter_map(|e| match e {
                RunEvent::GameFinished { result, .. } => Some(result.id),
                _ => None,
            })
            .collect();
        assert_eq!(ids, HashSet::from([1, 2, 3]));
        assert!(clock.oldest_start().is_none());
    }

    #[test]
    fn cancel_stops_handing_out_games() {
        let (tx, rx) = mpsc::channel();
        let config = SimConfig {
            game_type: GameType::TriPeaks,
            simulations: 50,
            parallel_games: 1,
            ..SimConfig::default()
        };
        let handle = spawn_simulation(
            config,
            ShuffleSource::Seeded(1),
            1,
            ActiveGamesClock::default(),
            tx,
        );
        handle.cancel();
        let events = collect_until_finished(&rx);
        let games = events
            .iter()
            .filter(|e| matches!(e, RunEvent::GameFinished { .. }))
            .count();
        assert!(games <= 1, "got {games} games after cancel");
    }
}
