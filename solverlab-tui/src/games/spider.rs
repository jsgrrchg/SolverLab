use std::time::Instant;

use solver_core::SpiderEngine;

use super::{GameContext, classify_stop_reason, deal_failed};
use crate::deck::{shuffled, spider_deck};
use crate::model::SimResult;

const INITIAL_SCORE: i64 = 500;

pub fn run(ctx: &GameContext, suit_count: u32) -> SimResult {
    let deck = shuffled(spider_deck(suit_count), ctx.shuffle, ctx.game_id);
    let engine = SpiderEngine::new(suit_count);
    let Ok(board) = engine.deal(deck) else {
        return deal_failed(ctx.game_id, None);
    };

    let start = Instant::now();
    let timeout_budget = ctx.timeout_budget();
    let mut final_board = board.clone();
    let mut score = INITIAL_SCORE;
    let mut applied_moves = 0;

    let planned = engine.solve(&final_board, true);
    let checkpoints = i64::from(engine.last_checkpoints_adopted());

    // -1 per move, +100 per newly completed set.
    for m in planned.into_iter().flatten() {
        let sets_before = engine.completed_sets(&final_board);
        let Some(next) = engine.apply_move(&final_board, m) else {
            break;
        };
        final_board = next;
        applied_moves += 1;
        score -= 1;

        let new_sets = engine.completed_sets(&final_board) - sets_before;
        if new_sets > 0 {
            score += i64::from(new_sets) * 100;
        }

        if engine.is_win(&final_board) {
            break;
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
    #[ignore = "slow: runs the real Spider solver (use --release)"]
    fn smoke_seeded_spider_one_suit() {
        let config = SimConfig {
            game_type: GameType::Spider1Suit,
            ..SimConfig::default()
        };
        let ctx = GameContext {
            game_id: 1,
            config: &config,
            shuffle: ShuffleSource::Seeded(1),
        };
        let result = run(&ctx, 1);
        assert_eq!(result.won, result.stop_reason == StopReason::Win);
        if result.won {
            assert_eq!(result.score, INITIAL_SCORE - result.move_count + 800);
        }
    }
}
