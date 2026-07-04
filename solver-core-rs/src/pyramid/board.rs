use crate::common::card::Card;
use std::collections::HashSet;

/// Pyramid board: 28 pyramid positions, stock, waste, and foundation.
#[derive(Debug, Clone)]
pub struct PyramidBoard {
    /// 28 positions. Some = card present, None = removed.
    pub pyramid: Vec<Option<Card>>,
    /// Face-down stock (top = last).
    pub stock: Vec<Card>,
    /// Face-up waste (top = last).
    pub waste: Vec<Card>,
    /// Removed cards.
    pub foundation: Vec<Card>,
    /// Precomputed state hash signature.
    pub signature: u64,
}

impl PyramidBoard {
    /// Total number of pyramid positions (7 rows = 28 cards).
    pub const PYRAMID_SIZE: usize = 28;

    /// Creates a Pyramid board and computes its initial hash signature.
    pub fn new(
        pyramid: Vec<Option<Card>>,
        stock: Vec<Card>,
        waste: Vec<Card>,
        foundation: Vec<Card>,
    ) -> PyramidBoard {
        assert_eq!(pyramid.len(), Self::PYRAMID_SIZE);
        let signature = compute_signature(&pyramid, &stock, &waste, &foundation);
        PyramidBoard {
            pyramid,
            stock,
            waste,
            foundation,
            signature,
        }
    }

    /// Row for the given index (0-6).
    pub fn row(index: usize) -> usize {
        assert!(index < Self::PYRAMID_SIZE);
        let mut row = 0;
        let mut start = 0;
        while start + (row + 1) <= index {
            start += row + 1;
            row += 1;
        }
        row
    }

    /// Left child index, if any.
    pub fn left_child(index: usize) -> Option<usize> {
        let row = Self::row(index);
        if row >= 6 {
            return None;
        }
        let column = index - (row * (row + 1)) / 2;
        Some(((row + 1) * (row + 2)) / 2 + column)
    }

    /// Right child index, if any.
    pub fn right_child(index: usize) -> Option<usize> {
        Self::left_child(index).map(|l| l + 1)
    }

    /// True if the position has a card and both children are empty.
    pub fn is_exposed(&self, index: usize) -> bool {
        if index >= self.pyramid.len() || self.pyramid[index].is_none() {
            return false;
        }
        let row = Self::row(index);
        if row == 6 {
            return true;
        }
        match (Self::left_child(index), Self::right_child(index)) {
            (Some(left), Some(right)) => {
                self.pyramid[left].is_none() && self.pyramid[right].is_none()
            }
            _ => true,
        }
    }

    /// Returns the currently exposed pyramid indices.
    pub fn exposed_pyramid_indices(&self) -> Vec<usize> {
        (0..self.pyramid.len())
            .filter(|&i| self.is_exposed(i))
            .collect()
    }

    /// Top waste card.
    pub fn waste_top(&self) -> Option<Card> {
        self.waste.last().copied()
    }
    /// Top stock card.
    pub fn stock_top(&self) -> Option<Card> {
        self.stock.last().copied()
    }
    /// Indicates whether stock can advance to waste.
    pub fn can_advance_stock(&self) -> bool {
        !self.stock.is_empty()
    }
    /// Indicates whether waste can be recycled to stock.
    pub fn can_reset_stock(&self) -> bool {
        self.stock.is_empty() && !self.waste.is_empty()
    }

    /// Number of remaining cards in the pyramid.
    pub fn remaining_pyramid(&self) -> usize {
        self.pyramid.iter().filter(|c| c.is_some()).count()
    }
}

/// Pyramid moves.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PyramidMove {
    Deal { deck: Vec<Card> },
    StockAdvance,
    StockReset,
    RemovePairWastePyramid { pyramid_index: usize },
    RemovePairPyramidPyramid { i: usize, j: usize },
    KingToFoundationPyramid { index: usize },
    KingToFoundationWaste,
}

impl PyramidMove {
    /// True if the move recycles stock.
    pub fn is_stock_reset(&self) -> bool {
        matches!(self, PyramidMove::StockReset)
    }
    /// True if the move advances stock.
    pub fn is_stock_advance(&self) -> bool {
        matches!(self, PyramidMove::StockAdvance)
    }
    /// True if the move removes a King.
    pub fn is_king_move(&self) -> bool {
        matches!(
            self,
            PyramidMove::KingToFoundationPyramid { .. } | PyramidMove::KingToFoundationWaste
        )
    }
    /// True if the move removes a pair summing to 13.
    pub fn is_pair_move(&self) -> bool {
        matches!(
            self,
            PyramidMove::RemovePairWastePyramid { .. }
                | PyramidMove::RemovePairPyramidPyramid { .. }
        )
    }

