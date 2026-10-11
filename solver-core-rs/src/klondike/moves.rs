use super::board::KlondikeBoard;
use crate::common::card::Card;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum KlondikeMove {
    ColumnToColumn {
        source: u8,
        destination: u8,
        count: u8,
    },
    ColumnToFoundation {
        source: u8,
        card: Card,
    },
    StockPileToColumn {
        destination: u8,
        card: Card,
    },
    StockPileToFoundation {
        card: Card,
    },
    FoundationToColumn {
        destination: u8,
        card: Card,
    },
    StockPileAdvance {
        beginning_index: u8,
        increment: u8,
    },
    StockPileRecycle {
        source_index: u8,
    },
    /// Stock macro: advance the stock `advances` times (`increment` cards each),
    /// then play the waste card to a column. Only generated with `advances > 0`;
    /// the search expands it back to primitives before returning a path.
    StockDrawToColumn {
        advances: u8,
        increment: u8,
        destination: u8,
        card: Card,
    },
    /// Stock macro: advance `advances` times, then play the waste card to its foundation.
    StockDrawToFoundation {
        advances: u8,
        increment: u8,
        card: Card,
    },
    /// Stock macro: advance `advances` times to the end of the stock, then recycle.
    /// Recycling mid-pass reaches the same state, so the search never needs
    /// the intermediate advances as nodes.
    StockDrawRecycle {
        advances: u8,
        increment: u8,
    },
    Deal,
}

impl KlondikeMove {
    /// Generates the legal moves the search explores, using one representative
    /// empty destination. The stock is modelled with macro moves: pure advances
    /// are never generated; every waste card reachable in the current pass is
    /// offered as a single "advance k times and play" move, and recycling is
    /// available from any position of the pass. Plays of the current waste card
    /// and a recycle at the end of the stock stay primitive.
    pub fn find_candidate_moves(board: &KlondikeBoard, draw_advance: u8) -> Vec<KlondikeMove> {
        let mut moves = Vec::with_capacity(32);
        let first_empty = board.columns.iter().position(|column| column.len == 0);
        let increment = draw_advance.max(1);
        let reachable = reachable_waste_positions(board, increment);

        // 1. Column to foundation.
        for c in 0..7 {
            if let Some(card) = board.columns[c].top_face_up() {
                if board.can_add_to_foundation(card) {
                    moves.push(KlondikeMove::ColumnToFoundation {
                        source: c as u8,
                        card,
                    });
                }
            }
        }

        // 2. Stock to foundation, for every waste card reachable in this pass.
        for &(advances, card) in &reachable {
            if board.can_add_to_foundation(card) {
                moves.push(if advances == 0 {
                    KlondikeMove::StockPileToFoundation { card }
                } else {
                    KlondikeMove::StockDrawToFoundation {
                        advances,
                        increment,
                        card,
                    }
                });
            }
        }

        // 3. Column to column.
        for src in 0..7 {
            let col = &board.columns[src];
            if !col.has_face_up() {
                continue;
            }
            let num_up = col.num_face_up();

            for k in 1..=num_up {
                let current_idx = col.len - k as u8;
                let run_base = col.cards[current_idx as usize];

                if run_base.value == 13 && k == num_up as usize && col.face_down_len == 0 {
                    continue; // Moving a King to an empty column is redundant.
                }

                for dest in 0..7 {
                    if src == dest {
                        continue;
                    }
                    if board.columns[dest].len == 0 && Some(dest) != first_empty {
                        continue;
                    }
                    if board.columns[dest].can_add_run(run_base) {
                        moves.push(KlondikeMove::ColumnToColumn {
                            source: src as u8,
                            destination: dest as u8,
                            count: k as u8,
                        });
                    }
                }
            }
        }

        // 4. Stock to column, for every waste card reachable in this pass.
        for &(advances, card) in &reachable {
            for dest in 0..7 {
                if board.columns[dest].len == 0 && Some(dest) != first_empty {
                    continue;
                }
                if board.columns[dest].can_add_run(card) {
                    moves.push(if advances == 0 {
                        KlondikeMove::StockPileToColumn {
                            destination: dest as u8,
                            card,
                        }
                    } else {
                        KlondikeMove::StockDrawToColumn {
                            advances,
                            increment,
                            destination: dest as u8,
                            card,
                        }
                    });
                }
            }
        }

        // 5. Foundation to column.
        for &suit in crate::common::card::Suit::ALL.iter() {
            if let Some(card) = board.top_of_foundation(suit) {
                for dest in 0..7 {
                    if board.columns[dest].len == 0 && Some(dest) != first_empty {
                        continue;
                    }
                    if board.columns[dest].can_add_run(card) {
                        moves.push(KlondikeMove::FoundationToColumn {
                            destination: dest as u8,
                            card,
                        });
                    }
                }
            }
        }

        // 6. Recycle from anywhere in the pass; there are no pure advances.
        if board.stock_len > 0 && board.stock_index > 0 {
            let advances = advances_to_end(board, increment);
            moves.push(if advances == 0 {
                KlondikeMove::StockPileRecycle {
                    source_index: board.stock_index,
                }
            } else {
                KlondikeMove::StockDrawRecycle {
                    advances,
                    increment,
                }
            });
        }

        moves
    }

