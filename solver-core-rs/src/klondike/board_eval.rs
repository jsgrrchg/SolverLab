//! Unified Klondike board evaluation and checkpoint policy.
//!
//! `BoardEval` computes all board metrics once, and exposes both
//! `heuristic_cost()` and `progress_score()` consistently.
//!
//! `CheckpointPolicy` encapsulates when to checkpoint and when to adopt.

use super::board::KlondikeBoard;
use super::weights::*;
use crate::common::card::Card;

// ═══════════════════════════════════════════
// BoardEval
// ═══════════════════════════════════════════

/// Complete evaluation of a board state.
/// Computed once and queried for heuristic cost and progress.
pub struct BoardEval {
    pub foundation_count: i64,
    pub face_up: i64,
    pub hidden: i64,
    pub empty_cols: i64,
    pub blocked_kings: i64,
    pub stock_remaining: i64,
    pub waste_count: i64,
    pub stock_inaccessible: i64,
    pub foundation_imbalance: i64, // max_rank - min_rank (raw, no tolerance)
    pub depth_penalty: i64,        // sum of fd + (fd-1)/DIVISOR per column
    pub stock_advance_cost: i64,   // minimum advances needed to access the stock
    pub target_burial_penalty: i64, // penalty for face-down target cards
    pub deadlock_count: i64,       // number of detected logical deadlocks
    pub reveal_bonus: i64,         // bonus for exposing a key card
    pub stranded_blockers: i64,    // blockers above targets with no destination
    pub stock_target_penalty: i64, // penalty for targets trapped in stock
}

impl BoardEval {
    /// Fast constructor: only base metrics needed for heuristic_cost.
    /// Used in the IDA* hot path (search_with_bound), where performance is critical.
    /// Does NOT compute thoughtful fields (target_burial, deadlock, reveal_bonus).
    pub fn from_board_fast(board: &KlondikeBoard, draw_advance: u8) -> Self {
        let mut depth_penalty = 0i64;
        let mut hidden = 0i64;
        let mut face_up = 0i64;
        let mut empty_cols = 0i64;

        for c in &board.columns {
            let fd = c.num_face_down() as i64;
            hidden += fd;
            depth_penalty += fd + fd.saturating_sub(1) / HIDDEN_DEPTH_DIVISOR;
            face_up += c.num_face_up() as i64;
            if c.len == 0 {
                empty_cols += 1;
            }
        }

        // Blocked Kings: only when there are no empty columns.
        let blocked_kings = if empty_cols == 0 {
            board
                .columns
                .iter()
                .filter(|c| {
                    c.has_face_down()
                        && c.has_face_up()
                        && c.face_up_cards()
                            .first()
                            .map(|card| card.value == 13)
                            .unwrap_or(false)
                })
                .count() as i64
        } else {
            0
        };

        // Foundation imbalance
        let mut min_rank: i64 = 13;
        let mut max_rank: i64 = 0;
        for &rank in &board.foundation {
            let r = rank as i64;
            if r < min_rank {
                min_rank = r;
            }
            if r > max_rank {
                max_rank = r;
            }
        }

        let foundation_count = board.total_foundation_count() as i64;
        let stock_remaining = board.stock_len.saturating_sub(board.stock_index) as i64;
        let waste_count = board.stock_index as i64;

        // Inaccessible stock: restored global formula.
        // With Draw-1 all cards are sequentially accessible.
        // With Draw-3 only ceil(remaining/3) are directly accessible.
        let da = (draw_advance as i64).max(1);
        let stock_inaccessible = if stock_remaining > 0 && da > 1 {
            let accessible = (stock_remaining + da - 1) / da;
            stock_remaining.saturating_sub(accessible)
        } else {
            0
        };

        // Stock advance cost: minimum number of advance moves needed to make
        // each remaining stock card accessible.
        // These are non-foundation moves, added to the base 52-fc.
        let stock_advance_cost = if stock_remaining > 0 {
            (stock_remaining + da - 1) / da
        } else {
            0
        };

        Self {
            foundation_count,
            face_up,
            hidden,
            empty_cols,
            blocked_kings,
            stock_remaining,
            waste_count,
            stock_inaccessible,
            foundation_imbalance: max_rank - min_rank,
            depth_penalty,
            stock_advance_cost,
            target_burial_penalty: 0,
            deadlock_count: 0,
            reveal_bonus: 0,
            stranded_blockers: 0,
            stock_target_penalty: 0,
        }
    }

