use crate::common::card::{Card, Suit};

/// FreeCell board: 4 free cells, 4 foundations, and 8 tableau columns.
#[derive(Debug, Clone)]
pub struct FreeCellBoard {
    /// 4 free cell slots: `None` means empty.
    pub free_cells: [Option<Card>; 4],
    /// Foundation pile indexed by suit ordinal.
    pub foundation: [Vec<Card>; 4],
    /// 8 tableau columns.
    pub tableau: [Vec<Card>; 8],
    pub signature: u64,
}

impl FreeCellBoard {
    // FreeCell board-size constants.
    pub const NUM_FREE_CELLS: usize = 4;
    pub const NUM_TABLEAU: usize = 8;

    // Main constructor: creates a board and computes its canonical signature.
    pub fn new(
        free_cells: [Option<Card>; 4],
        foundation: [Vec<Card>; 4],
        tableau: [Vec<Card>; 8],
    ) -> FreeCellBoard {
        let signature = compute_signature(&free_cells, &foundation, &tableau);
        FreeCellBoard {
            free_cells,
            foundation,
            tableau,
            signature,
        }
    }

    pub fn top_of_foundation(&self, suit: Suit) -> Option<Card> {
        // Returns the top foundation card for the requested suit.
        self.foundation[suit as usize].last().copied()
    }

    pub fn foundation_count(&self, suit: Suit) -> usize {
        // Number of cards in one suit foundation.
        self.foundation[suit as usize].len()
    }

    pub fn total_foundation_count(&self) -> usize {
        // Total accumulated across the 4 foundations.
        self.foundation.iter().map(|s| s.len()).sum()
    }

    pub fn can_add_to_foundation(&self, card: Card) -> bool {
        // A foundation only accepts the next card of its suit (A,2,3,...,K).
        let count = self.foundation[card.suit as usize].len();
        card.value as usize == count + 1
    }

    pub fn foundation_plus_card(&self, card: Card) -> [Vec<Card>; 4] {
        // Returns a copy of foundations with the card added to its suit.
        let mut f = self.foundation.clone();
        f[card.suit as usize].push(card);
        f
    }

    pub fn extract_from_foundation(&self, suit: Suit) -> Option<(Card, [Vec<Card>; 4])> {
        // Extracts the top card from the suit foundation and returns (card, new foundations).
        let mut f = self.foundation.clone();
        let card = f[suit as usize].pop()?;
        Some((card, f))
    }

    pub fn top_of_tableau(&self, i: usize) -> Option<Card> {
        // Returns the top card of the tableau column.
        self.tableau[i].last().copied()
    }

    /// Checks whether a run is valid (descending with alternating colors).
    pub fn is_valid_tableau_run(run: &[Card]) -> bool {
        if run.is_empty() {
            return false;
        }
        for i in 0..run.len() - 1 {
            if run[i + 1].value != run[i].value - 1 {
                return false;
            }
            if run[i + 1].color() == run[i].color() {
                return false;
            }
        }
        true
    }

    /// Indicates whether the run can be placed on the destination column.
    pub fn can_add_run_to_tableau(&self, run: &[Card], col: usize) -> bool {
        let first = match run.first() {
            Some(c) => c,
            None => return false,
        };
        if self.tableau[col].is_empty() {
            return true;
        }
        match self.tableau[col].last() {
            Some(top) => top.value == first.value + 1 && top.color() != first.color(),
            None => true,
        }
    }

    /// Maximum valid run length from the top of the column.
    pub fn max_tableau_run_length(&self, col: usize) -> usize {
        let column = &self.tableau[col];
        if column.is_empty() {
            return 0;
        }
        let mut count = 1;
        for i in (0..column.len() - 1).rev() {
            let curr = &column[i];
            let next = &column[i + 1];
            if next.value != curr.value - 1 || next.color() == curr.color() {
                break;
            }
            count += 1;
        }
        count
    }

    pub fn tableau_run_from_top(&self, col: usize, length: usize) -> Option<Vec<Card>> {
        // Extracts `length` cards from the top and validates that they form a legal run.
        let column = &self.tableau[col];
        if length < 1 || length > column.len() {
            return None;
        }
        let run = column[column.len() - length..].to_vec();
        if Self::is_valid_tableau_run(&run) {
            Some(run)
        } else {
            None
        }
    }

