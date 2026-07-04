use crate::common::card::{Card, Suit};

// ═══════════════════════════════════════════
// FastColumn
// ═══════════════════════════════════════════

/// Klondike tableau column backed by a fixed-size array to avoid heap
/// allocations. Cards are stored from base to top: `cards[0]` is the
/// deepest card in the column.
///
/// Memory layout:
/// ```text
/// cards[0 .. face_down_len]   -> face-down cards (hidden)
/// cards[face_down_len .. len] -> face-up cards (visible)
/// ```
/// Maximum capacity is 21 cards (7 initial cards + up to 14 moved on top).
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct FastColumn {
    pub cards: [Card; 21],
    /// Total number of cards in the column (face-down + face-up).
    pub len: u8,
    /// Number of face-down cards. Always <= len.
    pub face_down_len: u8,
}

impl FastColumn {
    /// Creates an empty column. Cards are initialized with a null value (Club, 0).
    pub fn empty() -> Self {
        Self {
            cards: [Card::new(Suit::Club, 0); 21],
            len: 0,
            face_down_len: 0,
        }
    }

    /// Adds a card to the top of the column.
    /// If `face_down` is true, also increments `face_down_len`.
    pub fn push(&mut self, card: Card, face_down: bool) {
        self.cards[self.len as usize] = card;
        self.len += 1;
        if face_down {
            self.face_down_len += 1;
        }
    }

    /// Pops and returns the top card. Returns None if the column is empty.
    /// If removing the card leaves the top in the face-down zone, adjusts
    /// `face_down_len` (the case where the column has more face-down cards
    /// than total cards).
    pub fn pop(&mut self) -> Option<Card> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        let c = self.cards[self.len as usize];
        if self.face_down_len > self.len {
            self.face_down_len = self.len;
        }
        Some(c)
    }

    /// Removes `count` cards from the top without returning them.
    /// Used to move complete stacks between columns.
    pub fn pop_count(&mut self, count: u8) {
        if count <= self.len {
            self.len -= count;
            if self.face_down_len > self.len {
                self.face_down_len = self.len;
            }
        }
    }

    /// Returns the top card whether it is face-up or face-down.
    /// Useful for generic inspection operations.
    pub fn top(&self) -> Option<Card> {
        if self.len > 0 {
            Some(self.cards[(self.len - 1) as usize])
        } else {
            None
        }
    }

    /// Returns the top card only if it is face-up.
    /// Returns None if the column is empty or if the top card is face-down
    /// (a situation that should not occur in a valid board, but is checked for safety).
    pub fn top_face_up(&self) -> Option<Card> {
        if self.len > self.face_down_len {
            Some(self.cards[(self.len - 1) as usize])
        } else {
            None
        }
    }

    /// Checks whether `run_base` can be placed on top of this column under
    /// Klondike rules: alternating color and descending value.
    /// If the column is empty (or entirely face-down), only a King (value 13) is allowed.
    pub fn can_add_run(&self, run_base: Card) -> bool {
        if self.len == self.face_down_len {
            // Column with no face-up cards: only accepts a King.
            return run_base.value == 13;
        }
        let top = self.cards[(self.len - 1) as usize];
        top.value == run_base.value + 1 && top.color() != run_base.color()
    }

    // ── State queries ─────────────────

    pub fn has_face_down(&self) -> bool {
        self.face_down_len > 0
    }
    pub fn num_face_down(&self) -> usize {
        self.face_down_len as usize
    }
    pub fn has_face_up(&self) -> bool {
        self.len > self.face_down_len
    }
    pub fn num_face_up(&self) -> usize {
        (self.len - self.face_down_len) as usize
    }
    /// Returns a slice containing only face-up cards (base-to-top order).
    pub fn face_up_cards(&self) -> &[Card] {
        &self.cards[self.face_down_len as usize..self.len as usize]
    }
}

// ═══════════════════════════════════════════
// KlondikeBoard
// ═══════════════════════════════════════════