    /// Full constructor: computes all metrics, including thoughtful signals.
    /// Used for progress_score in consider_progress and checkpoint adoption.
    pub fn from_board(board: &KlondikeBoard, draw_advance: u8) -> Self {
        let mut eval = Self::from_board_fast(board, draw_advance);

        // Compute thoughtful metrics only for progress_score.
        let mut target_cards = [255u8; 4];
        for suit in 0..4 {
            let required_val = board.foundation[suit] + 1;
            if required_val <= 13 {
                target_cards[suit] = required_val;
            }
        }

        for c in &board.columns {
            if c.face_down_len == 0 {
                continue;
            }

            // For deadlock detection: the lowest value seen for each suit,
            // iterating from surface to depth.
            let mut lowest_ranks = [255i32; 4];

            for i in (0..c.face_down_len).rev() {
                let card = c.cards[i as usize];
                let card_suit = card.suit as usize;
                let card_val = card.value;

                // Target depth penalty: buried target cards.
                if card_val == target_cards[card_suit] {
                    let obstacle_count = (c.len - i) as i64;
                    eval.target_burial_penalty += obstacle_count * BURIED_TARGET_PENALTY;
                }

                // Deadlock detection: rank inversion in the same suit.
                if lowest_ranks[card_suit] < card_val as i32 {
                    let severity = (card_val as i32 - lowest_ranks[card_suit]).min(3) as i64;
                    eval.deadlock_count += severity;
                }
                if (card_val as i32) < lowest_ranks[card_suit] {
                    lowest_ranks[card_suit] = card_val as i32;
                }
            }

            // Reveal value reward (thoughtful: the face-down card is known).
            if c.face_down_len > 0 {
                let would_reveal = c.cards[(c.face_down_len - 1) as usize];
                if would_reveal.value == 13 && eval.empty_cols > 0 {
                    eval.reveal_bonus += REVEALED_KING_BONUS;
                } else if would_reveal.value == target_cards[would_reveal.suit as usize] {
                    eval.reveal_bonus += REVEALED_FOUNDATION_CARD_BONUS;
                }
            }
        }

        // ── Blocker Viability Analysis (Deep Thoughtful) ────────────
        // For each buried target, check whether blockers above it have a visible
        // destination. A "stranded" blocker (no destination in the current state)
        // indicates that uncovering that target will be costly.
        let mut stranded_blockers = 0i64;
        for suit in 0..4usize {
            if target_cards[suit] == 255 {
                continue;
            }
            let target_val = target_cards[suit];

            for col_idx in 0..7usize {
                let col = &board.columns[col_idx];
                let mut found = false;

                for pos in 0..(col.face_down_len as usize) {
                    let card = col.cards[pos];
                    if card.suit as usize == suit && card.value == target_val {
                        // Target found at face-down position `pos`.
                        // Check face-down blockers above it (pos+1..face_down_len).
                        for bp in (pos + 1)..(col.face_down_len as usize) {
                            let blocker = col.cards[bp];
                            if !Self::card_has_column_destination(board, blocker, col_idx) {
                                stranded_blockers += 1;
                            }
                        }
                        // Check whether the face-up stack can move.
                        // The bottom of the face-up run determines whether the whole pile can move.
                        if col.num_face_up() > 0 {
                            let bottom_fu = col.cards[col.face_down_len as usize];
                            if !Self::card_has_column_destination(board, bottom_fu, col_idx) {
                                stranded_blockers += 1;
                            }
                        }
                        found = true;
                        break;
                    }
                }
                if found {
                    break; // Target for this suit already found.
                }
            }
        }
        eval.stranded_blockers = stranded_blockers;

        // ── Target Accessibility in Stock (Deep Thoughtful) ────────────
        // For each target in the stock (not in columns), evaluate how accessible
        // it is given the current position and draw_advance.
        let mut stock_target_penalty = 0i64;
        let da = draw_advance as usize;
        let si = board.stock_index as usize;

        'suit_loop: for suit in 0..4usize {
            if target_cards[suit] == 255 {
                continue;
            }
            let target_val = target_cards[suit];

            // First check whether the target is in any column.
            for col in &board.columns {
                for pos in 0..(col.len as usize) {
                    if col.cards[pos].suit as usize == suit && col.cards[pos].value == target_val {
                        continue 'suit_loop; // In a column: blocker analysis handles it.
                    }
                }
            }

            // Target is not in columns; search stock.
            for pos in 0..(board.stock_len as usize) {
                let card = board.stock[pos];
                if card.suit as usize == suit && card.value == target_val {
                    if si > 0 && pos == si - 1 {
                        // It is the current pile card: accessible now, no penalty.
                    } else if pos < si {
                        // In waste (already passed): requires recycle.
                        stock_target_penalty += STOCK_WASTE_TARGET_PENALTY as i64;
                    } else if da > 1 {
                        // In remaining stock: check alignment with draw_advance.
                        let offset = pos - si + 1;
                        if offset % da != 0 {
                            // Misaligned: not accessible in this pass.
                            stock_target_penalty += STOCK_MISALIGNED_TARGET_PENALTY as i64;
                        }
                    }
                    break;
                }
            }
        }
        eval.stock_target_penalty = stock_target_penalty;

