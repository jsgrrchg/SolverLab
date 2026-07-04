use super::board::SpiderBoard;
use super::moves::SpiderMove;
use super::rules::{SpiderRule, SpiderTransition};
use super::weights;
use crate::common::card::Card;

/// Spider move-generation engine: generates successors with rule pruning and priority ordering.
pub struct SpiderEngine {
    pub suit_count: u32,
    pub rules: Vec<SpiderRule>,
}

impl SpiderEngine {
    /// Creates the engine with default rules adjusted to the variant.
    pub fn new(suit_count: u32) -> SpiderEngine {
        SpiderEngine {
            suit_count,
            rules: SpiderRule::default_rules(suit_count),
        }
    }

    /// Win condition: 8 completed sequences.
    pub fn is_win(board: &SpiderBoard) -> bool {
        board.completed_sets == 8
    }

    /// Generates successor transitions with pruning and priority ordering.
    /// `attempt` controls deterministic ordering perturbation:
    /// attempt=0 uses the original ordering; attempt>0 adds noise
    /// to explore alternative paths.
    pub fn successors(
        &self,
        board: &SpiderBoard,
        previous_move: Option<&SpiderMove>,
        depth: usize,
        attempt: u64,
    ) -> Vec<SpiderTransition> {
        let candidate_moves = find_candidate_moves(board);
        let mut transitions = Vec::with_capacity(candidate_moves.len());
        let next_depth = depth + 1;

        for the_move in candidate_moves {
            // Early pruning: before `apply()` (`can_prune_early` optimization).
            let mut early_pruned = false;
            for rule in &self.rules {
                if rule.can_prune_early(&the_move, previous_move, next_depth) {
                    early_pruned = true;
                    break;
                }
            }
            if early_pruned {
                continue;
            }

            let next_board = match the_move.apply(board) {
                Some(b) => b,
                None => continue,
            };

            let transition = SpiderTransition {
                to_board: next_board,
                the_move,
                depth: next_depth,
            };

            let mut pruned = false;
            for rule in &self.rules {
                if rule.should_prune(&transition, board) {
                    pruned = true;
                    break;
                }
            }
            if !pruned {
                transitions.push(transition);
            }
        }

        // Precompute source-board metrics once (optimization).
        let from_face_down = board.total_face_down();
        let from_suit_run_total: usize = board.columns.iter().map(|c| c.longest_run).sum();
        let targets = board.suit_targets();
        let sc = self.suit_count;

        // 3.3: Two-level bucket ordering.
        // Bucket (ascending) -> local_priority (descending) within each bucket.
        // With attempt>0, deterministic perturbation is added to diversify exploration.
        transitions.sort_by(|lhs, rhs| {
            let lb = move_bucket(lhs, board, from_suit_run_total, &targets);
            let rb = move_bucket(rhs, board, from_suit_run_total, &targets);
            if lb != rb {
                return lb.cmp(&rb);
            }
            let lp = local_priority(
                lhs,
                board,
                from_face_down,
                from_suit_run_total,
                &targets,
                sc,
            ) + move_perturbation(attempt, board.signature, &lhs.the_move, sc);
            let rp = local_priority(
                rhs,
                board,
                from_face_down,
                from_suit_run_total,
                &targets,
                sc,
            ) + move_perturbation(attempt, board.signature, &rhs.the_move, sc);
            rp.cmp(&lp)
        });

        transitions
    }
}

/// Collects all candidate moves from the current board.
fn find_candidate_moves(board: &SpiderBoard) -> Vec<SpiderMove> {
    let mut moves = SpiderMove::find_column_to_column_moves(board);
    moves.extend(SpiderMove::find_deal_from_stock_moves(board));
    moves
}

/// 3.3: Assigns each transition to a priority bucket (0 = highest).
///
/// Bucket 0: Completes a K-to-A sequence
/// Bucket 1: Reveals a face-down thoughtful target
/// Bucket 2: Reveals any face-down card
/// Bucket 3: Improves suit run (consolidation)
/// Bucket 4: Move to a same-suit column
/// Bucket 5: Generic move between columns
/// Bucket 6: Deal from stock
fn move_bucket(
    transition: &SpiderTransition,
    from: &SpiderBoard,
    from_suit_run_total: usize,
    targets: &[Card],
) -> u8 {
    let the_move = &transition.the_move;
    let to = &transition.to_board;

    // Bucket 0: Completes K-to-A.
    if to.completed_sets > from.completed_sets {
        return 0;
    }

    // Bucket 6: Deal from stock.
    if the_move.is_deal_from_stock() {
        return 6;
    }

    if let SpiderMove::ColumnToColumn {
        source,
        destination,
        card_count,
    } = the_move
    {
        let src_col = &from.columns[*source];

        // Reveals a face-down card?
        let exposes_fd = *card_count == src_col.num_face_up() && src_col.has_face_down();

        if exposes_fd {
            let exposed = src_col.face_down.last().unwrap();
            if is_target(exposed, targets) {
                return 1; // Bucket 1: Reveals target.
            }
            return 2; // Bucket 2: Reveals any face-down card.
        }

        // Improves total suit run?
        let to_suit_run: usize = to.columns.iter().map(|c| c.longest_run).sum();
        if to_suit_run > from_suit_run_total {
            return 3; // Bucket 3: Consolidation.
        }

        // Same-suit move?
        if let Some(top_of_dest) = from.columns[*destination].top_card() {
            let bottom_idx = src_col.face_up.len() - card_count;
            let first_card = &src_col.face_up[bottom_idx];
            if top_of_dest.suit == first_card.suit {
                return 4; // Bucket 4: Suit affinity.
            }
        }

        return 5; // Bucket 5: Generic.
    }

    6
}