/// Complete Klondike board state. Designed to be `Copy` and fit on the stack,
/// allowing IDA* to clone states without allocations.
///
/// # Stock model
/// Stock cards are stored in `stock[0..stock_len]`.
/// `stock_index` is a 1-based index pointing to the current waste pile position
/// (the accessible card is `stock[stock_index - 1]`).
///
/// - `stock_index == 0`: no accessible card (initial or post-recycle state).
/// - Advancing the stock increments `stock_index` by `draw_advance` (1 or 3).
/// - Recycling resets `stock_index` to 0.
/// - When the pile card is played, it is removed from the array and the rest is compacted.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct KlondikeBoard {
    /// The 7 tableau columns.
    pub columns: [FastColumn; 7],
    /// Highest rank placed in each foundation, indexed by Suit as usize.
    /// Value 0 = empty foundation; value 13 = complete foundation.
    pub foundation: [u8; 4],
    /// Compact array of remaining stock cards (including waste).
    pub stock: [Card; 24],
    /// Number of cards currently in the stock array.
    pub stock_len: u8,
    /// 1-based position of the top of the waste pile. The playable card is stock[stock_index-1].
    pub stock_index: u8,
    /// Number of times the stock has been recycled. Used by optional rules.
    pub stock_recycles: u8,
    /// FNV-1a hash of the complete state. Used as a transposition table key.
    pub signature: u64,
}

// Board hashing delegates directly to the precomputed signature.
impl std::hash::Hash for KlondikeBoard {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.signature);
    }
}

impl KlondikeBoard {
    pub const NUM_COLUMNS: usize = 7;

    /// Creates an empty board with no cards. Used as the starting point for `deal`.
    pub fn new() -> Self {
        Self {
            columns: [FastColumn::empty(); 7],
            foundation: [0; 4],
            stock: [Card::new(Suit::Club, 0); 24],
            stock_len: 0,
            stock_index: 0,
            stock_recycles: 0,
            signature: 0,
        }
    }

    // ── Foundation queries ─────────────

    /// Returns the highest card in the foundation for `suit`, or None if it is empty.
    pub fn top_of_foundation(&self, suit: Suit) -> Option<Card> {
        let val = self.foundation[suit as usize];
        if val == 0 {
            None
        } else {
            Some(Card::new(suit, val))
        }
    }

    /// Number of cards in the foundation for `suit` (0-13).
    pub fn foundation_count(&self, suit: Suit) -> usize {
        self.foundation[suit as usize] as usize
    }

    /// Total number of cards across all foundations (0-52). Value 52 = victory.
    pub fn total_foundation_count(&self) -> usize {
        self.foundation.iter().map(|&x| x as usize).sum()
    }

    /// True if `card` can be placed in its foundation (it is exactly the next value).
    pub fn can_add_to_foundation(&self, card: Card) -> bool {
        self.foundation[card.suit as usize] + 1 == card.value
    }

    // ── Stock operations ────────────────

    /// Extracts the top waste pile card (stock[stock_index-1]) and compacts the array.
    /// Returns the extracted card together with the new board state.
    /// Returns None if there is no accessible pile card.
    pub fn extract_stock_pile_card(&self) -> Option<(Card, KlondikeBoard)> {
        if self.stock_index == 0 || self.stock_index > self.stock_len {
            return None;
        }
        let real_idx = (self.stock_index - 1) as usize;
        let card = self.stock[real_idx];

        let mut n = *self;
        // Compact by shifting the remaining cards one position back.
        for i in real_idx..n.stock_len as usize - 1 {
            n.stock[i] = n.stock[i + 1];
        }
        n.stock_len -= 1;
        n.stock_index -= 1;
        Some((card, n))
    }

    /// True if the stock has unrevealed cards (can advance).
    pub fn can_advance_stock(&self) -> bool {
        self.stock_index < self.stock_len
    }

    /// True if the stock has been fully advanced and can be recycled.
    pub fn can_recycle_stock(&self) -> bool {
        self.stock_len > 0 && self.stock_index >= self.stock_len
    }

    /// Returns the currently accessible waste pile card without modifying the state.
    pub fn stock_pile_card(&self) -> Option<Card> {
        if self.stock_index > 0 && self.stock_index <= self.stock_len {
            Some(self.stock[(self.stock_index - 1) as usize])
        } else {
            None
        }
    }