    /// Applies the move to a board.
    pub fn apply(&self, board: &PyramidBoard) -> Option<PyramidBoard> {
        match self {
            PyramidMove::Deal { deck } => apply_py_deal(deck),

            PyramidMove::StockAdvance => {
                if !board.can_advance_stock() {
                    return None;
                }
                let card = *board.stock.last()?;
                let new_stock = board.stock[..board.stock.len() - 1].to_vec();
                let mut new_waste = board.waste.clone();
                new_waste.push(card);
                Some(PyramidBoard::new(
                    board.pyramid.clone(),
                    new_stock,
                    new_waste,
                    board.foundation.clone(),
                ))
            }

            PyramidMove::StockReset => {
                if !board.can_reset_stock() {
                    return None;
                }
                let new_stock: Vec<Card> = board.waste.iter().rev().copied().collect();
                Some(PyramidBoard::new(
                    board.pyramid.clone(),
                    new_stock,
                    vec![],
                    board.foundation.clone(),
                ))
            }

            PyramidMove::RemovePairWastePyramid { pyramid_index } => {
                let idx = *pyramid_index;
                let waste_card = board.waste_top()?;
                let pyramid_card = board.pyramid.get(idx)?.as_ref()?;
                if !board.is_exposed(idx) {
                    return None;
                }
                if waste_card.value + pyramid_card.value != 13 {
                    return None;
                }

                let mut new_pyramid = board.pyramid.clone();
                new_pyramid[idx] = None;
                let new_waste = board.waste[..board.waste.len() - 1].to_vec();
                let mut new_found = board.foundation.clone();
                new_found.push(waste_card);
                new_found.push(*pyramid_card);
                Some(PyramidBoard::new(
                    new_pyramid,
                    board.stock.clone(),
                    new_waste,
                    new_found,
                ))
            }

            PyramidMove::RemovePairPyramidPyramid { i, j } => {
                let (i, j) = (*i, *j);
                if i == j || i >= board.pyramid.len() || j >= board.pyramid.len() {
                    return None;
                }
                let card_a = board.pyramid[i]?;
                let card_b = board.pyramid[j]?;
                if !can_remove_pair(board, i, j) {
                    return None;
                }
                if card_a.value + card_b.value != 13 {
                    return None;
                }

                let mut new_pyramid = board.pyramid.clone();
                new_pyramid[i] = None;
                new_pyramid[j] = None;
                let mut new_found = board.foundation.clone();
                new_found.push(card_a);
                new_found.push(card_b);
                Some(PyramidBoard::new(
                    new_pyramid,
                    board.stock.clone(),
                    board.waste.clone(),
                    new_found,
                ))
            }

            PyramidMove::KingToFoundationPyramid { index } => {
                let idx = *index;
                let card = board.pyramid.get(idx)?.as_ref()?;
                if card.value != 13 || !board.is_exposed(idx) {
                    return None;
                }

                let mut new_pyramid = board.pyramid.clone();
                new_pyramid[idx] = None;
                let mut new_found = board.foundation.clone();
                new_found.push(*card);
                Some(PyramidBoard::new(
                    new_pyramid,
                    board.stock.clone(),
                    board.waste.clone(),
                    new_found,
                ))
            }

            PyramidMove::KingToFoundationWaste => {
                let card = board.waste_top()?;
                if card.value != 13 {
                    return None;
                }
                let new_waste = board.waste[..board.waste.len() - 1].to_vec();
                let mut new_found = board.foundation.clone();
                new_found.push(card);
                Some(PyramidBoard::new(
                    board.pyramid.clone(),
                    board.stock.clone(),
                    new_waste,
                    new_found,
                ))
            }
        }
    }
}

fn apply_py_deal(deck: &[Card]) -> Option<PyramidBoard> {
    // Deals 28 cards to the pyramid and leaves the rest in stock.
    if deck.len() != 52 {
        return None;
    }
    let mut pyramid: Vec<Option<Card>> = vec![None; PyramidBoard::PYRAMID_SIZE];
    for i in 0..PyramidBoard::PYRAMID_SIZE {
        pyramid[i] = Some(deck[i]);
    }
    let stock = deck[PyramidBoard::PYRAMID_SIZE..].to_vec();
    Some(PyramidBoard::new(pyramid, stock, vec![], vec![]))
}

/// Allows removing a pair where one card may be covered by the other.
fn can_remove_pair(board: &PyramidBoard, first: usize, second: usize) -> bool {
    if board.is_exposed(first) && board.is_exposed(second) {
        return true;
    }
    if can_use_covered_when_source_removed(board, first, second) {
        return true;
    }
    can_use_covered_when_source_removed(board, second, first)
}

/// Special rule: a covered card can be removed if its only blocker is the source card.
fn can_use_covered_when_source_removed(board: &PyramidBoard, source: usize, dest: usize) -> bool {
    if !board.is_exposed(source) {
        return false;
    }
    if board.is_exposed(dest) {
        return false;
    }

    let (left, right) = match (
        PyramidBoard::left_child(dest),
        PyramidBoard::right_child(dest),
    ) {
        (Some(l), Some(r)) => (l, r),
        _ => return false,
    };

    if left != source && right != source {
        return false;
    }

    let blockers: Vec<usize> = [left, right]
        .iter()
        .filter(|&&idx| board.pyramid[idx].is_some())
        .copied()
        .collect();
    blockers.len() == 1 && blockers[0] == source
}

