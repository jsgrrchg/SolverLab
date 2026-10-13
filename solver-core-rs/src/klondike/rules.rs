use super::board::KlondikeBoard;
use super::moves::KlondikeMove;
use crate::common::card::Card;

// ═══════════════════════════════════════════
// Board transition
// ═══════════════════════════════════════════

/// Represents the step from one state to another during IDA* search.
/// Contains the destination board, the applied move, the previous move
/// (to detect immediate undos), and the current depth.
/// The source board (from_board) is kept by the caller and passed by reference
/// to functions that need it, avoiding ~376 bytes of redundant copy per transition.
pub struct KlondikeTransition {
    pub to_board: KlondikeBoard,
    pub the_move: KlondikeMove,
    pub previous_move: Option<KlondikeMove>,
    pub depth: usize,
}

// ═══════════════════════════════════════════
// Pruning rules
// ═══════════════════════════════════════════

/// Pruning rules applied by the solver to each transition before exploring it.
/// If any rule returns `should_prune = true`, the move is discarded without
/// descending into the search branch.
#[derive(Debug, Clone)]
pub enum KlondikeRule {
    /// Discards moves that exceed the maximum allowed depth.
    DepthLimit { max_depth: usize },

    /// Discards immediate inverses unless the previous move may have revealed a card.
    /// Ambiguous cases are left to the search's path-signature cycle check.
    NoImmediateUndo,

    /// Discards moves where the resulting board is identical to the previous one.
    /// Protects against moves that do not change state (e.g., stock advance with
    /// an empty stock, which the engine should not generate, but is checked for safety).
    NoopTransition,

    /// Only allows moving a King to an empty column if the move exposes at least
    /// one face-down card in the source column.
    /// Prevents unproductive King moves to empty columns.
    KingToEmptyMustExpose,

    /// Only allows returning a card from a foundation to the tableau when some
    /// visible card could then be placed on it: a face-up card in another
    /// column, or the current waste card, of rank - 1 and opposite color.
    /// Aces never come back. Without a receiver the rollback only spends nodes
    /// and undos. (The previous face-up count comparison never pruned: the
    /// returned card itself always added one face-up card.)
    FoundationRollbackNeedsReceiver,
}

