use super::board::SpiderBoard;
use super::column::SpiderColumn;
use crate::common::card::Card;

/// Todos los movimientos posibles en Spider.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpiderMove {
    /// Reparto inicial: distribuye cartas en columnas.
    Deal { deck: Vec<Card>, num_columns: usize },
    /// Mueve una racha descendente del mismo palo de una columna a otra.
    ColumnToColumn {
        source: usize,
        destination: usize,
        card_count: usize,
    },
    /// Reparte una carta del stock a cada columna.
    DealFromStock,
}

impl SpiderMove {
    /// Indica si el movimiento es un reparto desde stock.
    pub fn is_deal_from_stock(&self) -> bool {
        matches!(self, SpiderMove::DealFromStock)
    }

    /// Indica si el movimiento es entre columnas.
    pub fn is_column_to_column(&self) -> bool {
        matches!(self, SpiderMove::ColumnToColumn { .. })
    }

    /// Devuelve la columna de origen si aplica.
    pub fn source_column(&self) -> Option<usize> {
        match self {
            SpiderMove::ColumnToColumn { source, .. } => Some(*source),
            _ => None,
        }
    }

    /// Devuelve la columna de destino si aplica.
    pub fn destination_column(&self) -> Option<usize> {
        match self {
            SpiderMove::ColumnToColumn { destination, .. } => Some(*destination),
            _ => None,
        }
    }

    /// Cantidad de cartas movidas (0 para movimientos que no son entre columnas).
    pub fn card_count(&self) -> usize {
        match self {
            SpiderMove::ColumnToColumn { card_count, .. } => *card_count,
            _ => 0,
        }
    }

    /// Aplica este movimiento al tablero y devuelve el nuevo estado o `None` si es inválido.
    pub fn apply(&self, board: &SpiderBoard) -> Option<SpiderBoard> {
        match self {
            SpiderMove::Deal { deck, num_columns } => apply_deal(deck, *num_columns),

            SpiderMove::ColumnToColumn {
                source,
                destination,
                card_count,
            } => {
                let src = *source;
                let dest = *destination;
                if src == dest || src >= board.columns.len() || dest >= board.columns.len() {
                    return None;
                }

                // Extrae racha del mismo palo desde la columna de origen.
                let (extracted_run, col_from) =
                    board.columns[src].extract_same_suit_run(*card_count)?;

                // Verifica que el destino pueda aceptar la racha.
                if !board.columns[dest].can_add_run(&extracted_run) {
                    return None;
                }
                let col_to = board.columns[dest].with_cards(&extracted_run)?;

                let mut cols = board.columns.clone();
                cols[src] = col_from;
                cols[dest] = col_to;
                let mut new_completed = board.completed_sets;

                // Revisa secuencias completas en la columna de destino.
                let (cleaned_dest, removed_dest) = cols[dest].check_and_remove_complete_sequences();
                cols[dest] = cleaned_dest;
                new_completed += removed_dest;

                // Revisa secuencias completas en la columna de origen
                // (al voltear una carta puede aparecer una).
                let (cleaned_src, removed_src) = cols[src].check_and_remove_complete_sequences();
                cols[src] = cleaned_src;
                new_completed += removed_src;

                Some(SpiderBoard::new(cols, board.stock.clone(), new_completed))
            }

            SpiderMove::DealFromStock => {
                if !board.can_deal_from_stock() {
                    return None;
                }
                let num_cols = board.columns.len();
                if board.stock.len() < num_cols {
                    return None;
                }

                let mut cols = board.columns.clone();
                let dealt: Vec<Card> = board.stock[..num_cols].to_vec();
                let remaining_stock = board.stock[num_cols..].to_vec();

                for i in 0..num_cols {
                    cols[i] = cols[i].with_card(dealt[i]);
                }

                let mut new_completed = board.completed_sets;

                // Revisa secuencias completas en todas las columnas tras repartir.
                for i in 0..num_cols {
                    let (cleaned, removed) = cols[i].check_and_remove_complete_sequences();
                    cols[i] = cleaned;
                    new_completed += removed;
                }

                Some(SpiderBoard::new(cols, remaining_stock, new_completed))
            }
        }
    }
}

