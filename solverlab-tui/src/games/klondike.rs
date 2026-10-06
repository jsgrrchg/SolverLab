use std::time::Instant;

use solver_core::KlondikeEngine;

use super::{GameContext, classify_stop_reason, deal_failed};
use crate::deck::{shuffled, standard_deck};
use crate::model::SimResult;

/// Klondike reports timeout against the solver's fixed 100s budget, not the config.
const TIMEOUT_SECS: f64 = 100.0;

/// Score update for one applied move. Returns the new score and whether the
/// move counts as an undo (taking a card back from a foundation).
pub(crate) fn klondike_score(
    score: i64,
    to_foundation: bool,
    to_column: bool,
    from_foundation: bool,
    face_down_before: Option<i32>,
    face_down_after: Option<i32>,
) -> (i64, bool) {
    let mut score = score;
    if to_foundation {
        score += 10;
    } else if to_column {
        if from_foundation {
            score = (score - 15).max(0);
        } else {
            score += 5;
        }
    }
    if let (Some(before), Some(after)) = (face_down_before, face_down_after)
        && before >= 1
        && after == before - 1
    {
        score += 5;
    }
    (score, from_foundation)
}

pub fn run(ctx: &GameContext, draw_advance: i32) -> SimResult {
    let deck = shuffled(standard_deck(), ctx.shuffle, ctx.game_id);
    let engine = KlondikeEngine::new(draw_advance);
    let Ok(board) = engine.deal(deck) else {
        return deal_failed(ctx.game_id, Some(0));
    };

    let start = Instant::now();
    let mut final_board = board.clone();
    let mut score = 0;
    let mut undo_count = 0;
    let mut applied_moves = 0;

    let planned = engine.solve(&final_board, true);
    let checkpoints = i64::from(engine.last_checkpoints_adopted());

    for m in planned.into_iter().flatten() {
        let from_foundation = engine.is_from_foundation(m.clone());
        let to_foundation = engine.is_to_foundation(m.clone());
        let to_column = engine.is_to_column(m.clone());
        let source = m.source;
        let tracks_column = (0..7).contains(&source) && m.move_type.starts_with("columnTo");
        let face_down_before = tracks_column.then(|| engine.face_down_count(&final_board, source));

        let Some(next) = engine.apply_move(&final_board, m) else {
            break;
        };
        final_board = next;
        applied_moves += 1;

        let face_down_after = tracks_column.then(|| engine.face_down_count(&final_board, source));
        let (next_score, undo) = klondike_score(
            score,
            to_foundation,
            to_column,
            from_foundation,
            face_down_before,
            face_down_after,
        );
        score = next_score;
        if undo {
            undo_count += 1;
        }
    }

    let duration = start.elapsed().as_secs_f64();
    let won = engine.is_win(&final_board);
    let timed_out = duration >= TIMEOUT_SECS;
    let has_progress = engine.progress_token(&final_board) != engine.progress_token(&board);

    SimResult {
        id: ctx.game_id,
        move_count: applied_moves,
        undo_count,
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
    fn score_to_foundation() {
        assert_eq!(
            klondike_score(0, true, false, false, None, None),
            (10, false)
        );
    }

    #[test]
    fn score_column_move_that_reveals_a_card() {
        assert_eq!(
            klondike_score(10, false, true, false, Some(3), Some(2)),
            (20, false)
        );
        // No bonus when nothing was face down or nothing changed.
        assert_eq!(
            klondike_score(10, false, true, false, Some(0), Some(0)),
            (15, false)
        );
        assert_eq!(
            klondike_score(10, false, true, false, Some(2), Some(2)),
            (15, false)
        );
    }

    #[test]
    fn score_from_foundation_is_an_undo_with_floor() {
        assert_eq!(klondike_score(20, false, true, true, None, None), (5, true));
        assert_eq!(klondike_score(4, false, true, true, None, None), (0, true));
    }

    #[test]
    fn score_stock_to_column_has_no_reveal_bonus() {
        assert_eq!(
            klondike_score(0, false, true, false, None, None),
            (5, false)
        );
    }

    #[test]
    #[ignore = "slow: runs the real Klondike solver (use --release)"]
    fn smoke_seeded_klondike_draw1() {
        let config = SimConfig {
            game_type: GameType::KlondikeDraw1,
            ..SimConfig::default()
        };
        let ctx = GameContext {
            game_id: 1,
            config: &config,
            shuffle: ShuffleSource::Seeded(1),
        };
        let result = run(&ctx, 1);
        assert_eq!(result.id, 1);
        assert!(result.move_count >= 0);
        assert_eq!(result.won, result.stop_reason == StopReason::Win);
    }
}