        eval
    }

    /// Heuristic cost for IDA* (scale ~0-80+, lower is better).
    /// Maintains admissibility: never overestimates the real cost.
    /// Thoughtful signals are NOT included here to preserve admissibility.
    pub fn heuristic_cost(&self) -> i64 {
        // Auto-play detection: if there are no hidden cards and no stock, the game is won.
        if self.hidden == 0 && self.stock_remaining == 0 && self.waste_count == 0 {
            return 0;
        }

        // Improved endgame: with no hidden cards, the game is almost auto-won.
        // Only remaining foundation moves plus stock advances are needed.
        if self.hidden == 0 {
            return 52 - self.foundation_count + self.stock_advance_cost / STOCK_ADVANCE_DIVISOR;
        }

        // Base: cards missing from foundations.
        let mut cost = 52 - self.foundation_count;

        // Burial depth.
        cost += self.depth_penalty;

        // Mobility: empty-column bonus with diminishing returns.
        let idx = (self.empty_cols as usize).min(EMPTY_COL_BONUS.len() - 1);
        cost -= EMPTY_COL_BONUS[idx];

        // Blocked Kings.
        cost += self.blocked_kings * BLOCKED_KING_PENALTY;

        // Waste exhaustion.
        cost += self.waste_penalty();

        // Foundation imbalance.
        let excess = self.foundation_imbalance - IMBALANCE_TOLERANCE;
        if excess > 0 {
            cost += excess * IMBALANCE_WEIGHT;
        }

        // Stock advance cost.
        cost += self.stock_advance_cost / STOCK_ADVANCE_DIVISOR;

        // Do NOT add target_burial_penalty or deadlock_count here.
        // Those signals belong ONLY in progress_score and successor_ordering.

        cost
    }

    /// Progress score for checkpoints (higher is better).
    /// Incorporates thoughtful signals: the solver penalizes checkpoints that
    /// keep targets buried or deadlocks active.
    pub fn progress_score(&self) -> i64 {
        let mut score = self.foundation_count * PROGRESS_FOUNDATION_WEIGHT
            + self.face_up * PROGRESS_FACE_UP_WEIGHT
            - self.hidden * PROGRESS_HIDDEN_PENALTY
            + self.empty_cols * PROGRESS_EMPTY_COL_BONUS
            - self.blocked_kings * PROGRESS_BLOCKED_KINGS_PENALTY
            - self.waste_penalty() * PROGRESS_WASTE_MULTIPLIER
            + self.reveal_bonus
            - self.target_burial_penalty * PROGRESS_BURIAL_WEIGHT
            - self.deadlock_count * PROGRESS_DEADLOCK_WEIGHT
            - self.stranded_blockers * PROGRESS_STRANDED_BLOCKER_PENALTY
            - self.stock_target_penalty;

        // Near-autoplay bonus: states with few hidden cards are close to
        // auto-play (hidden==0 -> essentially won).
        // Quadratic bonus that grows quickly as hidden approaches 0.
        if self.hidden > 0 && self.hidden <= AUTOPLAY_PROXIMITY_THRESHOLD {
            let proximity = (AUTOPLAY_PROXIMITY_THRESHOLD + 1 - self.hidden) as i64;
            score += proximity * proximity * PROGRESS_AUTOPLAY_PROXIMITY;
        }

        score
    }

    /// Checks whether a card has a valid destination in any tableau column.
    /// Used by blocker viability analysis to determine whether a blocker can move
    /// out of the way of a buried target.
    fn card_has_column_destination(board: &KlondikeBoard, card: Card, exclude_col: usize) -> bool {
        if card.value == 13 {
            // King: needs an empty column.
            return board
                .columns
                .iter()
                .enumerate()
                .any(|(i, c)| i != exclude_col && c.len == 0);
        }
        for (i, col) in board.columns.iter().enumerate() {
            if i == exclude_col {
                continue;
            }
            if let Some(top) = col.top_face_up() {
                if top.value == card.value + 1 && top.color() != card.color() {
                    return true;
                }
            }
        }
        false
    }

    /// Waste penalty shared by heuristic_cost and progress_score.
    fn waste_penalty(&self) -> i64 {
        let mut p = 0;
        if self.stock_remaining <= WASTE_THRESHOLD_STOCK
            && self.waste_count >= WASTE_THRESHOLD_COUNT
        {
            p += 1 + (self.waste_count - WASTE_THRESHOLD_COUNT) / WASTE_SCALE_DIVISOR;
        }
        if self.stock_remaining == 0 && self.waste_count > 0 {
            p += self.waste_count / WASTE_EMPTY_STOCK_DIVISOR;
        }
        p
    }
}