    // ── Signature / Hash ────────────────────────

    /// Recomputes the board FNV-1a signature and stores it in `self.signature`.
    /// Must be called after any board state modification not produced by
    /// `KlondikeMove::apply` (which does this automatically).
    pub fn compute_signature(&mut self) {
        self.signature = klondike_signature(self);
    }
}

// ═══════════════════════════════════════════
// Board signature (FNV-1a)
// ═══════════════════════════════════════════

/// Computes an FNV-1a (Fowler-Noll-Vo) hash of the complete board state.
///
/// The hash includes:
/// 1. Each column: face-down cards first (in order), then face-up cards.
///    A separator (0xFE) distinguishes the boundary between both zones.
/// 2. The complete stock (cards in order) plus the current position (stock_index).
/// 3. The highest value in each foundation by suit.
///
/// Distinct prefixes are used per section (0x100 for columns, 0x200 for stock,
/// 0x300 for foundations) to avoid collisions between states with the same bytes
/// in different contexts.
fn klondike_signature(board: &KlondikeBoard) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;
    let mut h = FNV_OFFSET;

    // Tableau columns.
    for (col_idx, col) in board.columns.iter().enumerate() {
        h ^= (col_idx as u64).wrapping_add(0x100);
        h = h.wrapping_mul(FNV_PRIME);
        // Face-down cards.
        for i in 0..col.face_down_len {
            h ^= encode_card(col.cards[i as usize]);
            h = h.wrapping_mul(FNV_PRIME);
        }
        // Face-down / face-up zone separator.
        h ^= 0xFE;
        h = h.wrapping_mul(FNV_PRIME);
        // Face-up cards.
        for i in col.face_down_len..col.len {
            h ^= encode_card(col.cards[i as usize]);
            h = h.wrapping_mul(FNV_PRIME);
        }
    }

    // Separator between columns and stock.
    h ^= 0xFD;
    h = h.wrapping_mul(FNV_PRIME);

    // Stock (cards in order + current position).
    for i in 0..board.stock_len {
        h ^= encode_card(board.stock[i as usize]);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h ^= (board.stock_index as u64).wrapping_add(0x200);
    h = h.wrapping_mul(FNV_PRIME);

    // Foundations
    for (suit_idx, &val) in board.foundation.iter().enumerate() {
        h ^= (suit_idx as u64).wrapping_add(0x300);
        h = h.wrapping_mul(FNV_PRIME);
        if val > 0 {
            h ^= val as u64;
            h = h.wrapping_mul(FNV_PRIME);
        }
    }
    h
}

/// Encodes a card in 8 bits: [value (4 bits) | suit (4 bits)].
/// Inline version to maximize performance in the signature loop.
#[inline(always)]
fn encode_card(card: Card) -> u64 {
    let suit_val: u64 = card.suit as u64;
    ((card.value as u64) << 4) | suit_val
}

// ═══════════════════════════════════════════
// Deal
// ═══════════════════════════════════════════