// -- Move generation --

impl PyramidMove {
    /// Generates the stock advance move if available.
    pub fn find_stock_advance_moves(board: &PyramidBoard) -> Vec<PyramidMove> {
        if board.can_advance_stock() {
            vec![PyramidMove::StockAdvance]
        } else {
            vec![]
        }
    }

    /// Generates the stock recycle move if available.
    pub fn find_stock_reset_moves(board: &PyramidBoard) -> Vec<PyramidMove> {
        if board.can_reset_stock() {
            vec![PyramidMove::StockReset]
        } else {
            vec![]
        }
    }

    /// Generates moves to remove exposed Kings (pyramid or waste).
    pub fn find_king_moves(board: &PyramidBoard) -> Vec<PyramidMove> {
        let mut moves = Vec::new();
        for idx in board.exposed_pyramid_indices() {
            if let Some(card) = board.pyramid[idx] {
                if card.value == 13 {
                    moves.push(PyramidMove::KingToFoundationPyramid { index: idx });
                }
            }
        }
        if let Some(card) = board.waste_top() {
            if card.value == 13 {
                moves.push(PyramidMove::KingToFoundationWaste);
            }
        }
        moves
    }

    /// Generates valid waste-pyramid pairs that sum to 13.
    pub fn find_waste_pyramid_pairs(board: &PyramidBoard) -> Vec<PyramidMove> {
        let waste_card = match board.waste_top() {
            Some(c) => c,
            None => return vec![],
        };
        let target = 13u8.checked_sub(waste_card.value);
        let target = match target {
            Some(t) if t >= 1 && t <= 13 => t,
            _ => return vec![],
        };

        let mut moves = Vec::new();
        for idx in board.exposed_pyramid_indices() {
            if let Some(card) = board.pyramid[idx] {
                if card.value == target {
                    moves.push(PyramidMove::RemovePairWastePyramid { pyramid_index: idx });
                }
            }
        }
        moves
    }

    /// Generates valid pairs within the pyramid that sum to 13.
    pub fn find_pyramid_pyramid_pairs(board: &PyramidBoard) -> Vec<PyramidMove> {
        let exposed = board.exposed_pyramid_indices();
        if exposed.is_empty() {
            return vec![];
        }

        let mut moves = Vec::new();
        let mut seen: HashSet<(usize, usize)> = HashSet::new();

        for &source in &exposed {
            let source_card = match board.pyramid[source] {
                Some(c) => c,
                None => continue,
            };
            for dest in 0..board.pyramid.len() {
                if dest == source {
                    continue;
                }
                let dest_card = match board.pyramid[dest] {
                    Some(c) => c,
                    None => continue,
                };
                if source_card.value + dest_card.value != 13 {
                    continue;
                }
                if !can_remove_pair(board, source, dest) {
                    continue;
                }

                let key = if source < dest {
                    (source, dest)
                } else {
                    (dest, source)
                };
                if seen.insert(key) {
                    moves.push(PyramidMove::RemovePairPyramidPyramid { i: source, j: dest });
                }
            }
        }
        moves
    }

    /// Finds all available moves.
    pub fn find_all_moves(board: &PyramidBoard) -> Vec<PyramidMove> {
        let mut moves = Vec::new();
        moves.extend(Self::find_stock_reset_moves(board));
        moves.extend(Self::find_stock_advance_moves(board));
        moves.extend(Self::find_king_moves(board));
        moves.extend(Self::find_waste_pyramid_pairs(board));
        moves.extend(Self::find_pyramid_pyramid_pairs(board));
        moves
    }
}

// Custom equality that ignores the signature.
impl PartialEq for PyramidBoard {
    fn eq(&self, other: &Self) -> bool {
        self.pyramid == other.pyramid
            && self.stock == other.stock
            && self.waste == other.waste
            && self.foundation == other.foundation
    }
}

impl Eq for PyramidBoard {}

impl std::hash::Hash for PyramidBoard {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.signature);
    }
}

fn compute_signature(
    pyramid: &[Option<Card>],
    stock: &[Card],
    waste: &[Card],
    foundation: &[Card],
) -> u64 {
    // 64-bit FNV-1a for a stable and fast board-state signature.
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;

    let mut h: u64 = FNV_OFFSET;

    for (idx, card) in pyramid.iter().enumerate() {
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

    h ^= 0xFC; // separator
    h = h.wrapping_mul(FNV_PRIME);

    for card in foundation {
        h ^= encode_card(*card);
        h = h.wrapping_mul(FNV_PRIME);
    }

    h
}

#[inline]
fn encode_card(card: Card) -> u64 {
    // Compact encoding: value in high bits, suit in low bits.
    let suit_val: u64 = card.suit as u64;
    ((card.value as u64) << 4) | suit_val
}

#[inline]
fn encode_opt_card(card: Option<Card>) -> u64 {
    // 0xFF represents absence of a card.
    match card {
        Some(c) => encode_card(c),
        None => 0xFF,
    }
}