    pub fn empty_free_cells(&self) -> usize {
        // Number of free slots in free cells.
        self.free_cells.iter().filter(|c| c.is_none()).count()
    }

    pub fn empty_tableau_columns(&self) -> usize {
        // Number of empty tableau columns.
        self.tableau.iter().filter(|c| c.is_empty()).count()
    }

    /// FreeCell rule for maximum number of movable cards.
    pub fn max_movable_cards(&self, dest_col: usize) -> usize {
        // Classic formula:
        // (empty free cells + 1) * 2^(usable empty columns)
        let dest_empty = self.tableau[dest_col].is_empty();
        let usable = self
            .empty_tableau_columns()
            .saturating_sub(if dest_empty { 1 } else { 0 });
        let multiplier = 1usize << usable;
        (self.empty_free_cells() + 1) * multiplier
    }
}

/// Possible FreeCell moves.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FreeCellMove {
    Deal {
        deck: Vec<Card>,
    },
    TableauToTableau {
        source: usize,
        destination: usize,
        cards: Vec<Card>,
    },
    TableauToFoundation {
        source: usize,
        card: Card,
    },
    TableauToFreeCell {
        source: usize,
        card: Card,
        cell_index: usize,
    },
    FreeCellToTableau {
        cell_index: usize,
        destination: usize,
        card: Card,
    },
    FreeCellToFoundation {
        cell_index: usize,
        card: Card,
    },
    FoundationToTableau {
        suit: Suit,
        destination: usize,
        card: Card,
    },
}

impl FreeCellMove {
    // Indicates whether the move ends in foundation.
    pub fn is_foundation_move(&self) -> bool {
        matches!(
            self,
            FreeCellMove::TableauToFoundation { .. } | FreeCellMove::FreeCellToFoundation { .. }
        )
    }

    // Indicates whether the move extracts a card from foundation.
    pub fn is_from_foundation(&self) -> bool {
        matches!(self, FreeCellMove::FoundationToTableau { .. })
    }

    pub fn apply(&self, board: &FreeCellBoard) -> Option<FreeCellBoard> {
        // Validates move preconditions and, if valid, returns the next immutable board.
        match self {
            FreeCellMove::Deal { deck } => apply_fc_deal(deck),

            FreeCellMove::TableauToTableau {
                source,
                destination,
                cards,
            } => {
                let src = *source;
                let dest = *destination;
                if src == dest || src >= 8 || dest >= 8 {
                    return None;
                }
                if cards.is_empty() || cards.len() > board.max_movable_cards(dest) {
                    return None;
                }
                let run_from = board.tableau_run_from_top(src, cards.len())?;
                if run_from != *cards {
                    return None;
                }
                if !board.can_add_run_to_tableau(cards, dest) {
                    return None;
                }
                let mut tab = board.tableau.clone();
                let new_len = tab[src].len() - cards.len();
                tab[src].truncate(new_len);
                tab[dest].extend_from_slice(cards);
                Some(FreeCellBoard::new(
                    board.free_cells,
                    board.foundation.clone(),
                    tab,
                ))
            }

            FreeCellMove::TableauToFoundation { source, card } => {
                if board.top_of_tableau(*source)? != *card {
                    return None;
                }
                if !board.can_add_to_foundation(*card) {
                    return None;
                }
                let mut tab = board.tableau.clone();
                tab[*source].pop();
                let new_found = board.foundation_plus_card(*card);
                Some(FreeCellBoard::new(board.free_cells, new_found, tab))
            }

            FreeCellMove::TableauToFreeCell {
                source,
                card,
                cell_index,
            } => {
                if board.top_of_tableau(*source)? != *card {
                    return None;
                }
                if board.free_cells[*cell_index].is_some() {
                    return None;
                }
                let mut tab = board.tableau.clone();
                tab[*source].pop();
                let mut cells = board.free_cells;
                cells[*cell_index] = Some(*card);
                Some(FreeCellBoard::new(cells, board.foundation.clone(), tab))
            }

            FreeCellMove::FreeCellToTableau {
                cell_index,
                destination,
                card,
            } => {
                if board.free_cells[*cell_index]? != *card {
                    return None;
                }
                if !board.can_add_run_to_tableau(&[*card], *destination) {
                    return None;
                }
                let mut cells = board.free_cells;
                cells[*cell_index] = None;
                let mut tab = board.tableau.clone();
                tab[*destination].push(*card);
                Some(FreeCellBoard::new(cells, board.foundation.clone(), tab))
            }

            FreeCellMove::FreeCellToFoundation { cell_index, card } => {
                if board.free_cells[*cell_index]? != *card {
                    return None;
                }
                if !board.can_add_to_foundation(*card) {
                    return None;
                }
                let mut cells = board.free_cells;
                cells[*cell_index] = None;
                let new_found = board.foundation_plus_card(*card);
                Some(FreeCellBoard::new(cells, new_found, board.tableau.clone()))
            }

            FreeCellMove::FoundationToTableau {
                suit,
                destination,
                card,
            } => {
                if board.top_of_foundation(*suit)? != *card {
                    return None;
                }
                if !board.can_add_run_to_tableau(&[*card], *destination) {
                    return None;
                }
                let (_, new_found) = board.extract_from_foundation(*suit)?;
                let mut tab = board.tableau.clone();
                tab[*destination].push(*card);
                Some(FreeCellBoard::new(board.free_cells, new_found, tab))
            }
        }
    }
}

