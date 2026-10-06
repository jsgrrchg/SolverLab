//! Per-game runners, ported from `SimulationViewModel+<Game>.swift`.
//!
//! Each runner deals a shuffled deck, asks the solver for a (possibly partial)
//! plan, replays it through `apply_move` and reports score and stop reason the
//! same way the Swift console does.

pub mod freecell;
pub mod klondike;
pub mod pyramid;
pub mod spider;
pub mod tripeaks;

use crate::deck::ShuffleSource;
use crate::model::{GameType, SimConfig, SimResult, StopReason};

pub struct GameContext<'a> {
    pub game_id: u32,
    pub config: &'a SimConfig,
    pub shuffle: ShuffleSource,
}

impl GameContext<'_> {
    /// `config.timeoutSeconds` when positive, as the Swift runners read it.
    fn timeout_budget(&self) -> Option<f64> {
        (self.config.timeout_seconds > 0.0).then_some(self.config.timeout_seconds)
    }
}

/// Port of `runSingleGame`.
pub fn run_single_game(ctx: &GameContext) -> SimResult {
    match ctx.config.game_type {
        GameType::KlondikeDraw1 => klondike::run(ctx, 1),
        GameType::KlondikeDraw3 => klondike::run(ctx, 3),
        GameType::FreeCell => freecell::run(ctx),
        GameType::Pyramid => pyramid::run(ctx),
        GameType::TriPeaks => tripeaks::run(ctx),
        GameType::Spider1Suit => spider::run(ctx, 1),
        GameType::Spider2Suit => spider::run(ctx, 2),
        GameType::Spider4Suit => spider::run(ctx, 4),
    }
}

/// Timeout is reported only if useful progress was made; otherwise the game stalled.
pub fn classify_stop_reason(won: bool, timed_out: bool, has_progress: bool) -> StopReason {
    if won {
        StopReason::Win
    } else if timed_out && has_progress {
        StopReason::Timeout
    } else {
        StopReason::Stalled
    }
}

fn deal_failed(game_id: u32, checkpoint_count: Option<i64>) -> SimResult {
    SimResult {
        id: game_id,
        move_count: 0,
        undo_count: 0,
        checkpoint_count,
        won: false,
        stop_reason: StopReason::Stalled,
        duration: 0.0,
        score: 0,
        moves_detail: "deal_failed".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_stop_reason_table() {
        use StopReason::*;
        let cases = [
            (true, true, true, Win),
            (true, true, false, Win),
            (true, false, true, Win),
            (true, false, false, Win),
            (false, true, true, Timeout),
            (false, true, false, Stalled),
            (false, false, true, Stalled),
            (false, false, false, Stalled),
        ];
        for (won, timed_out, progress, expected) in cases {
            assert_eq!(classify_stop_reason(won, timed_out, progress), expected);
        }
    }

    /// Seeded runs must be reproducible: these are the columns compared across platforms.
    #[test]
    #[ignore = "slow: runs real solvers twice (use --release)"]
    fn seeded_runs_are_deterministic() {
        for game_type in [GameType::FreeCell, GameType::Pyramid, GameType::TriPeaks] {
            let config = SimConfig {
                game_type,
                ..SimConfig::default()
            };
            for game_id in 1..=3 {
                let ctx = GameContext {
                    game_id,
                    config: &config,
                    shuffle: ShuffleSource::Seeded(1234),
                };
                let a = run_single_game(&ctx);
                let b = run_single_game(&ctx);
                assert_eq!(
                    (
                        a.move_count,
                        a.undo_count,
                        a.checkpoint_count,
                        a.won,
                        a.score
                    ),
                    (
                        b.move_count,
                        b.undo_count,
                        b.checkpoint_count,
                        b.won,
                        b.score
                    ),
                    "{game_type:?} game {game_id}"
                );
            }
        }
    }
}
