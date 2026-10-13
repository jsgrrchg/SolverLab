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
    /// Zobrist hash of the complete state. Used as a transposition table key.
    /// Maintained incrementally by `KlondikeMove::apply`; recompute with
    /// `compute_signature` after editing the board by hand.
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
        let mut board = Self {
            columns: [FastColumn::empty(); 7],
            foundation: [0; 4],
            stock: [Card::new(Suit::Club, 0); 24],
            stock_len: 0,
            stock_index: 0,
            stock_recycles: 0,
            signature: 0,
        };
        board.compute_signature();
        board
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

    /// Recomputes the board Zobrist signature from scratch and stores it in
    /// `self.signature`. Must be called after any board state modification
    /// not produced by `KlondikeMove::apply`, which maintains the signature
    /// incrementally.
    pub fn compute_signature(&mut self) {
        self.signature = klondike_signature(self);
    }

    // ── Incremental state mutation (keeps `signature` in sync) ──────
    //
    // `KlondikeMove::apply` goes through these helpers so that each move
    // touches only the Zobrist keys of the cards it moves, instead of
    // rehashing the whole board.

    /// Appends a face-up card to a column.
    #[inline]
    pub(crate) fn push_face_up(&mut self, column: usize, card: Card) {
        let col = &mut self.columns[column];
        let position = col.len as usize;
        col.cards[position] = card;
        col.len += 1;
        self.signature ^= column_key(column, position, true, card);
    }

    /// Removes the top face-up card of a column. Returns None if the column
    /// has no face-up card.
    #[inline]
    pub(crate) fn pop_face_up(&mut self, column: usize) -> Option<Card> {
        let col = &mut self.columns[column];
        if col.len == 0 || col.len == col.face_down_len {
            return None;
        }
        col.len -= 1;
        let position = col.len as usize;
        let card = col.cards[position];
        self.signature ^= column_key(column, position, true, card);
        Some(card)
    }

    /// Turns the top card face-up when every remaining card of the column is
    /// face-down (the rule after moving a run away).
    #[inline]
    pub(crate) fn reveal_top_if_hidden(&mut self, column: usize) {
        let col = &mut self.columns[column];
        if col.len > 0 && col.face_down_len == col.len {
            col.face_down_len -= 1;
            let position = col.face_down_len as usize;
            let card = col.cards[position];
            self.signature ^= column_key(column, position, false, card)
                ^ column_key(column, position, true, card);
        }
    }

    /// Sets the highest rank of a foundation.
    #[inline]
    pub(crate) fn set_foundation_rank(&mut self, suit: Suit, rank: u8) {
        let index = suit as usize;
        self.signature ^=
            foundation_key(index, self.foundation[index]) ^ foundation_key(index, rank);
        self.foundation[index] = rank;
    }

    /// Moves the waste pointer.
    #[inline]
    pub(crate) fn set_stock_index(&mut self, stock_index: u8) {
        self.signature ^= stock_index_key(self.stock_index) ^ stock_index_key(stock_index);
        self.stock_index = stock_index;
    }

    /// Removes the current waste card, compacting the stock so the card below
    /// it becomes the new waste card. Resets the recycle counter.
    pub(crate) fn take_waste_card(&mut self) -> Option<Card> {
        if self.stock_index == 0 || self.stock_index > self.stock_len {
            return None;
        }
        let real_idx = (self.stock_index - 1) as usize;
        let card = self.stock[real_idx];
        for i in real_idx..self.stock_len as usize - 1 {
            self.stock[i] = self.stock[i + 1];
        }
        self.stock_len -= 1;
        // Stock cards hash by identity: their relative order never changes.
        self.signature ^= stock_key(card);
        self.set_stock_index(self.stock_index - 1);
        self.stock_recycles = 0;
        Some(card)
    }
}

// ═══════════════════════════════════════════
// Board signature (Zobrist hashing)
// ═══════════════════════════════════════════
//
// The signature is the XOR of one pseudo-random key per (card, location):
//
// 1. Column cards: key per (column, position, face-up?, card). Position and
//    facing are part of the key, so the face-down/face-up boundary of each
//    column is captured exactly as the previous FNV-1a layout did.
// 2. Stock cards: key per card only. Cards never return to the stock and
//    the compaction in `take_waste_card` preserves their relative order, so
//    the set of cards still in the stock determines the whole array.
// 3. The waste pointer: key per `stock_index` value.
// 4. Foundations: key per (suit, highest rank), including rank 0.
//
// `stock_recycles` is not part of the signature, as before. Keys come from a
// fixed-seed SplitMix64 sequence evaluated at compile time, so the hash is
// identical on every platform and build.

const COLUMN_SLOTS: usize = KlondikeBoard::NUM_COLUMNS * 21 * 2;
const CARD_SLOTS: usize = 52;

