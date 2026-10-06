use std::time::Instant;

use solver_core::PyramidEngine;

use super::{GameContext, classify_stop_reason, deal_failed};
use crate::deck::{shuffled, standard_deck};
use crate::model::SimResult;

pub fn run(ctx: &GameContext) -> SimResult {
    let deck = shuffled(standard_deck(), ctx.shuffle, ctx.game_id);
    let engine = PyramidEngine::new();
    let Ok(board) = engine.deal(deck) else {
        return deal_failed(ctx.game_id, None);
    };

    let start = Instant::now();
    let timeout_budget = ctx.timeout_budget();
    let mut final_board = board.clone();
    let mut score = 0;
    let mut applied_moves = 0;

    // A zero timeout means "no timeout" for the Pyramid solver.
    let planned = engine.solve(&final_board, timeout_budget.unwrap_or(0.0), true);
    let checkpoints = i64::from(engine.last_checkpoints_adopted());

    for m in planned.into_iter().flatten() {
        let is_king = engine.is_king_move(m.clone());
        let is_pair = engine.is_pair_move(m.clone());
        let Some(next) = engine.apply_move(&final_board, m) else {
            break;
        };
        final_board = next;
        if is_king {
            score += 10;
        } else if is_pair {
            score += 20;
        }
        applied_moves += 1;
    }

    let duration = start.elapsed().as_secs_f64();
    let won = engine.is_win(&final_board);
    let timed_out = timeout_budget.is_some_and(|budget| duration >= budget);
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
    use solver_core::PyramidMoveDesc;

    /// The Swift runner compares move type strings; the engine helpers must agree.
    #[test]
    fn engine_move_kinds_match_swift_string_checks() {
        let engine = PyramidEngine::new();
        for move_type in [
            "stockAdvance",
            "stockReset",
            "pairWP",
            "pairPP",
            "kingPyramid",
            "kingWaste",
            "deal",
        ] {
            let m = PyramidMoveDesc {
                move_type: move_type.into(),
                index_a: -1,
                index_b: -1,
            };
            assert_eq!(
                engine.is_king_move(m.clone()),
                move_type == "kingPyramid" || move_type == "kingWaste"
            );
            assert_eq!(
                engine.is_pair_move(m),
                move_type == "pairWP" || move_type == "pairPP"
            );
        }
    }

    #[test]
    fn seeded_pyramid_returns_consistent_result() {
        let config = SimConfig {
            game_type: GameType::Pyramid,
            timeout_seconds: 2.0,
            ..SimConfig::default()
        };
        let ctx = GameContext {
            game_id: 1,
            config: &config,
            shuffle: ShuffleSource::Seeded(1),
        };
        let result = run(&ctx);
        assert_eq!(result.won, result.stop_reason == StopReason::Win);
        assert_eq!(result.score % 10, 0);
    }
}