// ═══════════════════════════════════════════
// CheckpointPolicy
// ═══════════════════════════════════════════

/// Checkpoint policy: encapsulates all decisions about when to checkpoint,
/// when to adopt, and when to give up.
pub struct CheckpointPolicy {
    pub max_adoptions: u32,
    pub base_limits: [u64; 7],
}

impl CheckpointPolicy {
    /// Default policy: the values currently used by the solver.
    pub fn default_policy() -> Self {
        Self {
            max_adoptions: MAX_CHECKPOINTS,
            base_limits: CHECKPOINT_BASE_LIMITS,
        }
    }

    /// Calculates the base node_limit from board difficulty.
    pub fn base_limit_for(&self, board: &KlondikeBoard) -> u64 {
        let fc = board.total_foundation_count();
        let hidden: usize = board.columns.iter().map(|c| c.num_face_down()).sum();

        if hidden > 18 {
            self.base_limits[5]
        } else if hidden > 15 {
            self.base_limits[4]
        } else if hidden > 12 {
            self.base_limits[3]
        } else if hidden > 9 {
            self.base_limits[2]
        } else if hidden > 6 {
            self.base_limits[1]
        } else if fc > 30 {
            self.base_limits[6]
        } else {
            self.base_limits[0]
        }
    }
    /// Decides whether a progress delta is adoptable given the current phase.
    /// `foundation_count`: real number of cards in foundations (not derived from score).
    pub fn is_adoptable(&self, start_score: i64, best_score: i64, foundation_count: usize) -> bool {
        let delta = best_score - start_score;

        let fc = foundation_count as i64;
        let (clamp_min, clamp_max) = if fc < ADOPT_EARLY_THRESHOLD_FC {
            ADOPT_EARLY_CLAMP
        } else if fc < ADOPT_MID_THRESHOLD_FC {
            ADOPT_MID_CLAMP
        } else {
            ADOPT_LATE_CLAMP
        };
        let relative_threshold = (start_score.saturating_abs() * ADOPT_RELATIVE_PCT) / 100;
        let adopt_threshold = relative_threshold.clamp(clamp_min, clamp_max);
        delta >= adopt_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::card::Suit;

    fn card(suit: Suit, value: u8) -> Card {
        Card::new(suit, value)
    }

    fn board_with_hidden_counts(hidden_counts: [u8; 7]) -> KlondikeBoard {
        let mut board = KlondikeBoard::new();
        for (idx, hidden) in hidden_counts.into_iter().enumerate() {
            for n in 0..hidden {
                board.columns[idx].push(card(Suit::Club, n + 1), true);
            }
            board.columns[idx].push(card(Suit::Heart, 13 - idx as u8), false);
        }
        board.compute_signature();
        board
    }

    fn set_stock(board: &mut KlondikeBoard, cards: &[Card], stock_index: u8) {
        for (idx, card) in cards.iter().enumerate() {
            board.stock[idx] = *card;
        }
        board.stock_len = cards.len() as u8;
        board.stock_index = stock_index;
        board.compute_signature();
    }

    #[test]
    fn from_board_fast_calculates_tableau_stock_and_foundation_metrics() {
        let mut board = KlondikeBoard::new();
        board.columns[0].push(card(Suit::Club, 9), true);
        board.columns[0].push(card(Suit::Heart, 13), false);
        board.columns[1].push(card(Suit::Spade, 7), false);
        board.foundation = [1, 2, 0, 0];
        set_stock(
            &mut board,
            &[
                card(Suit::Club, 3),
                card(Suit::Diamond, 4),
                card(Suit::Heart, 5),
                card(Suit::Spade, 6),
            ],
            1,
        );

        let eval = BoardEval::from_board_fast(&board, 1);

        assert_eq!(eval.foundation_count, 3);
        assert_eq!(eval.hidden, 1);
        assert_eq!(eval.face_up, 2);
        assert_eq!(eval.empty_cols, 5);
        assert_eq!(eval.stock_remaining, 3);
        assert_eq!(eval.waste_count, 1);
        assert_eq!(eval.depth_penalty, 1);
        assert_eq!(eval.foundation_imbalance, 2);
    }

    #[test]
    fn draw_one_vs_draw_three_stock_accessibility_metrics() {
        let mut board = KlondikeBoard::new();
        set_stock(
            &mut board,
            &[
                card(Suit::Club, 1),
                card(Suit::Club, 2),
                card(Suit::Club, 3),
                card(Suit::Club, 4),
                card(Suit::Club, 5),
                card(Suit::Club, 6),
                card(Suit::Club, 7),
            ],
            1,
        );

        let draw_one = BoardEval::from_board_fast(&board, 1);
        let draw_three = BoardEval::from_board_fast(&board, 3);

        assert_eq!(draw_one.stock_remaining, 6);
        assert_eq!(draw_one.stock_inaccessible, 0);
        assert_eq!(draw_one.stock_advance_cost, 6);
        assert_eq!(draw_three.stock_inaccessible, 4);
        assert_eq!(draw_three.stock_advance_cost, 2);
    }

    #[test]
    fn blocked_kings_only_count_when_no_empty_columns() {
        let mut board = board_with_hidden_counts([1, 0, 0, 0, 0, 0, 0]);
        board.columns[0].cards[board.columns[0].face_down_len as usize] = card(Suit::Heart, 13);
        board.columns[6] = super::super::board::FastColumn::empty();

        assert_eq!(BoardEval::from_board_fast(&board, 1).blocked_kings, 0);

        board.columns[6].push(card(Suit::Club, 5), false);
        assert_eq!(BoardEval::from_board_fast(&board, 1).blocked_kings, 1);
    }

    #[test]
    fn from_board_covers_buried_target_deadlock_reveal_and_stock_target_penalty() {
        let mut board = KlondikeBoard::new();
        board.foundation = [0, 0, 0, 0];
        board.columns[0].push(card(Suit::Club, 4), true);
        board.columns[0].push(card(Suit::Club, 1), true);
        board.columns[0].push(card(Suit::Heart, 9), false);
        set_stock(
            &mut board,
            &[
                card(Suit::Diamond, 2),
                card(Suit::Diamond, 1),
                card(Suit::Heart, 1),
                card(Suit::Spade, 1),
            ],
            0,
        );

        let eval = BoardEval::from_board(&board, 3);

        assert!(eval.target_burial_penalty > 0);
        assert!(eval.deadlock_count > 0);
        assert_eq!(eval.reveal_bonus, REVEALED_FOUNDATION_CARD_BONUS);
        assert!(eval.stranded_blockers > 0);
        assert!(eval.stock_target_penalty >= STOCK_MISALIGNED_TARGET_PENALTY);
    }

    #[test]
    fn checkpoint_policy_base_limit_for_hidden_and_endgame_ranges() {
        let policy = CheckpointPolicy {
            max_adoptions: 1,
            base_limits: [10, 20, 30, 40, 50, 60, 70],
        };

        let hidden_0 = board_with_hidden_counts([0, 0, 0, 0, 0, 0, 0]);
        let hidden_7 = board_with_hidden_counts([1, 1, 1, 1, 1, 1, 1]);
        let hidden_10 = board_with_hidden_counts([2, 2, 2, 1, 1, 1, 1]);
        let hidden_13 = board_with_hidden_counts([2, 2, 2, 2, 2, 2, 1]);
        let hidden_16 = board_with_hidden_counts([3, 3, 2, 2, 2, 2, 2]);
        let hidden_19 = board_with_hidden_counts([3, 3, 3, 3, 3, 2, 2]);
        let mut endgame = hidden_0;
        endgame.foundation = [8, 8, 8, 7];

        assert_eq!(policy.base_limit_for(&hidden_0), 10);
        assert_eq!(policy.base_limit_for(&hidden_7), 20);
        assert_eq!(policy.base_limit_for(&hidden_10), 30);
        assert_eq!(policy.base_limit_for(&hidden_13), 40);
        assert_eq!(policy.base_limit_for(&hidden_16), 50);
        assert_eq!(policy.base_limit_for(&hidden_19), 60);
        assert_eq!(policy.base_limit_for(&endgame), 70);
    }

    #[test]
    fn checkpoint_policy_is_adoptable_uses_phase_thresholds() {
        let policy = CheckpointPolicy::default_policy();

        assert!(!policy.is_adoptable(1_000, 1_100, 0));
        assert!(policy.is_adoptable(1_000, 1_150, 0));

        assert!(!policy.is_adoptable(1_000, 1_090, 10));
        assert!(policy.is_adoptable(1_000, 1_100, 10));

        assert!(!policy.is_adoptable(100, 119, 30));
        assert!(policy.is_adoptable(100, 120, 30));
    }
}
