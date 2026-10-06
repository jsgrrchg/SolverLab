use std::time::Instant;

use solver_core::TriPeaksEngine;

use super::{GameContext, classify_stop_reason, deal_failed};
use crate::deck::{shuffled, standard_deck};
use crate::model::SimResult;

/// TriPeaks reports timeout against the solver's fixed 5s budget, not the config.
const TIMEOUT_SECS: f64 = 5.0;

/// Tableau plays extend the streak and score `5 * streak`; a stock draw
/// breaks the streak and costs 5 points (floored at zero).
pub(crate) fn tripeaks_score(score: i64, streak: i64, is_tableau: bool) -> (i64, i64) {
    if is_tableau {
        let streak = streak + 1;
        (score + 5 * streak, streak)
    } else {
        ((score - 5).max(0), 0)
    }
}

pub fn run(ctx: &GameContext) -> SimResult {
    let deck = shuffled(standard_deck(), ctx.shuffle, ctx.game_id);
    let engine = TriPeaksEngine::new();
    let Ok(board) = engine.deal(deck) else {
        return deal_failed(ctx.game_id, None);
    };

    let start = Instant::now();
    let mut final_board = board.clone();
    let mut score = 0;
    let mut streak = 0;
    let mut applied_moves = 0;

    let planned = engine.solve(&final_board, true);
    let checkpoints = i64::from(engine.last_checkpoints_adopted());

    for m in planned.into_iter().flatten() {
        let is_tableau = engine.is_tableau_move(m.clone());
        let Some(next) = engine.apply_move(&final_board, m) else {
            break;
        };
        (score, streak) = tripeaks_score(score, streak, is_tableau);
        final_board = next;
        applied_moves += 1;
    }

    let duration = start.elapsed().as_secs_f64();
    let won = engine.is_win(&final_board);
    let timed_out = duration >= TIMEOUT_SECS;
    let has_progress = engine.progress_token(&final_board) != engine.progress_token(&board);

    SimResult {
        id: ctx.game_id,
        move_count: applied_moves,
        undo_count: 0,
        checkpoint_count: Some(checkpoints),
        won,
        stop_reason: classify_stop_reason(won, timed_out, has_progress),
        duration,
        score,
        moves_detail: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deck::ShuffleSource;
    use crate::model::{GameType, SimConfig, StopReason};

    #[test]
    fn streak_scoring() {
        let (score, streak) = tripeaks_score(0, 0, true);
        assert_eq!((score, streak), (5, 1));
        let (score, streak) = tripeaks_score(score, streak, true);
        assert_eq!((score, streak), (15, 2));
        let (score, streak) = tripeaks_score(score, streak, true);
        assert_eq!((score, streak), (30, 3));
        let (score, streak) = tripeaks_score(score, streak, false);
        assert_eq!((score, streak), (25, 0));
        assert_eq!(tripeaks_score(3, 4, false), (0, 0));
    }

    #[test]
    fn seeded_tripeaks_returns_consistent_result() {
        let config = SimConfig {
            game_type: GameType::TriPeaks,
            ..SimConfig::default()
        };
        let ctx = GameContext {
            game_id: 1,
            config: &config,
            shuffle: ShuffleSource::Seeded(1),
        };
        let result = run(&ctx);
        assert_eq!(result.won, result.stop_reason == StopReason::Win);
        assert!(result.score >= 0);
    }
}
