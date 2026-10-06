use std::time::Instant;

use solver_core::FreeCellEngine;

use super::{GameContext, classify_stop_reason, deal_failed};
use crate::deck::{shuffled, standard_deck};
use crate::model::SimResult;

/// Solver timeout used when the config does not set one.
const DEFAULT_SOLVE_TIMEOUT_SECS: f64 = 15.0;

pub fn run(ctx: &GameContext) -> SimResult {
    let deck = shuffled(standard_deck(), ctx.shuffle, ctx.game_id);
    let engine = FreeCellEngine::new();
    let Ok(board) = engine.deal(deck) else {
        return deal_failed(ctx.game_id, None);
    };

    let start = Instant::now();
    let timeout_budget = ctx.timeout_budget();
    let mut final_board = board.clone();
    let mut score = 0;
    let mut applied_moves = 0;

    let planned = engine.solve(
        &final_board,
        timeout_budget.unwrap_or(DEFAULT_SOLVE_TIMEOUT_SECS),
        true,
    );
    let checkpoints = i64::from(engine.last_checkpoints_adopted());

    // Replay the plan to get the final board and the UI score (+10 per foundation move).
    for m in planned.into_iter().flatten() {
        let is_foundation = engine.is_foundation_move(m.clone());
        let Some(next) = engine.apply_move(&final_board, m) else {
            break;
        };
        final_board = next;
        applied_moves += 1;
        if is_foundation {
            score += 10;
        }
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

    #[test]
    fn seeded_freecell_returns_consistent_result() {
        let config = SimConfig {
            game_type: GameType::FreeCell,
            timeout_seconds: 2.0,
            ..SimConfig::default()
        };
        let ctx = GameContext {
            game_id: 1,
            config: &config,
            shuffle: ShuffleSource::Seeded(1),
        };
        let result = run(&ctx);
        assert_eq!(result.id, 1);
        assert!(result.move_count >= 0);
        assert_eq!(result.won, result.stop_reason == StopReason::Win);
        assert_eq!(result.score % 10, 0);
        if result.won {
            assert_eq!(result.score, 520);
        }
    }
}