fn apply_fc_deal(deck: &[Card]) -> Option<FreeCellBoard> {
    if deck.len() != 52 {
        return None;
    }
    let mut tableau: [Vec<Card>; 8] = Default::default();
    // Deals by rows across 8 columns:
    // columns 0..3 receive 7 cards and 4..7 receive 6 cards
    // (classic FreeCell pattern: 7/7/7/7/6/6/6/6).
    for c in 0..8 {
        let num_cards = if c < 4 { 7 } else { 6 };
        for row in 0..num_cards {
            let idx = c + row * 8;
            if idx >= deck.len() {
                return None;
            }
            tableau[c].push(deck[idx]);
        }
    }
    Some(FreeCellBoard::new(
        [None; 4],
        [vec![], vec![], vec![], vec![]],
        tableau,
    ))
}

// -- Move generation --

impl FreeCellMove {
    pub fn find_tableau_to_foundation(board: &FreeCellBoard) -> Vec<FreeCellMove> {
        let mut moves = Vec::new();
        for i in 0..8 {
            if let Some(card) = board.top_of_tableau(i) {
                if board.can_add_to_foundation(card) {
                    moves.push(FreeCellMove::TableauToFoundation { source: i, card });
                }
            }
        }
        moves
    }

    pub fn find_freecell_to_foundation(board: &FreeCellBoard) -> Vec<FreeCellMove> {
        let mut moves = Vec::new();
        for i in 0..4 {
            if let Some(card) = board.free_cells[i] {
                if board.can_add_to_foundation(card) {
                    moves.push(FreeCellMove::FreeCellToFoundation {
                        cell_index: i,
                        card,
                    });
                }
            }
        }
        moves
    }

    pub fn find_tableau_to_freecell(board: &FreeCellBoard) -> Vec<FreeCellMove> {
        // Enumerates moves from tableau tops to any available free cell.
        let mut moves = Vec::new();
        // Fast pruning: if there are no free slots, no free-cell moves exist.
        if board.empty_free_cells() == 0 {
            return moves;
        }
        // Generates all single-card top(tableau) -> each empty free cell moves.
        for src in 0..8 {
            if let Some(card) = board.top_of_tableau(src) {
                for ci in 0..4 {
                    if board.free_cells[ci].is_none() {
                        moves.push(FreeCellMove::TableauToFreeCell {
                            source: src,
                            card,
                            cell_index: ci,
                        });
                    }
                }
            }
        }
        moves
    }

    pub fn find_freecell_to_tableau(board: &FreeCellBoard) -> Vec<FreeCellMove> {
        let mut moves = Vec::new();
        // For each card in a free cell, try all valid tableau destinations.
        for ci in 0..4 {
            if let Some(card) = board.free_cells[ci] {
                for dest in 0..8 {
                    if board.can_add_run_to_tableau(&[card], dest) {
                        moves.push(FreeCellMove::FreeCellToTableau {
                            cell_index: ci,
                            destination: dest,
                            card,
                        });
                    }
                }
            }
        }
        moves
    }

