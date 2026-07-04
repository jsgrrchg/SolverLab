use crate::common::card::Card;

/// TriPeaks board: 28 tableau positions (3 peaks, 4 rows), stock, and waste.
#[derive(Debug, Clone)]
pub struct TriPeaksBoard {
    /// 28 positions. Some = card present, None = removed.
    pub tableau: Vec<Option<Card>>,
    /// Stock pile. Cards are drawn from stock.last() to waste.
    pub stock: Vec<Card>,
    /// Waste pile. Top = waste.last(); used as adjacency reference.
    pub waste: Vec<Card>,
    /// State hash signature, used for search deduplication.
    pub signature: u64,
}

impl TriPeaksBoard {
    pub const TABLEAU_SIZE: usize = 28;

    /// Creates a TriPeaks board and recomputes its signature.
    pub fn new(tableau: Vec<Option<Card>>, stock: Vec<Card>, waste: Vec<Card>) -> TriPeaksBoard {
        assert_eq!(tableau.len(), Self::TABLEAU_SIZE);
        let signature = compute_signature(&tableau, &stock, &waste);
        TriPeaksBoard {
            tableau,
            stock,
            waste,
            signature,
        }
    }

    /// Start indices and counts per row for the 3-peak layout.
    /// Row 0: 3 cards (peaks), row 1: 6, row 2: 9, row 3: 10.
    const ROW_START: [usize; 4] = [0, 3, 9, 18];
    const ROW_COUNT: [usize; 4] = [3, 6, 9, 10];

    /// Returns the row (0-3) for the given index.
    pub fn row(index: usize) -> usize {
        assert!(index < Self::TABLEAU_SIZE);
        for r in 0..4 {
            if index < Self::ROW_START[r] + Self::ROW_COUNT[r] {
                return r;
            }
        }
        3
    }

    /// Returns parent indices for a position (cards that cover it from the row above).
    /// Must exactly match the hardcoded `parents()` mapping in Swift.
    pub fn parents(index: usize) -> Vec<usize> {
        assert!(index < Self::TABLEAU_SIZE);
        let r = Self::row(index);
        if r == 0 {
            return vec![];
        }
        let start = Self::ROW_START[r];
        let prev_start = Self::ROW_START[r - 1];
        let col_in_row = index - start;

        match r {
            1 => match col_in_row {
                0 => vec![prev_start],     // 3  → [0]
                1 => vec![prev_start],     // 4  → [0]
                2 => vec![prev_start + 1], // 5  → [1]
                3 => vec![prev_start + 1], // 6  → [1]
                4 => vec![prev_start + 2], // 7  → [2]
                5 => vec![prev_start + 2], // 8  → [2]
                _ => vec![],
            },
            2 => match col_in_row {
                0 => vec![prev_start],                     // 9  → [3]
                1 => vec![prev_start, prev_start + 1],     // 10 → [3, 4]
                2 => vec![prev_start + 1],                 // 11 → [4]
                3 => vec![prev_start + 2],                 // 12 → [5]
                4 => vec![prev_start + 2, prev_start + 3], // 13 → [5, 6]
                5 => vec![prev_start + 3],                 // 14 → [6]
                6 => vec![prev_start + 4],                 // 15 → [7]
                7 => vec![prev_start + 4, prev_start + 5], // 16 → [7, 8]
                8 => vec![prev_start + 5],                 // 17 → [8]
                _ => vec![],
            },
            3 => match col_in_row {
                0 => vec![prev_start],                     // 18 → [9]
                1 => vec![prev_start, prev_start + 1],     // 19 → [9, 10]
                2 => vec![prev_start + 1, prev_start + 2], // 20 → [10, 11]
                3 => vec![prev_start + 2, prev_start + 3], // 21 → [11, 12]
                4 => vec![prev_start + 3, prev_start + 4], // 22 → [12, 13]
                5 => vec![prev_start + 4, prev_start + 5], // 23 → [13, 14]
                6 => vec![prev_start + 5, prev_start + 6], // 24 → [14, 15]
                7 => vec![prev_start + 6, prev_start + 7], // 25 → [15, 16]
                8 => vec![prev_start + 7, prev_start + 8], // 26 → [16, 17]
                9 => vec![prev_start + 8],                 // 27 → [17]
                _ => vec![],
            },
            _ => vec![],
        }
    }

