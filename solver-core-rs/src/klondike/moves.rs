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
    Deal,
}

impl KlondikeMove {
    // Generates legal primitive moves, using one representative empty destination.
    pub fn find_candidate_moves(board: &KlondikeBoard, draw_advance: u8) -> Vec<KlondikeMove> {
        let mut moves = Vec::with_capacity(32);
        let first_empty = board.columns.iter().position(|column| column.len == 0);

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

        // 2. Stock to foundation.
        if let Some(card) = board.stock_pile_card() {
            if board.can_add_to_foundation(card) {
                moves.push(KlondikeMove::StockPileToFoundation { card });
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

        // 4. Stock to column.
        if let Some(card) = board.stock_pile_card() {
            for dest in 0..7 {
                if board.columns[dest].len == 0 && Some(dest) != first_empty {
                    continue;
                }
                if board.columns[dest].can_add_run(card) {
                    moves.push(KlondikeMove::StockPileToColumn {
                        destination: dest as u8,
                        card,
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

        // 6. Stock advance.
        if board.can_advance_stock() {
            moves.push(KlondikeMove::StockPileAdvance {
                beginning_index: board.stock_index as u8,
                increment: draw_advance,
            });
        }

        // 7. Stock recycle.
        if board.can_recycle_stock() {
            moves.push(KlondikeMove::StockPileRecycle {
                source_index: board.stock_index as u8,
            });
        }

        moves
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
                if board.stock_index == 0 || board.stock_index > board.stock_len {
                    return None;
                }
                let real_idx = (board.stock_index - 1) as usize;
                let c = board.stock[real_idx];
                for i in real_idx..board.stock_len as usize - 1 {
                    board.stock[i] = board.stock[i + 1];
                }
                board.stock_len -= 1;
                board.stock_index -= 1;
                board.stock_recycles = 0;

                let d_col = &mut board.columns[destination as usize];
                d_col.cards[d_col.len as usize] = c;
                d_col.len += 1;
            }
            KlondikeMove::StockPileToFoundation { card } => {
                if board.stock_index == 0 || board.stock_index > board.stock_len {
                    return None;
                }
                let real_idx = (board.stock_index - 1) as usize;
                for i in real_idx..board.stock_len as usize - 1 {
                    board.stock[i] = board.stock[i + 1];
                }
                board.stock_len -= 1;
                board.stock_index -= 1;
                board.stock_recycles = 0;
                board.foundation[card.suit as usize] = card.value;
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
                if !board.can_advance_stock() {
                    return None;
                }
                board.stock_index += increment;
                if board.stock_index > board.stock_len {
                    board.stock_index = board.stock_len;
                }
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
            KlondikeMove::ColumnToFoundation { .. } | KlondikeMove::StockPileToFoundation { .. }
        )
    }

    pub fn is_from_foundation(self) -> bool {
        matches!(self, KlondikeMove::FoundationToColumn { .. })
    }

    pub fn is_stock_advance(self) -> bool {
        matches!(self, KlondikeMove::StockPileAdvance { .. })
    }

    pub fn is_stock_recycle(self) -> bool {
        matches!(self, KlondikeMove::StockPileRecycle { .. })
    }
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