impl KlondikeRule {
    /// Evaluates whether a move can be pruned BEFORE calling `apply()`.
    /// Only evaluates rules that do not need the resulting board, avoiding
    /// the ~376-byte copy + compute_signature when pruning is safe.
    pub fn can_prune_early(
        &self,
        board: &KlondikeBoard,
        the_move: KlondikeMove,
        previous_move: Option<KlondikeMove>,
        depth: usize,
    ) -> bool {
        match self {
            KlondikeRule::DepthLimit { max_depth } => depth > *max_depth,

            KlondikeRule::NoImmediateUndo => {
                let prev = match previous_move {
                    Some(p) => p,
                    None => return false,
                };
                match (prev, the_move) {
                    (
                        KlondikeMove::ColumnToColumn {
                            source: a,
                            destination: b,
                            count: c1,
                        },
                        KlondikeMove::ColumnToColumn {
                            source: c,
                            destination: d,
                            count: c2,
                        },
                    ) => {
                        // A reveal leaves exactly one face-up card in the source.
                        // One visible card can also remain without a reveal; path
                        // signatures reject those genuine cycles after apply().
                        a == d && b == c && c1 == c2 && board.columns[a as usize].num_face_up() != 1
                    }
                    (
                        KlondikeMove::ColumnToFoundation { source, card },
                        KlondikeMove::FoundationToColumn {
                            destination,
                            card: same_card,
                        },
                    ) => {
                        source == destination
                            && card == same_card
                            && board.columns[source as usize].num_face_up() != 1
                    }
                    (
                        KlondikeMove::FoundationToColumn { destination, card },
                        KlondikeMove::ColumnToFoundation {
                            source,
                            card: same_card,
                        },
                    ) => source == destination && card == same_card,
                    _ => false,
                }
            }

            KlondikeRule::KingToEmptyMustExpose => {
                let (source, destination, count) = match the_move {
                    KlondikeMove::ColumnToColumn {
                        source,
                        destination,
                        count,
                    } => (source, destination, count),
                    _ => return false,
                };
                let col = &board.columns[source as usize];
                if count == 0 || count > col.num_face_up() as u8 || count > col.len {
                    return false;
                }
                if col.len == 0 {
                    return false;
                }
                let run_start = col.len - count;
                if col.cards[run_start as usize].value != 13 {
                    return false;
                }
                let to_col_before = &board.columns[destination as usize];
                if to_col_before.has_face_up() {
                    return false;
                }
                let source_len_after = col.len - count;
                let exposes_hidden = source_len_after > 0 && source_len_after == col.face_down_len;
                !exposes_hidden
            }

            KlondikeRule::FoundationRollbackNeedsReceiver => {
                let (destination, card) = match the_move {
                    KlondikeMove::FoundationToColumn { destination, card } => (destination, card),
                    _ => return false,
                };
                if card.value <= 1 {
                    return true;
                }
                let fits = |c: Card| c.value == card.value - 1 && c.color() != card.color();
                let in_tableau = board
                    .columns
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != destination as usize)
                    .any(|(_, column)| column.face_up_cards().iter().any(|&c| fits(c)));
                let in_waste = board.stock_pile_card().is_some_and(fits);
                !(in_tableau || in_waste)
            }

            _ => false,
        }
    }

    /// Rule set for Fast mode: includes all heuristic pruning rules
    /// (`KingToEmptyMustExpose`, `FoundationRollbackNeedsReceiver`) that
    /// aggressively reduce the search space at the cost of not exploring some
    /// uncommon paths.
    pub fn default_rules_fast(max_depth: usize) -> Vec<KlondikeRule> {
        vec![
            KlondikeRule::DepthLimit { max_depth },
            KlondikeRule::NoImmediateUndo,
            KlondikeRule::NoopTransition,
            KlondikeRule::KingToEmptyMustExpose,
            KlondikeRule::FoundationRollbackNeedsReceiver,
        ]
    }

    /// Rule set for Strict mode: only includes safe pruning rules
    /// (depth limit, immediate undo, noop). Slower but complete: it does not
    /// discard potentially valid branches.
    pub fn default_rules_strict(max_depth: usize) -> Vec<KlondikeRule> {
        vec![
            KlondikeRule::DepthLimit { max_depth },
            KlondikeRule::NoImmediateUndo,
            KlondikeRule::NoopTransition,
        ]
    }

    /// Evaluates whether a transition should be pruned by this rule.
    /// Returns `true` if the move should be discarded.
    ///
    /// Note: `DepthLimit`, `NoImmediateUndo`, `KingToEmptyMustExpose` and
    /// `FoundationRollbackNeedsReceiver` are evaluated in `can_prune_early()`
    /// before `apply()`. They return `false` here.
    pub fn should_prune(
        &self,
        transition: &KlondikeTransition,
        from_board: &KlondikeBoard,
    ) -> bool {
        match self {
            // Evaluated in can_prune_early; they do not reach this point.
            KlondikeRule::DepthLimit { .. }
            | KlondikeRule::NoImmediateUndo
            | KlondikeRule::KingToEmptyMustExpose
            | KlondikeRule::FoundationRollbackNeedsReceiver => false,

            // Noop pruning: the board did not change after the move.
            KlondikeRule::NoopTransition => *from_board == transition.to_board,
        }
    }
}