/// Aplica el reparto inicial: 104 cartas en 10 columnas.
/// Columnas 0-3: 6 cartas cada una (5 boca abajo + 1 boca arriba).
/// Columnas 4-9: 5 cartas cada una (4 boca abajo + 1 boca arriba).
/// Las 50 cartas restantes van al stock.
fn apply_deal(cards: &[Card], num_columns: usize) -> Option<SpiderBoard> {
    if cards.len() != 104 {
        return None;
    }

    let mut stacks: Vec<Vec<Card>> = vec![vec![]; num_columns];
    let mut idx = 0;

    for col in 0..num_columns {
        let count = if col < 4 { 6 } else { 5 };
        for _ in 0..count {
            if idx >= cards.len() {
                return None;
            }
            stacks[col].push(cards[idx]);
            idx += 1;
        }
    }

    // Todas menos la última de cada pila van boca abajo; la última va boca arriba.
    let columns: Vec<SpiderColumn> = stacks
        .into_iter()
        .map(|stack| {
            if stack.is_empty() {
                return SpiderColumn::empty();
            }
            let fd = stack[..stack.len() - 1].to_vec();
            let fu = vec![*stack.last().unwrap()];
            SpiderColumn::new(fd, fu)
        })
        .collect();

    let stock_pile = cards[idx..].to_vec();
    Some(SpiderBoard::new(columns, stock_pile, 0))
}

// -- Generación de movimientos --

impl SpiderMove {
    /// Genera todos los movimientos válidos entre columnas.
    pub fn find_column_to_column_moves(board: &SpiderBoard) -> Vec<SpiderMove> {
        let mut moves = Vec::new();
        let cols = &board.columns;

        for src in 0..cols.len() {
            let src_col = &cols[src];
            if !src_col.has_face_up() {
                continue;
            }

            let max_run_len = src_col.longest_run;
            if max_run_len < 1 {
                continue;
            }

            let src_fu = &src_col.face_up;

            for run_len in 1..=max_run_len {
                // Obtiene la carta inferior de la racha para filtrar destinos rápido.
                let bottom_card = src_fu[src_fu.len() - run_len];

                // Verifica si existe al menos un destino válido.
                let has_valid_dest = cols
                    .iter()
                    .enumerate()
                    .any(|(dest, col)| dest != src && col.can_add_card(bottom_card));
                if !has_valid_dest {
                    continue;
                }

                for dest in 0..cols.len() {
                    if dest == src {
                        continue;
                    }
                    if !cols[dest].can_add_card(bottom_card) {
                        continue;
                    }

                    // Evita movimientos triviales: mover todo `face_up` a una columna vacía
                    // cuando la columna origen no tiene cartas boca abajo.
                    if cols[dest].is_empty() {
                        if run_len == src_col.num_face_up() && !src_col.has_face_down() {
                            continue;
                        }
                    }

                    moves.push(SpiderMove::ColumnToColumn {
                        source: src,
                        destination: dest,
                        card_count: run_len,
                    });
                }
            }
        }

        moves
    }

    /// Genera el movimiento de reparto desde stock si es legal.
    pub fn find_deal_from_stock_moves(board: &SpiderBoard) -> Vec<SpiderMove> {
        if board.can_deal_from_stock() {
            vec![SpiderMove::DealFromStock]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::card::Suit;

    fn card(suit: Suit, value: u8) -> Card {
        Card::new(suit, value)
    }

    #[test]
    fn test_deal() {
        let mut deck = Vec::with_capacity(104);
        for _ in 0..8 {
            for v in 1..=13 {
                deck.push(card(Suit::Spade, v));
            }
        }
        let board = apply_deal(&deck, 10).unwrap();
        assert_eq!(board.columns.len(), 10);
        assert_eq!(board.stock.len(), 50);
        // Columns 0-3 have 6 cards (5 face-down + 1 face-up)
        for i in 0..4 {
            assert_eq!(board.columns[i].num_face_down(), 5);
            assert_eq!(board.columns[i].num_face_up(), 1);
        }
        // Columns 4-9 have 5 cards (4 face-down + 1 face-up)
        for i in 4..10 {
            assert_eq!(board.columns[i].num_face_down(), 4);
            assert_eq!(board.columns[i].num_face_up(), 1);
        }
    }

    #[test]
    fn test_column_to_column_move() {
        // Prepara un tablero con una carta que puede moverse a otra columna.
        let mut cols: Vec<SpiderColumn> = (0..10).map(|_| SpiderColumn::empty()).collect();
        cols[0] = SpiderColumn::new(vec![], vec![card(Suit::Spade, 5)]);
        cols[1] = SpiderColumn::new(vec![], vec![card(Suit::Heart, 6)]);

        let board = SpiderBoard::new(cols, vec![], 0);
        let moves = SpiderMove::find_column_to_column_moves(&board);

        // El 5 puede ir sobre el 6.
        assert!(moves.iter().any(|m| {
            matches!(
                m,
                SpiderMove::ColumnToColumn {
                    source: 0,
                    destination: 1,
                    ..
                }
            )
        }));
    }
}