/// Priority score with thoughtful signals (targets, critical path).
/// `suit_count` allows signals to scale by variant.
fn local_priority(
    transition: &SpiderTransition,
    from: &SpiderBoard,
    from_face_down: usize,
    from_suit_run_total: usize,
    targets: &[Card],
    suit_count: u32,
) -> i64 {
    let the_move = &transition.the_move;
    let to = &transition.to_board;

    // Completing a K-to-A sequence: maximum priority.
    let completed_diff = to.completed_sets as i64 - from.completed_sets as i64;
    let completed_bonus = completed_diff * 5000;

    if completed_bonus > 0 {
        return 10000 + completed_bonus;
    }

    // Deal from stock: low priority (last resort).
    if the_move.is_deal_from_stock() {
        return 10;
    }

    let mut priority: i64 = 0;

    // Bonus for revealing face-down cards.
    let after_face_down = to.total_face_down() as i64;
    let exposed_bonus = (from_face_down as i64 - after_face_down).max(0) * 200;
    priority += exposed_bonus;

    // Bonus for consolidating same-suit runs.
    let after_suit_run_total: i64 = to.columns.iter().map(|c| c.longest_run as i64).sum();
    let consolidation_bonus = (after_suit_run_total - from_suit_run_total as i64).max(0) * 100;
    priority += consolidation_bonus;

    if let SpiderMove::ColumnToColumn {
        source,
        destination,
        card_count,
    } = the_move
    {
        let src_col = &from.columns[*source];

        // Bonus for moving onto the same suit (suit affinity), scaled by variant.
        if let Some(top_of_dest) = from.columns[*destination].top_card() {
            let bottom_idx = src_col.face_up.len() - card_count;
            let first_card = &src_col.face_up[bottom_idx];
            if top_of_dest.suit == first_card.suit {
                priority += weights::same_suit_affinity(suit_count);
            }
        }

        // Penalty for breaking an existing suit run, scaled by variant.
        let src_col_after = &to.columns[*source];
        if src_col.longest_run > src_col_after.longest_run + card_count {
            priority -= weights::break_run_penalty(suit_count);
        }

        // 2.1: Bonus for revealing a target (thoughtful signal).
        // If this move removes all face_up cards from the source column
        // and there are face_down cards, the revealed card is the last face_down card.
        if *card_count == src_col.num_face_up() && src_col.has_face_down() {
            let exposed = src_col.face_down.last().unwrap();
            if is_target(exposed, targets) {
                priority += weights::O_TARGET_REVEAL_BONUS;
            } else {
                // Smaller bonus if the revealed card is 1-2 values away from a target.
                for target in targets {
                    if exposed.suit == target.suit {
                        let diff = (exposed.value as i64 - target.value as i64).unsigned_abs();
                        if diff >= 1 && diff <= 2 {
                            priority += weights::O_NEAR_TARGET_BONUS;
                            break;
                        }
                    }
                }
            }
            // Revealed King with an empty column available: very valuable.
            if exposed.value == 13 && from.has_empty_column() {
                priority += weights::O_KING_EMPTY_COL_BONUS;
            }
        }

        // 2.3: Critical path: moving cards from a column with a buried target
        // reduces the stack above the target, bringing it closer to being revealed.
        if src_col.face_down.iter().any(|c| is_target(c, targets)) {
            priority += weights::O_CRITICAL_PATH_BONUS;
        }
    }

    // Minimum base for moves between columns.
    priority += 50;

    priority
}

/// Deterministic local_priority perturbation based on attempt and board signature.
/// attempt=0 -> no perturbation (original ordering).
/// attempt>0 -> adds noise in [0, range) to reorder moves within the same bucket
/// without changing the structure between buckets.
fn move_perturbation(attempt: u64, board_sig: u64, the_move: &SpiderMove, suit_count: u32) -> i64 {
    if attempt == 0 {
        return 0;
    }
    let move_hash: u64 = match the_move {
        SpiderMove::ColumnToColumn {
            source,
            destination,
            card_count,
        } => (*source as u64)
            .wrapping_mul(17)
            .wrapping_add(*destination as u64)
            .wrapping_mul(31)
            .wrapping_add(*card_count as u64),
        SpiderMove::DealFromStock => 0xDEAD,
        SpiderMove::Deal { .. } => 0,
    };
    // Scale perturbation with attempt number:
    // early attempts (1-9): mild perturbation (1x)
    // middle attempts (10-19): moderate perturbation (2x)
    // late attempts (20+): aggressive perturbation (3x)
    let scale: u64 = if attempt <= 9 {
        1
    } else if attempt <= 19 {
        2
    } else {
        3
    };
    let range = weights::perturbation_range(suit_count) * scale;
    let h = board_sig
        .wrapping_mul(2654435761) // Knuth multiplicative hash
        .wrapping_add(move_hash)
        .wrapping_mul(attempt.wrapping_mul(6364136223846793005).wrapping_add(1));
    (h % range) as i64
}

/// Checks whether a card matches any target (same suit and value).
#[inline]
fn is_target(card: &Card, targets: &[Card]) -> bool {
    targets
        .iter()
        .any(|t| t.suit == card.suit && t.value == card.value)
}

/// Deals a deck onto a Spider board.
pub fn deal(deck: &[crate::common::card::Card], num_columns: usize) -> Option<SpiderBoard> {
    let deal_move = SpiderMove::Deal {
        deck: deck.to_vec(),
        num_columns,
    };
    deal_move.apply(&SpiderBoard::new(vec![], vec![], 0))
}