const fn splitmix64(state: u64) -> u64 {
    let mut z = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

const fn zobrist_table<const N: usize>(seed: u64) -> [u64; N] {
    let mut table = [0u64; N];
    let mut state = seed;
    let mut i = 0;
    while i < N {
        state = splitmix64(state);
        table[i] = state;
        i += 1;
    }
    table
}

static COLUMN_KEYS: [u64; COLUMN_SLOTS * CARD_SLOTS] = zobrist_table(0x4B4C_4F4E_4449_4B45); // "KLONDIKE"
static STOCK_KEYS: [u64; CARD_SLOTS] = zobrist_table(0x5354_4F43_4B00_0001);
static STOCK_INDEX_KEYS: [u64; 25] = zobrist_table(0x5354_4F43_4B00_0002);
static FOUNDATION_KEYS: [u64; 4 * 14] = zobrist_table(0x464F_554E_4441_5449);

/// Dense card index in 0..52. Illegal ranks (the null card) share slot 0 of
/// their suit; they never appear in a dealt board.
#[inline(always)]
fn card_slot(card: Card) -> usize {
    (card.suit as usize) * 13 + (card.value.clamp(1, 13) as usize - 1)
}

#[inline(always)]
fn column_key(column: usize, position: usize, face_up: bool, card: Card) -> u64 {
    let slot = ((column * 21 + position) * 2 + face_up as usize) * CARD_SLOTS + card_slot(card);
    COLUMN_KEYS[slot]
}

#[inline(always)]
fn stock_key(card: Card) -> u64 {
    STOCK_KEYS[card_slot(card)]
}

#[inline(always)]
fn stock_index_key(stock_index: u8) -> u64 {
    STOCK_INDEX_KEYS[(stock_index as usize).min(24)]
}

#[inline(always)]
fn foundation_key(suit: usize, rank: u8) -> u64 {
    FOUNDATION_KEYS[suit * 14 + (rank as usize).min(13)]
}

/// Computes the Zobrist signature of the complete board state from scratch.
pub(crate) fn klondike_signature(board: &KlondikeBoard) -> u64 {
    let mut h = 0u64;
    for (column, col) in board.columns.iter().enumerate() {
        for position in 0..col.len as usize {
            let face_up = position >= col.face_down_len as usize;
            h ^= column_key(column, position, face_up, col.cards[position]);
        }
    }
    for i in 0..board.stock_len as usize {
        h ^= stock_key(board.stock[i]);
    }
    h ^= stock_index_key(board.stock_index);
    for (suit, &rank) in board.foundation.iter().enumerate() {
        h ^= foundation_key(suit, rank);
    }
    h
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
    fn incremental_mutations_keep_the_signature_in_sync() {
        let mut board = KlondikeBoard::new();
        board.columns[0].push(card(Suit::Club, 9), true);
        board.columns[0].push(card(Suit::Heart, 8), false);
        board.columns[1].push(card(Suit::Spade, 7), false);
        board.stock[0] = card(Suit::Diamond, 2);
        board.stock[1] = card(Suit::Club, 5);
        board.stock[2] = card(Suit::Heart, 6);
        board.stock_len = 3;
        board.stock_index = 2;
        board.foundation = [1, 0, 0, 0];
        board.compute_signature();
        let check = |board: &KlondikeBoard| assert_eq!(board.signature, klondike_signature(board));

        // Column push / pop / reveal.
        board.push_face_up(1, card(Suit::Heart, 6));
        check(&board);
        assert_eq!(board.pop_face_up(1), Some(card(Suit::Heart, 6)));
        check(&board);
        assert_eq!(board.pop_face_up(0), Some(card(Suit::Heart, 8)));
        check(&board);
        assert_eq!(board.columns[0].face_down_len, 1);
        board.reveal_top_if_hidden(0);
        check(&board);
        assert_eq!(board.columns[0].face_down_len, 0);
        assert_eq!(board.pop_face_up(0), Some(card(Suit::Club, 9)));
        assert_eq!(board.pop_face_up(0), None);
        check(&board);

        // Foundation and stock pointer.
        board.set_foundation_rank(Suit::Club, 2);
        check(&board);
        board.set_stock_index(3);
        check(&board);
        board.set_stock_index(0);
        check(&board);

        // Waste removal compacts and re-hashes only the removed card.
        board.set_stock_index(2);
        assert_eq!(board.take_waste_card(), Some(card(Suit::Club, 5)));
        check(&board);
        assert_eq!(board.stock_len, 2);
        assert_eq!(board.stock_index, 1);
        assert_eq!(board.stock[1], card(Suit::Heart, 6));
        assert_eq!(board.take_waste_card(), Some(card(Suit::Diamond, 2)));
        check(&board);
        assert_eq!(board.take_waste_card(), None);
    }

    #[test]
    fn signature_distinguishes_facing_position_column_and_stock_pointer() {
        let base = KlondikeBoard::new();
        assert_eq!(base.signature, klondike_signature(&base));

        let mut face_down = base;
        face_down.columns[0].push(card(Suit::Club, 9), true);
        face_down.compute_signature();
        let mut face_up = base;
        face_up.columns[0].push(card(Suit::Club, 9), false);
        face_up.compute_signature();
        assert_ne!(face_down.signature, face_up.signature);

        let mut other_column = base;
        other_column.columns[1].push(card(Suit::Club, 9), false);
        other_column.compute_signature();
        assert_ne!(face_up.signature, other_column.signature);

        let mut pointer = base;
        pointer.stock[0] = card(Suit::Club, 9);
        pointer.stock_len = 1;
        pointer.compute_signature();
        let with_pointer = {
            let mut b = pointer;
            b.stock_index = 1;
            b.compute_signature();
            b
        };
        assert_ne!(pointer.signature, with_pointer.signature);
        assert_ne!(pointer.signature, face_up.signature);
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