    pub fn find_tableau_to_tableau(board: &FreeCellBoard) -> Vec<FreeCellMove> {
        // Enumerates column transfers considering valid runs and maximum mobility.
        let mut moves = Vec::new();
        for src in 0..8 {
            let max_run = board.max_tableau_run_length(src);
            if max_run < 1 {
                continue;
            }
            for dest in 0..8 {
                if dest == src {
                    continue;
                }
                // Cannot move more than allowed by the mobility rule.
                let allowed = max_run.min(board.max_movable_cards(dest));
                if allowed == 0 {
                    continue;
                }
                // Try each valid run length from 1..=allowed.
                for length in 1..=allowed {
                    if let Some(run) = board.tableau_run_from_top(src, length) {
                        if board.can_add_run_to_tableau(&run, dest) {
                            moves.push(FreeCellMove::TableauToTableau {
                                source: src,
                                destination: dest,
                                cards: run,
                            });
                        }
                    }
                }
            }
        }
        moves
    }

    pub fn find_foundation_to_tableau(board: &FreeCellBoard) -> Vec<FreeCellMove> {
        // Enumerates rollback moves from foundation to tableau.
        let mut moves = Vec::new();
        // Generates rollback moves (foundation -> tableau):
        // take each foundation top and try all valid destinations.
        for &suit in &Suit::ALL {
            if let Some(card) = board.top_of_foundation(suit) {
                for dest in 0..8 {
                    if board.can_add_run_to_tableau(&[card], dest) {
                        moves.push(FreeCellMove::FoundationToTableau {
                            suit,
                            destination: dest,
                            card,
                        });
                    }
                }
            }
        }
        moves
    }
}

// Custom equality that ignores the precomputed signature.
impl PartialEq for FreeCellBoard {
    fn eq(&self, other: &Self) -> bool {
        self.free_cells == other.free_cells
            && self.foundation == other.foundation
            && self.tableau == other.tableau
    }
}

impl Eq for FreeCellBoard {}

impl std::hash::Hash for FreeCellBoard {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.signature);
    }
}

fn compute_signature(
    free_cells: &[Option<Card>; 4],
    foundation: &[Vec<Card>; 4],
    tableau: &[Vec<Card>; 8],
) -> u64 {
    // Canonical FNV-1a signature for transposition/hash use.
    // Free cells and tableau columns are normalized by sorting, so equivalent
    // states share the same signature.
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;

    let mut h: u64 = FNV_OFFSET;

    // Foundation already has fixed order (suit ordinal).
    for (idx, pile) in foundation.iter().enumerate() {
        h ^= (idx as u64).wrapping_add(0x100);
        h = h.wrapping_mul(FNV_PRIME);
        for card in pile {
            h ^= encode_card(*card);
            h = h.wrapping_mul(FNV_PRIME);
        }
    }

    // Canonical free cells: sorted to ignore equivalent permutations.
    let mut sorted_cells: Vec<u64> = free_cells.iter().map(|c| encode_opt_card(*c)).collect();
    sorted_cells.sort_unstable();
    h ^= 0xFD; // separator
    h = h.wrapping_mul(FNV_PRIME);
    for val in sorted_cells {
        h ^= val;
        h = h.wrapping_mul(FNV_PRIME);
    }

    // Canonical tableau: hash each column separately, then sort.
    let mut col_hashes: Vec<u64> = tableau
        .iter()
        .map(|col| {
            let mut col_h: u64 = FNV_OFFSET;
            for card in col {
                col_h ^= encode_card(*card);
                col_h = col_h.wrapping_mul(FNV_PRIME);
            }
            col_h
        })
        .collect();
    col_hashes.sort_unstable();

    h ^= 0xFE; // separator
    h = h.wrapping_mul(FNV_PRIME);
    for ch in col_hashes {
        h ^= ch;
        h = h.wrapping_mul(FNV_PRIME);
    }

    h
}

#[inline]
fn encode_card(card: Card) -> u64 {
    // Packs value and suit into a compact integer for hashing/signature.
    let suit_val: u64 = card.suit as u64;
    ((card.value as u64) << 4) | suit_val
}

#[inline]
fn encode_opt_card(card: Option<Card>) -> u64 {
    // Uses 0xFF as a sentinel to represent an empty slot.
    match card {
        Some(c) => encode_card(c),
        None => 0xFF,
    }
}
