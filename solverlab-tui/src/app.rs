//! Application state, mirroring `SimulationViewModel` (metrics, run control,
//! countdown and ETA). The UI only reads it; input only calls its methods.

use std::collections::VecDeque;
use std::sync::mpsc::Sender;
use std::time::Instant;

use crate::deck::ShuffleSource;
use crate::format::format_duration;
use crate::model::{SimConfig, SimResult, StopReason};
use crate::runtime::{
    ActiveGamesClock, RunEvent, RunHandle, auto_parallel_initial, cpu_count, spawn_simulation,
};

/// Rows kept for the results table (`allResults` keeps everything for the CSV).
pub const VISIBLE_RESULTS_CAP: usize = 1500;
/// Countdown shown when the config has no timeout.
const DEFAULT_TIMEOUT_SECS: f64 = 15.0;

pub type Spawner =
    Box<dyn Fn(SimConfig, ShuffleSource, u64, ActiveGamesClock, Sender<RunEvent>) -> RunHandle>;

pub struct App {
    pub config: SimConfig,
    pub shuffle: ShuffleSource,
    pub is_running: bool,
    pub completed_games: u64,
    pub wins: u64,
    pub total_moves: i64,
    pub total_undos: i64,
    pub total_duration: f64,
    pub timeout_count: u64,
    pub stalled_count: u64,
    pub win_count: u64,
    pub results: VecDeque<SimResult>,
    pub all_results: Vec<SimResult>,
    pub status_text: String,
    pub countdown_remaining: f64,
    pub estimated_remaining_total: f64,
    pub current_parallel_used: usize,
    pub(crate) last_run_config: Option<SimConfig>,
    run_started_at: Option<Instant>,
    run: Option<RunHandle>,
    next_run_id: u64,
    clock: ActiveGamesClock,
    tx: Sender<RunEvent>,
    spawner: Spawner,
    cpu: usize,
}

impl App {
    pub fn new(config: SimConfig, shuffle: ShuffleSource, tx: Sender<RunEvent>) -> Self {
        Self::with_spawner(config, shuffle, tx, Box::new(spawn_simulation), cpu_count())
    }

    pub fn with_spawner(
        config: SimConfig,
        shuffle: ShuffleSource,
        tx: Sender<RunEvent>,
        spawner: Spawner,
        cpu: usize,
    ) -> Self {
        App {
            config,
            shuffle,
            is_running: false,
            completed_games: 0,
            wins: 0,
            total_moves: 0,
            total_undos: 0,
            total_duration: 0.0,
            timeout_count: 0,
            stalled_count: 0,
            win_count: 0,
            results: VecDeque::new(),
            all_results: Vec::new(),
            status_text: "Ready".into(),
            countdown_remaining: 0.0,
            estimated_remaining_total: 0.0,
            current_parallel_used: 0,
            last_run_config: None,
            run_started_at: None,
            run: None,
            next_run_id: 1,
            clock: ActiveGamesClock::default(),
            tx,
            spawner,
            cpu,
        }
    }

    fn average(&self, total: f64) -> f64 {
        if self.completed_games == 0 {
            0.0
        } else {
            total / self.completed_games as f64
        }
    }

    pub fn win_rate(&self) -> f64 {
        self.average(self.wins as f64)
    }

    pub fn average_moves(&self) -> f64 {
        self.average(self.total_moves as f64)
    }

    pub fn average_duration(&self) -> f64 {
        self.average(self.total_duration)
    }

    pub fn average_undos(&self) -> f64 {
        self.average(self.total_undos as f64)
    }

    pub fn progress_fraction(&self) -> f64 {
        let total = self.config.simulations.max(1) as f64;
        (self.completed_games as f64 / total).min(1.0)
    }

    pub fn countdown_display(&self) -> String {
        format_duration(self.countdown_remaining)
    }

    pub fn estimated_remaining_display(&self) -> String {
        format_duration(self.estimated_remaining_total)
    }