    /// Returns indices of cards that cover this position (lower row).
    /// A card is exposed when all its children have been removed.
    pub fn children(index: usize) -> Vec<usize> {
        assert!(index < Self::TABLEAU_SIZE);
        let r = Self::row(index);
        if r >= 3 {
            return vec![];
        }
        let start = Self::ROW_START[r];
        let next_start = Self::ROW_START[r + 1];
        let col_in_row = index - start;

        match r {
            0 => match col_in_row {
                0 => vec![next_start, next_start + 1],     // 0 → [3, 4]
                1 => vec![next_start + 2, next_start + 3], // 1 → [5, 6]
                2 => vec![next_start + 4, next_start + 5], // 2 → [7, 8]
                _ => vec![],
            },
            1 => match col_in_row {
                0 => vec![next_start, next_start + 1],     // 3  → [9, 10]
                1 => vec![next_start + 1, next_start + 2], // 4  → [10, 11]
                2 => vec![next_start + 3, next_start + 4], // 5  → [12, 13]
                3 => vec![next_start + 4, next_start + 5], // 6  → [13, 14]
                4 => vec![next_start + 6, next_start + 7], // 7  → [15, 16]
                5 => vec![next_start + 7, next_start + 8], // 8  → [16, 17]
                _ => vec![],
            },
            2 => match col_in_row {
                0 => vec![next_start, next_start + 1],     // 9  → [18, 19]
                1 => vec![next_start + 1, next_start + 2], // 10 → [19, 20]
                2 => vec![next_start + 2, next_start + 3], // 11 → [20, 21]
                3 => vec![next_start + 3, next_start + 4], // 12 → [21, 22]
                4 => vec![next_start + 4, next_start + 5], // 13 → [22, 23]
                5 => vec![next_start + 5, next_start + 6], // 14 → [23, 24]
                6 => vec![next_start + 6, next_start + 7], // 15 → [24, 25]
                7 => vec![next_start + 7, next_start + 8], // 16 → [25, 26]
                8 => vec![next_start + 8, next_start + 9], // 17 → [26, 27]
                _ => vec![],
            },
            _ => vec![],
        }
    }

    /// True if the position has a card and all cards covering it (lower row) were removed.
    pub fn is_exposed(&self, index: usize) -> bool {
        if index >= self.tableau.len() || self.tableau[index].is_none() {
            return false;
        }
        let kids = Self::children(index);
        kids.iter().all(|&c| self.tableau[c].is_none())
    }

    /// List of currently exposed tableau indices.
    pub fn exposed_tableau_indices(&self) -> Vec<usize> {
        (0..self.tableau.len())
            .filter(|&i| self.is_exposed(i))
            .collect()
    }

    /// Returns the top waste card, if any.
    pub fn waste_top(&self) -> Option<Card> {
        self.waste.last().copied()
    }

    /// Indicates whether a card can be drawn from stock.
    pub fn can_draw_from_stock(&self) -> bool {
        !self.stock.is_empty()
    }

    /// Counts how many cards remain in the tableau.
    pub fn remaining_tableau(&self) -> usize {
        self.tableau.iter().filter(|c| c.is_some()).count()
    }
}

/// Checks adjacency: values differ by 1, with King-Ace wrap.
pub fn is_adjacent(card: Card, waste_top: Card) -> bool {
    let a = card.value as i32;
    let b = waste_top.value as i32;
    if (a - b).abs() == 1 {
        return true;
    }
    if (a == 1 && b == 13) || (a == 13 && b == 1) {
        return true;
    }
    false
}

/// TriPeaks moves.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TriPeaksMove {
    /// Deals the initial deck.
    Deal { deck: Vec<Card> },
    /// Moves an exposed tableau card to waste.
    TableauToWaste { tableau_index: usize },
    /// Draws a card from stock to waste.
    DrawFromStock,
}

impl TriPeaksMove {
    /// True if the move removes a tableau card.
    pub fn is_tableau_move(&self) -> bool {
        matches!(self, TriPeaksMove::TableauToWaste { .. })
    }