    /// Legal primitive moves, as a UI applies them one by one: the candidate
    /// moves without stock macros, plus the single-step stock advance.
    pub fn find_primitive_moves(board: &KlondikeBoard, draw_advance: u8) -> Vec<KlondikeMove> {
        let mut moves: Vec<KlondikeMove> = Self::find_candidate_moves(board, draw_advance)
            .into_iter()
            .filter(|m| !m.is_stock_macro())
            .collect();
        if board.can_advance_stock() {
            moves.push(KlondikeMove::StockPileAdvance {
                beginning_index: board.stock_index,
                increment: draw_advance,
            });
        }
        moves
    }

    /// Number of primitive moves this move stands for (1 + stock advances).
    pub fn primitive_len(self) -> usize {
        1 + self.stock_advances() as usize
    }

    /// Stock advances folded into this move; zero for primitives.
    pub fn stock_advances(self) -> u8 {
        match self {
            KlondikeMove::StockDrawToColumn { advances, .. }
            | KlondikeMove::StockDrawToFoundation { advances, .. }
            | KlondikeMove::StockDrawRecycle { advances, .. } => advances,
            _ => 0,
        }
    }

    pub fn is_stock_macro(self) -> bool {
        matches!(
            self,
            KlondikeMove::StockDrawToColumn { .. }
                | KlondikeMove::StockDrawToFoundation { .. }
                | KlondikeMove::StockDrawRecycle { .. }
        )
    }

    /// Primitive moves equivalent to `self` from `board`: the advances it folds
    /// (with their real `beginning_index`) followed by the final play. Primitive
    /// moves return themselves.
    pub fn primitive_steps(self, board: &KlondikeBoard) -> Vec<KlondikeMove> {
        let advances = self.stock_advances();
        if advances == 0 {
            return vec![self];
        }
        let mut steps = Vec::with_capacity(advances as usize + 1);
        let mut index = board.stock_index;
        let increment = match self {
            KlondikeMove::StockDrawToColumn { increment, .. }
            | KlondikeMove::StockDrawToFoundation { increment, .. }
            | KlondikeMove::StockDrawRecycle { increment, .. } => increment,
            _ => unreachable!("only stock macros fold advances"),
        };
        for _ in 0..advances {
            steps.push(KlondikeMove::StockPileAdvance {
                beginning_index: index,
                increment,
            });
            index = index.saturating_add(increment).min(board.stock_len);
        }
        steps.push(match self {
            KlondikeMove::StockDrawToColumn {
                destination, card, ..
            } => KlondikeMove::StockPileToColumn { destination, card },
            KlondikeMove::StockDrawToFoundation { card, .. } => {
                KlondikeMove::StockPileToFoundation { card }
            }
            KlondikeMove::StockDrawRecycle { .. } => KlondikeMove::StockPileRecycle {
                source_index: index,
            },
            _ => unreachable!("only stock macros fold advances"),
        });
        steps
    }