    pub fn start(&mut self) {
        if self.is_running {
            return;
        }
        self.reset_metrics();
        self.is_running = true;
        self.status_text = "Running...".into();

        let config = self.config.clone();
        self.last_run_config = Some(config.clone());
        self.run_started_at = Some(Instant::now());
        self.countdown_remaining = timeout_or_default(&config);
        self.estimated_remaining_total = 0.0;

        let run_id = self.next_run_id;
        self.next_run_id += 1;
        self.run = Some((self.spawner)(
            config,
            self.shuffle,
            run_id,
            self.clock.clone(),
            self.tx.clone(),
        ));
    }

    fn cancel_run(&mut self) {
        if let Some(run) = self.run.take() {
            run.cancel();
        }
        self.is_running = false;
        self.run_started_at = None;
        self.countdown_remaining = 0.0;
        self.estimated_remaining_total = 0.0;
        self.current_parallel_used = 0;
    }

    pub fn stop(&mut self) {
        self.cancel_run();
        self.status_text = "Stopped".into();
    }

    pub fn clear_data(&mut self) {
        self.cancel_run();
        self.reset_metrics();
        self.status_text = "Data cleared".into();
    }

    fn reset_metrics(&mut self) {
        self.completed_games = 0;
        self.wins = 0;
        self.total_moves = 0;
        self.total_undos = 0;
        self.total_duration = 0.0;
        self.timeout_count = 0;
        self.stalled_count = 0;
        self.win_count = 0;
        self.results.clear();
        self.all_results.clear();
        self.last_run_config = None;
        self.run_started_at = None;
        self.countdown_remaining = 0.0;
        self.estimated_remaining_total = 0.0;
        self.current_parallel_used = 0;
        self.status_text = "Ready".into();
    }

    /// Applies a runtime event; events from stopped or older runs are ignored.
    pub fn handle_event(&mut self, event: RunEvent) {
        let current = self.run.as_ref().map(|run| run.run_id);
        match event {
            RunEvent::WorkersChanged { run_id, workers } if Some(run_id) == current => {
                self.current_parallel_used = workers;
            }
            RunEvent::GameFinished { run_id, result } if Some(run_id) == current => {
                self.record(result);
            }
            RunEvent::Finished { run_id } if Some(run_id) == current => {
                self.run = None;
                self.clock.clear();
                self.run_started_at = None;
                self.countdown_remaining = 0.0;
                self.estimated_remaining_total = 0.0;
                self.current_parallel_used = 0;
                self.is_running = false;
                self.status_text = "Completed".into();
            }
            _ => {}
        }
    }

    pub(crate) fn record(&mut self, result: SimResult) {
        self.completed_games += 1;
        if result.won {
            self.wins += 1;
        }
        self.total_moves += result.move_count;
        self.total_undos += result.undo_count;
        self.total_duration += result.duration;
        match result.stop_reason {
            StopReason::Win => self.win_count += 1,
            StopReason::Stalled => self.stalled_count += 1,
            StopReason::Timeout => self.timeout_count += 1,
        }

        self.all_results.push(result.clone());
        self.results.push_back(result);
        while self.results.len() > VISIBLE_RESULTS_CAP {
            self.results.pop_front();
        }

        let total = self
            .last_run_config
            .as_ref()
            .map_or(self.config.simulations, |c| c.simulations);
        self.status_text = format!("Running {}/{}", self.completed_games, total);
    }

    /// Port of `runCountdown`: refreshes the active timeout and the ETA.
    pub fn tick(&mut self, now: Instant) {
        if !self.is_running {
            return;
        }
        let Some(config) = self.last_run_config.clone() else {
            return;
        };
        let timeout = timeout_or_default(&config);
        let total_games = config.simulations.max(1);
        let configured_workers = config.parallel_games.min(total_games).max(1);

        self.countdown_remaining = self.clock.oldest_start().map_or(timeout, |oldest| {
            (timeout - now.saturating_duration_since(oldest).as_secs_f64()).max(0.0)
        });

        let completed = i64::try_from(self.completed_games).unwrap_or(i64::MAX);
        let remaining_games = (total_games - completed).max(0);
        if remaining_games == 0 {
            self.estimated_remaining_total = 0.0;
            return;
        }

        if let Some(started_at) = self.run_started_at
            && self.completed_games > 0
        {
            let elapsed = now.saturating_duration_since(started_at).as_secs_f64();
            let per_game = elapsed / self.completed_games as f64;
            self.estimated_remaining_total = (remaining_games as f64 * per_game).max(0.0);
        } else {
            let fallback = if config.auto_parallel {
                auto_parallel_initial(total_games, self.cpu) as i64
            } else {
                configured_workers
            };
            let dynamic = if self.current_parallel_used > 0 {
                self.current_parallel_used as i64
            } else {
                fallback
            };
            let workers = dynamic.min(remaining_games).max(1);
            self.estimated_remaining_total =
                (remaining_games as f64 * timeout / workers as f64).max(0.0);
        }
    }
}

