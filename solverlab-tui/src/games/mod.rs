//! Per-game runners, ported from `SimulationViewModel+<Game>.swift`.
//!
//! Each runner deals a shuffled deck, asks the solver for a (possibly partial)
//! plan, replays it through `apply_move` and reports score and stop reason the
//! same way the Swift console does.

pub mod freecell;
pub mod klondike;

use crate::deck::ShuffleSource;
use crate::model::{SimConfig, SimResult, StopReason};

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
}