/// Builds the initial board from a 52-card deck.
///
/// Standard Klondike layout:
/// - Column 0: 1 face-up card
/// - Column 1: 1 face-down + 1 face-up
/// - Column k: k face-down + 1 face-up (on top)
/// - The remaining 24 cards go to the stock (face-down, ready to advance).
///
/// Returns None if the deck does not have exactly 52 cards.
pub fn deal(deck: &[Card]) -> Option<KlondikeBoard> {
    if deck.len() != 52 {
        return None;
    }
    let mut board = KlondikeBoard::new();
    let mut d_idx = 0;

    // Deal 28 cards to the tableau (1+2+3+4+5+6+7).
    for col in 0..7 {
        for row in 0..=col {
            let is_down = row < col; // Only the last card in each column is face-up.
            board.columns[col].push(deck[d_idx], is_down);
            d_idx += 1;
        }
    }

    // The remaining 24 cards form the initial stock.
    while d_idx < 52 {
        board.stock[board.stock_len as usize] = deck[d_idx];
        board.stock_len += 1;
        d_idx += 1;
    }

    board.compute_signature();
    Some(board)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(suit: Suit, value: u8) -> Card {
        Card::new(suit, value)
    }

    #[test]
    fn fast_column_pop_count_adjusts_face_down_len() {
        let mut col = FastColumn::empty();
        col.push(card(Suit::Club, 9), true);
        col.push(card(Suit::Diamond, 8), true);
        col.push(card(Suit::Heart, 7), false);

        col.pop_count(2);

        assert_eq!(col.len, 1);
        assert_eq!(col.face_down_len, 1);
        assert_eq!(col.top(), Some(card(Suit::Club, 9)));
        assert_eq!(col.top_face_up(), None);
    }

    #[test]
    fn fast_column_top_distinguishes_face_down_and_face_up() {
        let mut col = FastColumn::empty();
        col.push(card(Suit::Club, 9), true);

        assert_eq!(col.top(), Some(card(Suit::Club, 9)));
        assert_eq!(col.top_face_up(), None);

        col.push(card(Suit::Heart, 8), false);

        assert_eq!(col.top(), Some(card(Suit::Heart, 8)));
        assert_eq!(col.top_face_up(), Some(card(Suit::Heart, 8)));
    }

    #[test]
    fn fast_column_can_add_run_handles_empty_alternating_and_value() {
        let empty = FastColumn::empty();
        assert!(empty.can_add_run(card(Suit::Club, 13)));
        assert!(!empty.can_add_run(card(Suit::Club, 12)));

        let mut col = FastColumn::empty();
        col.push(card(Suit::Club, 9), false);

        assert!(col.can_add_run(card(Suit::Heart, 8)));
        assert!(!col.can_add_run(card(Suit::Spade, 8)));
        assert!(!col.can_add_run(card(Suit::Heart, 7)));
    }

    #[test]
    fn extract_stock_pile_card_compacts_stock_and_decrements_index() {
        let mut board = KlondikeBoard::new();
        board.stock[0] = card(Suit::Club, 1);
        board.stock[1] = card(Suit::Diamond, 2);
        board.stock[2] = card(Suit::Heart, 3);
        board.stock_len = 3;
        board.stock_index = 2;

        let (removed, next) = board
            .extract_stock_pile_card()
            .expect("waste card should be extracted");

        assert_eq!(removed, card(Suit::Diamond, 2));
        assert_eq!(next.stock_len, 2);
        assert_eq!(next.stock_index, 1);
        assert_eq!(next.stock[0], card(Suit::Club, 1));
        assert_eq!(next.stock[1], card(Suit::Heart, 3));
    }

    #[test]
    fn stock_advance_and_recycle_reflect_stock_position() {
        let mut board = KlondikeBoard::new();
        board.stock_len = 3;

        board.stock_index = 0;
        assert!(board.can_advance_stock());
        assert!(!board.can_recycle_stock());

        board.stock_index = 2;
        assert!(board.can_advance_stock());
        assert!(!board.can_recycle_stock());

        board.stock_index = 3;
        assert!(!board.can_advance_stock());
        assert!(board.can_recycle_stock());
    }

    #[test]
    fn stock_pile_card_handles_zero_valid_and_out_of_range_indices() {
        let mut board = KlondikeBoard::new();
        board.stock[0] = card(Suit::Club, 1);
        board.stock[1] = card(Suit::Diamond, 2);
        board.stock_len = 2;

        board.stock_index = 0;
        assert_eq!(board.stock_pile_card(), None);

        board.stock_index = 2;
        assert_eq!(board.stock_pile_card(), Some(card(Suit::Diamond, 2)));

        board.stock_index = 3;
        assert_eq!(board.stock_pile_card(), None);
    }

    #[test]
    fn compute_signature_changes_when_state_changes() {
        let mut board = KlondikeBoard::new();
        board.columns[0].push(card(Suit::Club, 1), false);
        board.stock[0] = card(Suit::Heart, 5);
        board.stock_len = 1;
        board.stock_index = 1;
        board.compute_signature();
        let original = board.signature;

        board.foundation[Suit::Club as usize] = 1;
        board.compute_signature();
        assert_ne!(board.signature, original);

        let foundation_signature = board.signature;
        board.stock_index = 0;
        board.compute_signature();
        assert_ne!(board.signature, foundation_signature);
    }
}