fn timeout_or_default(config: &SimConfig) -> f64 {
    if config.timeout_seconds > 0.0 {
        config.timeout_seconds
    } else {
        DEFAULT_TIMEOUT_SECS
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::mpsc::{self, Receiver};
    use std::time::Duration;

    pub(crate) fn result(id: u32, won: bool, stop_reason: StopReason) -> SimResult {
        SimResult {
            id,
            move_count: 10,
            undo_count: 1,
            checkpoint_count: Some(2),
            won,
            stop_reason,
            duration: 1.5,
            score: 50,
            moves_detail: String::new(),
        }
    }

    /// App whose runs never spawn threads; events are injected by the test.
    pub(crate) fn idle_app(config: SimConfig) -> (App, Receiver<RunEvent>) {
        let (tx, rx) = mpsc::channel();
        let spawner: Spawner = Box::new(|_, _, run_id, _, _| RunHandle::idle(run_id));
        (
            App::with_spawner(config, ShuffleSource::Seeded(1), tx, spawner, 8),
            rx,
        )
    }

    #[test]
    fn metrics() {
        let (mut app, _rx) = idle_app(SimConfig::default());
        app.config.simulations = 4;
        app.completed_games = 2;
        app.wins = 1;
        app.total_moves = 30;
        app.total_duration = 12.0;
        app.total_undos = 4;

        assert_eq!(app.win_rate(), 0.5);
        assert_eq!(app.average_moves(), 15.0);
        assert_eq!(app.average_duration(), 6.0);
        assert_eq!(app.average_undos(), 2.0);
        assert_eq!(app.progress_fraction(), 0.5);
    }

    #[test]
    fn stop_clears_running_state_and_timers() {
        let (mut app, _rx) = idle_app(SimConfig::default());
        app.start();
        app.countdown_remaining = 10.0;
        app.estimated_remaining_total = 20.0;

        app.stop();

        assert!(!app.is_running);
        assert_eq!(app.countdown_remaining, 0.0);
        assert_eq!(app.estimated_remaining_total, 0.0);
        assert!(app.run_started_at.is_none());
        assert_eq!(app.status_text, "Stopped");
    }

    #[test]
    fn clear_data_resets_counters_and_results() {
        let (mut app, _rx) = idle_app(SimConfig::default());
        app.completed_games = 3;
        app.wins = 2;
        app.total_moves = 50;
        app.total_undos = 7;
        app.total_duration = 9.0;
        app.timeout_count = 1;
        app.stalled_count = 1;
        app.win_count = 1;
        app.results.push_back(result(1, true, StopReason::Win));
        app.all_results = app.results.iter().cloned().collect();

        app.clear_data();

        assert_eq!(app.completed_games, 0);
        assert_eq!(app.wins, 0);
        assert_eq!(app.total_moves, 0);
        assert_eq!(app.total_undos, 0);
        assert_eq!(app.total_duration, 0.0);
        assert_eq!(app.timeout_count, 0);
        assert_eq!(app.stalled_count, 0);
        assert_eq!(app.win_count, 0);
        assert!(app.results.is_empty());
        assert!(app.all_results.is_empty());
        assert_eq!(app.status_text, "Data cleared");
    }

    #[test]
    fn start_sets_running_state() {
        let config = SimConfig {
            simulations: 10,
            timeout_seconds: 0.0,
            ..SimConfig::default()
        };
        let (mut app, _rx) = idle_app(config.clone());
        app.start();
        assert!(app.is_running);
        assert_eq!(app.status_text, "Running...");
        assert_eq!(app.countdown_remaining, 15.0);
        assert_eq!(app.last_run_config, Some(config));
    }

    #[test]
    fn events_update_metrics_and_finish() {
        let config = SimConfig {
            simulations: 3,
            ..SimConfig::default()
        };
        let (mut app, _rx) = idle_app(config);
        app.start();
        let run_id = app.run.as_ref().unwrap().run_id;

        app.handle_event(RunEvent::WorkersChanged { run_id, workers: 2 });
        assert_eq!(app.current_parallel_used, 2);

        app.handle_event(RunEvent::GameFinished {
            run_id,
            result: result(1, true, StopReason::Win),
        });
        app.handle_event(RunEvent::GameFinished {
            run_id,
            result: result(2, false, StopReason::Timeout),
        });
        assert_eq!(app.completed_games, 2);
        assert_eq!(
            (app.win_count, app.timeout_count, app.stalled_count),
            (1, 1, 0)
        );
        assert_eq!(app.status_text, "Running 2/3");

        app.handle_event(RunEvent::Finished { run_id });
        assert!(!app.is_running);
        assert_eq!(app.status_text, "Completed");
        assert_eq!(app.current_parallel_used, 0);
    }

    #[test]
    fn events_from_stopped_or_old_runs_are_ignored() {
        let (mut app, _rx) = idle_app(SimConfig {
            simulations: 5,
            ..SimConfig::default()
        });
        app.start();
        let old_run = app.run.as_ref().unwrap().run_id;
        app.stop();
        app.handle_event(RunEvent::GameFinished {
            run_id: old_run,
            result: result(1, true, StopReason::Win),
        });
        assert_eq!(app.completed_games, 0);

        app.start();
        app.handle_event(RunEvent::Finished { run_id: old_run });
        assert!(app.is_running);
    }

    #[test]
    fn record_caps_visible_results_but_keeps_all() {
        let (mut app, _rx) = idle_app(SimConfig::default());
        for id in 1..=(VISIBLE_RESULTS_CAP as u32 + 5) {
            app.record(result(id, false, StopReason::Stalled));
        }
        assert_eq!(app.results.len(), VISIBLE_RESULTS_CAP);
        assert_eq!(app.results.front().unwrap().id, 6);
        assert_eq!(app.all_results.len(), VISIBLE_RESULTS_CAP + 5);
    }

    #[test]
    fn tick_without_completed_games_uses_conservative_estimate() {
        let config = SimConfig {
            simulations: 10,
            parallel_games: 2,
            timeout_seconds: 4.0,
            ..SimConfig::default()
        };
        let (mut app, _rx) = idle_app(config);
        app.start();
        app.tick(Instant::now());
        assert_eq!(app.countdown_remaining, 4.0);
        // 10 games * 4s / 2 workers.
        assert_eq!(app.estimated_remaining_total, 20.0);

        app.current_parallel_used = 5;
        app.tick(Instant::now());
        assert_eq!(app.estimated_remaining_total, 8.0);
    }

    #[test]
    fn tick_with_completed_games_uses_wall_time_per_game() {
        let (mut app, _rx) = idle_app(SimConfig {
            simulations: 4,
            ..SimConfig::default()
        });
        app.start();
        let started = app.run_started_at.unwrap();
        app.record(result(1, true, StopReason::Win));
        app.tick(started + Duration::from_secs(6));
        // 3 remaining games * 6s per game.
        assert!((app.estimated_remaining_total - 18.0).abs() < 1e-9);
    }

    #[test]
    fn tick_counts_down_from_oldest_active_game() {
        let (mut app, _rx) = idle_app(SimConfig {
            simulations: 4,
            timeout_seconds: 10.0,
            ..SimConfig::default()
        });
        app.start();
        app.clock.begin(1);
        let oldest = app.clock.oldest_start().unwrap();
        app.tick(oldest + Duration::from_secs(3));
        assert!((app.countdown_remaining - 7.0).abs() < 1e-9);
        app.tick(oldest + Duration::from_secs(30));
        assert_eq!(app.countdown_remaining, 0.0);
    }
}