// ═══════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::card::{Card, Suit};

    fn has_foundation_rollback(rules: &[KlondikeRule]) -> bool {
        rules
            .iter()
            .any(|r| matches!(r, KlondikeRule::FoundationRollbackNeedsReceiver))
    }

    fn has_king_to_empty_must_expose(rules: &[KlondikeRule]) -> bool {
        rules
            .iter()
            .any(|r| matches!(r, KlondikeRule::KingToEmptyMustExpose))
    }

    #[test]
    fn immediate_inverse_is_still_pruned_when_the_source_cannot_have_revealed() {
        let rule = KlondikeRule::NoImmediateUndo;
        for remaining_face_up in [0, 2] {
            let moved = Card::new(Suit::Heart, if remaining_face_up == 0 { 13 } else { 7 });
            let mut board = KlondikeBoard::new();
            if remaining_face_up == 2 {
                board.columns[0].push(Card::new(Suit::Diamond, 9), false);
                board.columns[0].push(Card::new(Suit::Club, 8), false);
            }
            board.columns[0].push(moved, false);
            if remaining_face_up == 2 {
                board.columns[1].push(Card::new(Suit::Spade, 8), false);
            }
            board.foundation[Suit::Heart as usize] = moved.value - 1;
            board.compute_signature();

            for (forward, inverse) in [
                (
                    KlondikeMove::ColumnToColumn {
                        source: 0,
                        destination: 1,
                        count: 1,
                    },
                    KlondikeMove::ColumnToColumn {
                        source: 1,
                        destination: 0,
                        count: 1,
                    },
                ),
                (
                    KlondikeMove::ColumnToFoundation {
                        source: 0,
                        card: moved,
                    },
                    KlondikeMove::FoundationToColumn {
                        destination: 0,
                        card: moved,
                    },
                ),
            ] {
                let after = forward.apply(board).unwrap();
                assert_eq!(after.columns[0].num_face_up(), remaining_face_up);
                assert!(after.columns[0].can_add_run(moved));
                assert!(rule.can_prune_early(&after, inverse, Some(forward), 2));
                assert!(!rule.can_prune_early(&after, inverse, None, 2));
                assert_eq!(inverse.apply(after).unwrap().signature, board.signature);
            }
        }
    }

    /// Verifies that Fast includes the heuristic rules and Strict does not.
    #[test]
    fn fast_vs_strict_rule_sets_differ_on_heuristic_pruning() {
        let fast = KlondikeRule::default_rules_fast(100);
        let strict = KlondikeRule::default_rules_strict(100);

        assert!(has_foundation_rollback(&fast));
        assert!(has_king_to_empty_must_expose(&fast));

        assert!(!has_foundation_rollback(&strict));
        assert!(!has_king_to_empty_must_expose(&strict));
    }

    /// A foundation rollback is only worth exploring when a visible card could
    /// be placed on the returned card.
    #[test]
    fn foundation_rollback_requires_a_visible_receiver() {
        let rule = KlondikeRule::FoundationRollbackNeedsReceiver;
        let rollback = KlondikeMove::FoundationToColumn {
            destination: 0,
            card: Card::new(Suit::Heart, 7),
        };

        // Column 0 can receive the 7♥ (8♣ on top); nothing visible fits on a 7♥.
        let mut board = KlondikeBoard::new();
        board.foundation[Suit::Heart as usize] = 7;
        board.columns[0].push(Card::new(Suit::Club, 8), false);
        board.columns[1].push(Card::new(Suit::Spade, 9), true);
        board.columns[1].push(Card::new(Suit::Heart, 6), false); // same color: does not fit
        board.columns[2].push(Card::new(Suit::Club, 5), false); // wrong rank
        board.stock[0] = Card::new(Suit::Club, 6);
        board.stock_len = 1;
        board.stock_index = 0; // not yet in the waste
        board.compute_signature();
        assert!(rule.can_prune_early(&board, rollback, None, 1));

        // A black six face up in another column is a receiver, even if it is
        // not the top card of its run.
        let mut with_tableau_receiver = board;
        with_tableau_receiver.columns[3].push(Card::new(Suit::Spade, 6), false);
        with_tableau_receiver.columns[3].push(Card::new(Suit::Diamond, 5), false);
        assert!(!rule.can_prune_early(&with_tableau_receiver, rollback, None, 1));

        // A receiver sitting in the destination column itself does not count.
        let mut receiver_in_destination = board;
        receiver_in_destination.columns[0].push(Card::new(Suit::Spade, 6), false);
        assert!(rule.can_prune_early(&receiver_in_destination, rollback, None, 1));

        // The current waste card is a receiver too.
        let mut with_waste_receiver = board;
        with_waste_receiver.stock_index = 1; // 6♣ becomes the waste card
        assert!(!rule.can_prune_early(&with_waste_receiver, rollback, None, 1));

        // Aces never return to the tableau.
        let ace_back = KlondikeMove::FoundationToColumn {
            destination: 0,
            card: Card::new(Suit::Spade, 1),
        };
        assert!(rule.can_prune_early(&board, ace_back, None, 1));

        // Other move kinds are never pruned by this rule, and nothing reaches should_prune.
        let other = KlondikeMove::ColumnToColumn {
            source: 2,
            destination: 0,
            count: 1,
        };
        assert!(!rule.can_prune_early(&board, other, None, 1));
        let transition = KlondikeTransition {
            to_board: rollback.apply(board).unwrap(),
            the_move: rollback,
            previous_move: None,
            depth: 1,
        };
        assert!(!rule.should_prune(&transition, &board));
    }

    /// Verifies that KingToEmptyMustExpose allows the move when it exposes a
    /// face-down card, and prunes it when it exposes none.
    /// Pruning is evaluated in can_prune_early (pre-apply), not in should_prune.
    #[test]
    fn king_to_empty_must_expose_only_allows_hidden_flip() {
        let rule = KlondikeRule::KingToEmptyMustExpose;
        let the_move = KlondikeMove::ColumnToColumn {
            source: 0,
            destination: 1,
            count: 1,
        };

        // Allowed case: column 0 has a hidden card below the King.
        let mut from = KlondikeBoard::new();
        from.columns[0].push(Card::new(Suit::Club, 9), true);
        from.columns[0].push(Card::new(Suit::Heart, 13), false);

        assert!(
            !rule.can_prune_early(&from, the_move, None, 1),
            "moving king should be allowed if it flips a hidden card"
        );

        // Pruned case: column 0 only has the King, with no hidden cards below.
        let mut no_expose = KlondikeBoard::new();
        no_expose.columns[0].push(Card::new(Suit::Spade, 13), false);

        assert!(
            rule.can_prune_early(&no_expose, the_move, None, 1),
            "moving king should be pruned if it does not expose hidden cards"
        );
    }
}