    /// Applies a move to a board and returns the new state if valid.
    pub fn apply(&self, board: &TriPeaksBoard) -> Option<TriPeaksBoard> {
        match self {
            TriPeaksMove::Deal { deck } => apply_deal(deck),

            TriPeaksMove::TableauToWaste { tableau_index } => {
                let idx = *tableau_index;
                if idx >= board.tableau.len() {
                    return None;
                }
                let card = board.tableau[idx]?;
                if !board.is_exposed(idx) {
                    return None;
                }
                let waste_top = board.waste_top()?;
                if !is_adjacent(card, waste_top) {
                    return None;
                }

                let mut new_tableau = board.tableau.clone();
                new_tableau[idx] = None;
                let mut new_waste = board.waste.clone();
                new_waste.push(card);
                Some(TriPeaksBoard::new(
                    new_tableau,
                    board.stock.clone(),
                    new_waste,
                ))
            }

            TriPeaksMove::DrawFromStock => {
                if !board.can_draw_from_stock() {
                    return None;
                }
                let card = *board.stock.last()?;
                let new_stock = board.stock[..board.stock.len() - 1].to_vec();
                let mut new_waste = board.waste.clone();
                new_waste.push(card);
                Some(TriPeaksBoard::new(
                    board.tableau.clone(),
                    new_stock,
                    new_waste,
                ))
            }
        }
    }
}

/// Applies the initial deal logic for TriPeaks.
fn apply_deal(deck: &[Card]) -> Option<TriPeaksBoard> {
    if deck.len() != 52 {
        return None;
    }
    let mut tableau: Vec<Option<Card>> = vec![None; TriPeaksBoard::TABLEAU_SIZE];
    for i in 0..TriPeaksBoard::TABLEAU_SIZE {
        tableau[i] = Some(deck[i]);
    }
    let remaining = &deck[TriPeaksBoard::TABLEAU_SIZE..];
    if remaining.is_empty() {
        return None;
    }
    let stock = remaining[..remaining.len() - 1].to_vec();
    let waste = vec![*remaining.last().unwrap()];
    Some(TriPeaksBoard::new(tableau, stock, waste))
}

// -- Move generation --

impl TriPeaksMove {
    /// Generates valid moves from exposed tableau cards to waste.
    pub fn find_tableau_to_waste_moves(board: &TriPeaksBoard) -> Vec<TriPeaksMove> {
        let waste_top = match board.waste_top() {
            Some(c) => c,
            None => return vec![],
        };
        let mut moves = Vec::new();
        for idx in board.exposed_tableau_indices() {
            if let Some(card) = board.tableau[idx] {
                if is_adjacent(card, waste_top) {
                    moves.push(TriPeaksMove::TableauToWaste { tableau_index: idx });
                }
            }
        }
        moves
    }

    /// Only allows drawing from stock when no tableau moves are available.
    pub fn find_draw_from_stock_moves(board: &TriPeaksBoard) -> Vec<TriPeaksMove> {
        if !board.can_draw_from_stock() {
            return vec![];
        }
        let tableau_moves = Self::find_tableau_to_waste_moves(board);
        if !tableau_moves.is_empty() {
            return vec![];
        }
        vec![TriPeaksMove::DrawFromStock]
    }
}

// Custom equality that ignores `signature`.
impl PartialEq for TriPeaksBoard {
    fn eq(&self, other: &Self) -> bool {
        self.tableau == other.tableau && self.stock == other.stock && self.waste == other.waste
    }
}

impl Eq for TriPeaksBoard {}

impl std::hash::Hash for TriPeaksBoard {
    /// Hash based on the precomputed state signature.
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.signature);
    }
}

/// Computes an FNV-1a signature of the full state (tableau, stock, and waste).
fn compute_signature(tableau: &[Option<Card>], stock: &[Card], waste: &[Card]) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;

    let mut h: u64 = FNV_OFFSET;

    for (idx, card) in tableau.iter().enumerate() {
        h ^= (idx as u64).wrapping_add(0x100);
        h = h.wrapping_mul(FNV_PRIME);
        h ^= encode_opt_card(*card);
        h = h.wrapping_mul(FNV_PRIME);
    }

    h ^= 0xFD; // separator
    h = h.wrapping_mul(FNV_PRIME);

    for card in stock {
        h ^= encode_card(*card);
        h = h.wrapping_mul(FNV_PRIME);
    }

    h ^= 0xFE; // separator
    h = h.wrapping_mul(FNV_PRIME);

    for card in waste {
        h ^= encode_card(*card);
        h = h.wrapping_mul(FNV_PRIME);
    }

    h
}