    pub fn apply(self, mut board: KlondikeBoard) -> Option<KlondikeBoard> {
        match self {
            KlondikeMove::ColumnToColumn {
                source,
                destination,
                count,
            } => {
                board.stock_recycles = 0;
                let s_idx = source as usize;
                let d_idx = destination as usize;
                let s_col = &mut board.columns[s_idx];
                if s_col.len < s_col.face_down_len + count {
                    return None;
                }

                let start_idx = s_col.len - count;
                let mut moved =
                    [crate::common::card::Card::new(crate::common::card::Suit::Club, 0); 13];
                for i in 0..count {
                    moved[i as usize] = s_col.cards[(start_idx + i) as usize];
                }

                s_col.len -= count;
                // Reveals a card if the moved run consumed all face-up cards.
                if s_col.len > 0 && s_col.face_down_len == s_col.len {
                    s_col.face_down_len -= 1;
                }

                let d_col = &mut board.columns[d_idx];
                for i in 0..count {
                    d_col.cards[d_col.len as usize] = moved[i as usize];
                    d_col.len += 1;
                }
            }
            KlondikeMove::ColumnToFoundation { source, .. } => {
                board.stock_recycles = 0;
                let s_idx = source as usize;
                let card = board.columns[s_idx].pop()?;
                board.foundation[card.suit as usize] = card.value; // Stores the highest rank.

                let col = &mut board.columns[s_idx];
                if col.len > 0 && col.face_down_len == col.len {
                    col.face_down_len -= 1;
                }
            }
            KlondikeMove::StockPileToColumn {
                destination,
                card: _,
            } => {
                let c = take_waste_card(&mut board)?;
                let d_col = &mut board.columns[destination as usize];
                d_col.cards[d_col.len as usize] = c;
                d_col.len += 1;
            }
            KlondikeMove::StockPileToFoundation { card } => {
                take_waste_card(&mut board)?;
                board.foundation[card.suit as usize] = card.value;
            }
            KlondikeMove::StockDrawToColumn {
                advances,
                increment,
                destination,
                card,
            } => {
                for _ in 0..advances {
                    advance_stock(&mut board, increment)?;
                }
                let c = take_waste_card(&mut board)?;
                if c != card {
                    return None;
                }
                let d_col = &mut board.columns[destination as usize];
                d_col.cards[d_col.len as usize] = c;
                d_col.len += 1;
            }
            KlondikeMove::StockDrawToFoundation {
                advances,
                increment,
                card,
            } => {
                for _ in 0..advances {
                    advance_stock(&mut board, increment)?;
                }
                if take_waste_card(&mut board)? != card {
                    return None;
                }
                board.foundation[card.suit as usize] = card.value;
            }
            KlondikeMove::StockDrawRecycle {
                advances,
                increment,
            } => {
                for _ in 0..advances {
                    advance_stock(&mut board, increment)?;
                }
                if !board.can_recycle_stock() {
                    return None;
                }
                board.stock_index = 0;
                board.stock_recycles += 1;
            }
            KlondikeMove::FoundationToColumn { destination, card } => {
                board.stock_recycles = 0;
                if board.foundation[card.suit as usize] != card.value {
                    return None;
                }
                board.foundation[card.suit as usize] -= 1;
                let col = &mut board.columns[destination as usize];
                col.cards[col.len as usize] = card;
                col.len += 1;
            }
            KlondikeMove::StockPileAdvance {
                beginning_index: _,
                increment,
            } => {
                advance_stock(&mut board, increment)?;
            }
            KlondikeMove::StockPileRecycle { source_index: _ } => {
                if !board.can_recycle_stock() {
                    return None;
                }
                board.stock_index = 0;
                board.stock_recycles += 1;
            }
            KlondikeMove::Deal => return None, // Deal should not be applied dynamically in the IDA* flow.
        }

        board.compute_signature();
        Some(board)
    }

