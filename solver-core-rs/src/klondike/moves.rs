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
    // Genera todos los movimientos primitivos válidos
    pub fn find_candidate_moves(board: &KlondikeBoard, draw_advance: u8) -> Vec<KlondikeMove> {
        let mut moves = Vec::with_capacity(32);

        // 1. Columna a Foundation
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

        // 2. Stock a Foundation
        if let Some(card) = board.stock_pile_card() {
            if board.can_add_to_foundation(card) {
                moves.push(KlondikeMove::StockPileToFoundation { card });
            }
        }

        // 3. Columna a Columna
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
                    continue; // Mover Rey en columna vacía es redundante
                }

                for dest in 0..7 {
                    if src == dest {
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

        // 4. Stock a Columna
        if let Some(card) = board.stock_pile_card() {
            for dest in 0..7 {
                if board.columns[dest].can_add_run(card) {
                    moves.push(KlondikeMove::StockPileToColumn {
                        destination: dest as u8,
                        card,
                    });
                }
            }
        }

        // 5. Foundation a Columna
        for &suit in crate::common::card::Suit::ALL.iter() {
            if let Some(card) = board.top_of_foundation(suit) {
                for dest in 0..7 {
                    if board.columns[dest].can_add_run(card) {
                        moves.push(KlondikeMove::FoundationToColumn {
                            destination: dest as u8,
                            card,
                        });
                    }
                }
            }
        }

        // 6. Avance de Stock
        if board.can_advance_stock() {
            moves.push(KlondikeMove::StockPileAdvance {
                beginning_index: board.stock_index as u8,
                increment: draw_advance,
            });
        }

        // 7. Reciclar Stock
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
                // Expone una carta si la secuencia consumió todas las cartas boca arriba
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
                board.foundation[card.suit as usize] = card.value; // Almacena el rango más alto

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
            KlondikeMove::Deal => return None, // Deal no debería ser aplicado dinámicamente dentro del flujo IDA*
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

// Estos métodos de compatibilidad son solo atajos para el agrupamiento de find_candidate_moves
pub fn find_stock_to_foundation_moves(board: &KlondikeBoard) -> Vec<KlondikeMove> {
    if let Some(card) = board.stock_pile_card() {
        if board.can_add_to_foundation(card) {
            return vec![KlondikeMove::StockPileToFoundation { card }];
        }
    }
    vec![]
}