#[inline]
/// Encodes a card into a compact integer for the signature.
fn encode_card(card: Card) -> u64 {
    let suit_val: u64 = card.suit as u64;
    ((card.value as u64) << 4) | suit_val
}

#[inline]
/// Encodes an optional card; uses a sentinel for `None`.
fn encode_opt_card(card: Option<Card>) -> u64 {
    match card {
        Some(c) => encode_card(c),
        None => 0xFF,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::card::Suit;

    fn card(value: u8) -> Card {
        Card::new(Suit::Club, value)
    }

    fn board_with_values(
        values: [u8; TriPeaksBoard::TABLEAU_SIZE],
        stock: Vec<Card>,
        waste: Vec<Card>,
    ) -> TriPeaksBoard {
        TriPeaksBoard::new(
            values.into_iter().map(|value| Some(card(value))).collect(),
            stock,
            waste,
        )
    }

    fn simple_board() -> TriPeaksBoard {
        board_with_values(
            [
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
                13, 1, 2,
            ],
            vec![card(9), card(10)],
            vec![card(5)],
        )
    }

    fn remove_indices(board: &mut TriPeaksBoard, indices: &[usize]) {
        for &idx in indices {
            board.tableau[idx] = None;
        }
        board.signature = compute_signature(&board.tableau, &board.stock, &board.waste);
    }

    #[test]
    fn row_maps_all_tableau_indices() {
        for idx in 0..=2 {
            assert_eq!(TriPeaksBoard::row(idx), 0);
        }
        for idx in 3..=8 {
            assert_eq!(TriPeaksBoard::row(idx), 1);
        }
        for idx in 9..=17 {
            assert_eq!(TriPeaksBoard::row(idx), 2);
        }
        for idx in 18..=27 {
            assert_eq!(TriPeaksBoard::row(idx), 3);
        }
    }

    #[test]
    fn parents_mapping_matches_layout() {
        assert_eq!(TriPeaksBoard::parents(0), Vec::<usize>::new());
        assert_eq!(TriPeaksBoard::parents(3), vec![0]);
        assert_eq!(TriPeaksBoard::parents(4), vec![0]);
        assert_eq!(TriPeaksBoard::parents(10), vec![3, 4]);
        assert_eq!(TriPeaksBoard::parents(17), vec![8]);
        assert_eq!(TriPeaksBoard::parents(18), vec![9]);
        assert_eq!(TriPeaksBoard::parents(27), vec![17]);
    }

    #[test]
    fn children_mapping_matches_layout() {
        assert_eq!(TriPeaksBoard::children(0), vec![3, 4]);
        assert_eq!(TriPeaksBoard::children(1), vec![5, 6]);
        assert_eq!(TriPeaksBoard::children(2), vec![7, 8]);
        assert_eq!(TriPeaksBoard::children(3), vec![9, 10]);
        assert_eq!(TriPeaksBoard::children(8), vec![16, 17]);
        assert_eq!(TriPeaksBoard::children(17), vec![26, 27]);
        assert_eq!(TriPeaksBoard::children(18), Vec::<usize>::new());
    }

    #[test]
    fn exposed_indices_only_include_unblocked_cards() {
        let mut board = simple_board();

        assert_eq!(
            board.exposed_tableau_indices(),
            (18..=27).collect::<Vec<_>>()
        );
        assert!(!board.is_exposed(9));
        assert!(!board.is_exposed(0));

        remove_indices(&mut board, &[18, 19]);
        assert!(board.is_exposed(9));
        assert!(!board.is_exposed(10));

        remove_indices(&mut board, &[3, 4]);
        assert!(board.is_exposed(0));
    }

    #[test]
    fn is_adjacent_handles_regular_and_ace_king_wrap() {
        assert!(is_adjacent(card(5), card(6)));
        assert!(is_adjacent(card(6), card(5)));
        assert!(is_adjacent(card(1), card(13)));
        assert!(is_adjacent(card(13), card(1)));
        assert!(!is_adjacent(card(5), card(5)));
        assert!(!is_adjacent(card(5), card(7)));
    }

    #[test]
    fn apply_tableau_to_waste_rejects_unexposed_or_non_adjacent() {
        let board = simple_board();

        assert_eq!(
            TriPeaksMove::TableauToWaste { tableau_index: 0 }.apply(&board),
            None
        );
        assert_eq!(
            TriPeaksMove::TableauToWaste { tableau_index: 20 }.apply(&board),
            None
        );
        assert_eq!(
            TriPeaksMove::TableauToWaste {
                tableau_index: TriPeaksBoard::TABLEAU_SIZE
            }
            .apply(&board),
            None
        );
    }

    #[test]
    fn apply_tableau_to_waste_removes_card_and_pushes_waste() {
        let board = simple_board();
        let moved = TriPeaksMove::TableauToWaste { tableau_index: 18 }
            .apply(&board)
            .expect("exposed adjacent card should move to waste");

        assert_eq!(moved.tableau[18], None);
        assert_eq!(moved.waste, vec![card(5), card(6)]);
        assert_eq!(moved.stock, board.stock);
        assert_eq!(moved.remaining_tableau(), TriPeaksBoard::TABLEAU_SIZE - 1);
        assert_ne!(moved.signature, board.signature);
    }

    #[test]
    fn draw_from_stock_moves_only_when_no_tableau_moves() {
        let mut board = board_with_values([5; TriPeaksBoard::TABLEAU_SIZE], vec![], vec![card(9)]);
        board.waste = vec![card(5)];
        board.stock = vec![card(11), card(12)];
        board.signature = compute_signature(&board.tableau, &board.stock, &board.waste);

        assert!(TriPeaksMove::find_tableau_to_waste_moves(&board).is_empty());
        assert_eq!(
            TriPeaksMove::find_draw_from_stock_moves(&board),
            vec![TriPeaksMove::DrawFromStock]
        );

        let drawn = TriPeaksMove::DrawFromStock
            .apply(&board)
            .expect("stock card should draw to waste");
        assert_eq!(drawn.stock, vec![card(11)]);
        assert_eq!(drawn.waste, vec![card(5), card(12)]);

        let mut board_with_tableau_move = simple_board();
        board_with_tableau_move.waste = vec![card(5)];
        assert_eq!(
            TriPeaksMove::find_tableau_to_waste_moves(&board_with_tableau_move),
            vec![TriPeaksMove::TableauToWaste { tableau_index: 18 }]
        );
        assert!(TriPeaksMove::find_draw_from_stock_moves(&board_with_tableau_move).is_empty());
    }

    #[test]
    fn deal_rejects_wrong_deck_size_and_initializes_stock_waste() {
        let deck = Card::standard_deck();

        assert_eq!(
            TriPeaksMove::Deal {
                deck: deck[..51].to_vec()
            }
            .apply(&simple_board()),
            None
        );

        let board = TriPeaksMove::Deal { deck: deck.clone() }
            .apply(&simple_board())
            .expect("standard deck should deal");

        assert_eq!(board.tableau.len(), TriPeaksBoard::TABLEAU_SIZE);
        assert_eq!(board.tableau[0], Some(deck[0]));
        assert_eq!(board.tableau[27], Some(deck[27]));
        assert_eq!(board.stock, deck[28..51].to_vec());
        assert_eq!(board.waste, vec![deck[51]]);
    }

    #[test]
    fn signature_changes_when_state_changes() {
        let board = simple_board();

        let mut without_tableau_card = board.clone();
        remove_indices(&mut without_tableau_card, &[18]);

        let mut with_different_stock = board.clone();
        with_different_stock.stock.pop();
        with_different_stock.signature = compute_signature(
            &with_different_stock.tableau,
            &with_different_stock.stock,
            &with_different_stock.waste,
        );

        let mut with_different_waste = board.clone();
        with_different_waste.waste.push(card(6));
        with_different_waste.signature = compute_signature(
            &with_different_waste.tableau,
            &with_different_waste.stock,
            &with_different_waste.waste,
        );

        assert_ne!(board.signature, without_tableau_card.signature);
        assert_ne!(board.signature, with_different_stock.signature);
        assert_ne!(board.signature, with_different_waste.signature);
    }
}