    pub fn is_to_foundation(self) -> bool {
        matches!(
            self,
            KlondikeMove::ColumnToFoundation { .. }
                | KlondikeMove::StockPileToFoundation { .. }
                | KlondikeMove::StockDrawToFoundation { .. }
        )
    }

    pub fn is_from_foundation(self) -> bool {
        matches!(self, KlondikeMove::FoundationToColumn { .. })
    }

    pub fn is_stock_advance(self) -> bool {
        matches!(self, KlondikeMove::StockPileAdvance { .. })
    }

    pub fn is_stock_recycle(self) -> bool {
        matches!(
            self,
            KlondikeMove::StockPileRecycle { .. } | KlondikeMove::StockDrawRecycle { .. }
        )
    }
}

/// Waste cards reachable in the current pass as `(advances, card)`: the
/// current waste card first (0 advances), then every `increment`-th card and
/// the last one. Positions are 1-based stock indices, like `stock_index`.
fn reachable_waste_positions(board: &KlondikeBoard, increment: u8) -> Vec<(u8, Card)> {
    let mut reachable = Vec::with_capacity(9);
    let mut index = board.stock_index;
    let mut advances = 0u8;
    loop {
        if index > 0 && index <= board.stock_len {
            reachable.push((advances, board.stock[(index - 1) as usize]));
        }
        if index >= board.stock_len {
            break;
        }
        index = index.saturating_add(increment).min(board.stock_len);
        advances += 1;
    }
    reachable
}

/// Advances needed to reach the end of the stock from the current position.
fn advances_to_end(board: &KlondikeBoard, increment: u8) -> u8 {
    let remaining = board.stock_len.saturating_sub(board.stock_index);
    remaining.div_ceil(increment.max(1))
}

fn advance_stock(board: &mut KlondikeBoard, increment: u8) -> Option<()> {
    if !board.can_advance_stock() {
        return None;
    }
    board.stock_index = board
        .stock_index
        .saturating_add(increment)
        .min(board.stock_len);
    Some(())
}

/// Removes the current waste card, compacting the stock so the card below it
/// becomes the new waste card. Resets the recycle counter.
fn take_waste_card(board: &mut KlondikeBoard) -> Option<Card> {
    if board.stock_index == 0 || board.stock_index > board.stock_len {
        return None;
    }
    let real_idx = (board.stock_index - 1) as usize;
    let card = board.stock[real_idx];
    for i in real_idx..board.stock_len as usize - 1 {
        board.stock[i] = board.stock[i + 1];
    }
    board.stock_len -= 1;
    board.stock_index -= 1;
    board.stock_recycles = 0;
    Some(card)
}

/// Replays `path` from `initial`, expanding stock macros into the primitive
/// moves a UI can apply one by one. Stops at the first move that fails to
/// apply, returning the legal prefix.
pub fn expand_stock_macros(initial: &KlondikeBoard, path: &[KlondikeMove]) -> Vec<KlondikeMove> {
    let mut out = Vec::with_capacity(path.len());
    let mut board = *initial;
    for &m in path {
        for step in m.primitive_steps(&board) {
            let Some(next) = step.apply(board) else {
                debug_assert!(false, "solver paths must replay: {step:?}");
                return out;
            };
            out.push(step);
            board = next;
        }
    }
    out
}

pub fn find_column_to_foundation_moves(board: &KlondikeBoard) -> Vec<KlondikeMove> {
    let mut moves = Vec::new();
    for c in 0..7 {
        if let Some(card) = board.columns[c].top_face_up() {
            if board.can_add_to_foundation(card) {
                moves.push(KlondikeMove::ColumnToFoundation {
                    source: c as u8,
                    card,
                });
            }
        }
    }
    moves
}

// These compatibility methods are just shortcuts for the find_candidate_moves grouping.
pub fn find_stock_to_foundation_moves(board: &KlondikeBoard) -> Vec<KlondikeMove> {
    if let Some(card) = board.stock_pile_card() {
        if board.can_add_to_foundation(card) {
            return vec![KlondikeMove::StockPileToFoundation { card }];
        }
    }
    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::card::Suit;

    /// Draw-3 stock of eight cards. From `stock_index = 0` the pass reaches
    /// positions 3 (7♠), 6 (5♣) and 8 (7♣); 2♦ at position 2 is skipped.
    fn macro_stock_board(stock_index: u8) -> KlondikeBoard {
        let mut board = KlondikeBoard::new();
        board.columns[0].push(Card::new(Suit::Heart, 8), false); // accepts a black 7
        board.foundation[Suit::Club as usize] = 4; // accepts 5♣
        board.foundation[Suit::Diamond as usize] = 1; // would accept 2♦
        for (i, card) in [
            Card::new(Suit::Club, 9),
            Card::new(Suit::Diamond, 2),
            Card::new(Suit::Spade, 7),
            Card::new(Suit::Heart, 3),
            Card::new(Suit::Diamond, 4),
            Card::new(Suit::Club, 5),
            Card::new(Suit::Spade, 10),
            Card::new(Suit::Club, 7),
        ]
        .into_iter()
        .enumerate()
        {
            board.stock[i] = card;
        }
        board.stock_len = 8;
        board.stock_index = stock_index;
        board.compute_signature();
        board
    }

    #[test]
    fn macro_stock_offers_every_reachable_waste_card_and_no_pure_advances() {
        let board = macro_stock_board(0);
        let moves = KlondikeMove::find_candidate_moves(&board, 3);
        let seven_spades = KlondikeMove::StockDrawToColumn {
            advances: 1,
            increment: 3,
            destination: 0,
            card: Card::new(Suit::Spade, 7),
        };
        let five_clubs = KlondikeMove::StockDrawToFoundation {
            advances: 2,
            increment: 3,
            card: Card::new(Suit::Club, 5),
        };
        let seven_clubs = KlondikeMove::StockDrawToColumn {
            advances: 3,
            increment: 3,
            destination: 0,
            card: Card::new(Suit::Club, 7),
        };
        for m in [seven_spades, five_clubs, seven_clubs] {
            assert!(moves.contains(&m), "{m:?} missing from {moves:?}");
            assert_eq!(m.primitive_len(), 1 + m.stock_advances() as usize);
            assert!(m.is_stock_macro());
        }
        assert!(!moves.iter().any(|m| m.is_stock_advance()));
        assert!(!moves.iter().any(|m| m.is_stock_recycle()));
        // 2♦ sits between two draw positions: not reachable in this pass.
        assert!(!moves.iter().any(|m| matches!(
            m,
            KlondikeMove::StockDrawToFoundation { card, .. } if *card == Card::new(Suit::Diamond, 2)
        )));

        // The primitive view keeps the single-step advance and drops the macros.
        let primitive = KlondikeMove::find_primitive_moves(&board, 3);
        assert!(primitive.contains(&KlondikeMove::StockPileAdvance {
            beginning_index: 0,
            increment: 3,
        }));
        assert!(!primitive.iter().any(|m| m.is_stock_macro()));
        assert_eq!(
            primitive.len(),
            moves.iter().filter(|m| !m.is_stock_macro()).count() + 1
        );
    }

    #[test]
    fn macro_stock_keeps_the_current_waste_card_primitive_and_recycles_mid_pass() {
        let board = macro_stock_board(3);
        let moves = KlondikeMove::find_candidate_moves(&board, 3);
        assert!(moves.contains(&KlondikeMove::StockPileToColumn {
            destination: 0,
            card: Card::new(Suit::Spade, 7),
        }));
        assert!(moves.contains(&KlondikeMove::StockDrawToFoundation {
            advances: 1,
            increment: 3,
            card: Card::new(Suit::Club, 5),
        }));
        // Five cards remain: two advances reach the end, then recycle.
        assert!(moves.contains(&KlondikeMove::StockDrawRecycle {
            advances: 2,
            increment: 3,
        }));
        assert!(!moves.iter().any(|m| m.is_stock_advance()));

        let at_end = macro_stock_board(8);
        let moves = KlondikeMove::find_candidate_moves(&at_end, 3);
        assert!(moves.contains(&KlondikeMove::StockPileRecycle { source_index: 8 }));
        let primitive = KlondikeMove::find_primitive_moves(&at_end, 3);
        assert!(primitive.contains(&KlondikeMove::StockPileRecycle { source_index: 8 }));
        assert!(!primitive.iter().any(|m| m.is_stock_advance()));
        assert!(!moves
            .iter()
            .any(|m| matches!(m, KlondikeMove::StockDrawRecycle { .. })));
    }

    #[test]
    fn stock_macros_apply_exactly_like_their_primitive_steps() {
        for stock_index in [0, 3, 5] {
            let board = macro_stock_board(stock_index);
            let macros: Vec<_> = KlondikeMove::find_candidate_moves(&board, 3)
                .into_iter()
                .filter(|m| m.is_stock_macro())
                .collect();
            assert!(!macros.is_empty());
            for m in macros {
                let via_macro = m.apply(board).expect("legal macro");
                let steps = m.primitive_steps(&board);
                assert_eq!(steps.len(), m.primitive_len());
                let mut replayed = board;
                for step in steps {
                    assert!(!step.is_stock_macro());
                    assert!(
                        KlondikeMove::find_primitive_moves(&replayed, 3).contains(&step),
                        "{step:?} must be a legal primitive"
                    );
                    replayed = step.apply(replayed).unwrap();
                }
                assert!(via_macro == replayed, "{m:?}");
                assert_eq!(via_macro.signature, replayed.signature);
                assert_eq!(via_macro.stock_recycles, replayed.stock_recycles);
            }
        }
    }

    #[test]
    fn stock_macros_reject_a_mismatched_card_or_an_impossible_advance() {
        let board = macro_stock_board(0);
        assert!(KlondikeMove::StockDrawToColumn {
            advances: 1,
            increment: 3,
            destination: 0,
            card: Card::new(Suit::Club, 7),
        }
        .apply(board)
        .is_none());
        assert!(KlondikeMove::StockDrawToFoundation {
            advances: 4,
            increment: 3,
            card: Card::new(Suit::Club, 7),
        }
        .apply(board)
        .is_none());
        assert!(KlondikeMove::StockDrawRecycle {
            advances: 1,
            increment: 3,
        }
        .apply(board)
        .is_none());
    }

    #[test]
    fn expand_stock_macros_replays_to_the_same_board_with_primitives_only() {
        let board = macro_stock_board(0);
        let path = [
            KlondikeMove::StockDrawToColumn {
                advances: 1,
                increment: 3,
                destination: 0,
                card: Card::new(Suit::Spade, 7),
            },
            // After the play the stock compacts to seven cards with 2♦ as the
            // waste card; one more advance reaches 5♣.
            KlondikeMove::StockDrawToFoundation {
                advances: 1,
                increment: 3,
                card: Card::new(Suit::Club, 5),
            },
            KlondikeMove::StockDrawRecycle {
                advances: 1,
                increment: 3,
            },
        ];
        let mut via_macros = board;
        for m in path {
            via_macros = m.apply(via_macros).expect("legal macro path");
        }
        let expanded = expand_stock_macros(&board, &path);
        assert_eq!(
            expanded,
            vec![
                KlondikeMove::StockPileAdvance {
                    beginning_index: 0,
                    increment: 3,
                },
                KlondikeMove::StockPileToColumn {
                    destination: 0,
                    card: Card::new(Suit::Spade, 7),
                },
                KlondikeMove::StockPileAdvance {
                    beginning_index: 2,
                    increment: 3,
                },
                KlondikeMove::StockPileToFoundation {
                    card: Card::new(Suit::Club, 5),
                },
                KlondikeMove::StockPileAdvance {
                    beginning_index: 4,
                    increment: 3,
                },
                KlondikeMove::StockPileRecycle { source_index: 6 },
            ]
        );
        let mut via_primitives = board;
        for step in &expanded {
            via_primitives = step.apply(via_primitives).unwrap();
        }
        assert!(via_primitives == via_macros);
        // Primitive paths pass through untouched.
        assert_eq!(expand_stock_macros(&board, &expanded), expanded);
    }

    #[test]
    fn kings_use_one_empty_destination_from_tableau_stock_and_foundation() {
        for empty_columns in [vec![1, 4], vec![4], vec![]] {
            let mut board = KlondikeBoard::new();
            board.columns[0].push(Card::new(Suit::Diamond, 2), true);
            board.columns[0].push(Card::new(Suit::Heart, 13), false);
            for column in 1..7 {
                if !empty_columns.contains(&column) {
                    board.columns[column].push(Card::new(Suit::Spade, 6 + column as u8), false);
                }
            }
            board.stock[0] = Card::new(Suit::Diamond, 13);
            board.stock_len = 1;
            board.stock_index = 1;
            board.foundation[Suit::Club as usize] = 13;
            board.compute_signature();
            for draw in [1, 3] {
                let moves = KlondikeMove::find_candidate_moves(&board, draw);
                for origin in 0..3 {
                    let destinations: Vec<_> = moves
                        .iter()
                        .filter_map(|m| match (origin, m) {
                            (
                                0,
                                KlondikeMove::ColumnToColumn {
                                    source: 0,
                                    destination,
                                    count: 1,
                                },
                            )
                            | (1, KlondikeMove::StockPileToColumn { destination, .. })
                            | (2, KlondikeMove::FoundationToColumn { destination, .. }) => {
                                Some(*destination as usize)
                            }
                            _ => None,
                        })
                        .collect();
                    assert_eq!(
                        destinations,
                        empty_columns
                            .first()
                            .copied()
                            .into_iter()
                            .collect::<Vec<_>>()
                    );
                }
                if let Some(&destination) = empty_columns.first() {
                    let m = KlondikeMove::ColumnToColumn {
                        source: 0,
                        destination: destination as u8,
                        count: 1,
                    };
                    assert_eq!(m.apply(board).unwrap().columns[0].face_down_len, 0);
                }
            }
        }
    }

    #[test]
    fn nonempty_destinations_keep_all_legal_tableau_stock_and_foundation_moves() {
        let mut board = KlondikeBoard::new();
        board.columns[0].push(Card::new(Suit::Heart, 7), false);
        for (column, suit, rank) in [
            (1, Suit::Club, 8),
            (2, Suit::Spade, 8),
            (3, Suit::Club, 7),
            (4, Suit::Spade, 7),
        ] {
            board.columns[column].push(Card::new(suit, rank), false);
        }
        board.stock[0] = Card::new(Suit::Diamond, 7);
        board.stock_len = 1;
        board.stock_index = 1;
        board.foundation[Suit::Heart as usize] = 6;
        for draw in [1, 3] {
            let moves = KlondikeMove::find_candidate_moves(&board, draw);
            for destination in [1, 2] {
                assert!(moves.contains(&KlondikeMove::ColumnToColumn {
                    source: 0,
                    destination,
                    count: 1
                }));
                assert!(moves.contains(&KlondikeMove::StockPileToColumn {
                    destination,
                    card: Card::new(Suit::Diamond, 7)
                }));
            }
            for destination in [3, 4] {
                assert!(moves.contains(&KlondikeMove::FoundationToColumn {
                    destination,
                    card: Card::new(Suit::Heart, 6)
                }));
            }
        }
    }
}
