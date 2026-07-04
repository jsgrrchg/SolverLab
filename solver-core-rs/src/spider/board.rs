use super::column::SpiderColumn;
use crate::common::card::{Card, Suit};

/// Spider Solitaire board: 10 columns, stock, and completed-sequence counter.
#[derive(Debug, Clone)]
pub struct SpiderBoard {
    pub columns: Vec<SpiderColumn>,
    pub stock: Vec<Card>,
    /// Number of completed K-to-A sequences removed (0-8).
    pub completed_sets: usize,
    /// FNV-1a signature for fast hashing.
    pub signature: u64,
    /// Cache: total face-down cards (0-104).
    cached_face_down: u8,
    /// Cache: targets by suit (precomputed in `new()`).
    cached_suit_targets: Vec<Card>,
}

impl SpiderBoard {
    pub const NUM_COLUMNS: usize = 10;

    /// Creates a board and computes its incremental signature.
    pub fn new(columns: Vec<SpiderColumn>, stock: Vec<Card>, completed_sets: usize) -> SpiderBoard {
        let signature = compute_signature(&columns, &stock, completed_sets);
        let cached_face_down = columns.iter().map(|c| c.num_face_down()).sum::<usize>() as u8;
        let cached_suit_targets = compute_suit_targets(&columns, &stock);
        SpiderBoard {
            columns,
            stock,
            completed_sets,
            signature,
            cached_face_down,
            cached_suit_targets,
        }
    }

    /// Returns an immutable reference to the requested column.
    pub fn column(&self, index: usize) -> &SpiderColumn {
        &self.columns[index]
    }

    /// Stock can be dealt only if there are cards and no column is empty.
    pub fn can_deal_from_stock(&self) -> bool {
        !self.stock.is_empty() && self.columns.iter().all(|c| !c.is_empty())
    }

    /// Total face-down cards across all columns (cached in `new()`).
    pub fn total_face_down(&self) -> usize {
        self.cached_face_down as usize
    }

    /// Total face-up cards across all columns.
    pub fn total_face_up(&self) -> usize {
        self.columns.iter().map(|c| c.num_face_up()).sum()
    }

    /// Number of empty columns.
    pub fn empty_column_count(&self) -> usize {
        self.columns.iter().filter(|c| c.is_empty()).count()
    }

    /// Indicates whether at least one empty column exists.
    pub fn has_empty_column(&self) -> bool {
        self.columns.iter().any(|c| c.is_empty())
    }

    /// Returns the "targets" by suit (precomputed in `new()`).
    pub fn suit_targets(&self) -> &[Card] {
        &self.cached_suit_targets
    }
}

// Custom `PartialEq` that ignores `signature` (it is derived).
impl PartialEq for SpiderBoard {
    fn eq(&self, other: &Self) -> bool {
        self.completed_sets == other.completed_sets
            && self.stock == other.stock
            && self.columns == other.columns
    }
}

impl Eq for SpiderBoard {}

// `Hash` uses `signature` for O(1) hashing.
impl std::hash::Hash for SpiderBoard {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.signature);
    }
}

/// Computes "targets" by suit: for each suit present in the game, identifies
/// the lowest card that is not yet in the longest K-to-A run.
/// If no run headed by a King exists for a suit, the target is King.
fn compute_suit_targets(columns: &[SpiderColumn], stock: &[Card]) -> Vec<Card> {
    let mut best_run: [usize; 4] = [0; 4];
    let mut suit_seen = [false; 4];

    for col in columns {
        let fu = &col.face_up;
        for c in fu.iter().chain(col.face_down.iter()) {
            suit_seen[c.suit as usize] = true;
        }
        for start in 0..fu.len() {
            if fu[start].value != 13 {
                continue;
            }
            let suit_idx = fu[start].suit as usize;
            let mut run_len: usize = 1;
            let mut pos = start + 1;
            while pos < fu.len()
                && fu[pos].suit == fu[start].suit
                && fu[pos].value as usize == 13 - run_len
            {
                run_len += 1;
                pos += 1;
            }
            if run_len > best_run[suit_idx] {
                best_run[suit_idx] = run_len;
            }
        }
    }
    for c in stock {
        suit_seen[c.suit as usize] = true;
    }

    let mut targets = Vec::with_capacity(4);
    for &suit in &Suit::ALL {
        let idx = suit as usize;
        if !suit_seen[idx] {
            continue;
        }
        let run_len = best_run[idx];
        if run_len >= 13 {
            continue;
        }
        let target_value = (13 - run_len) as u8;
        targets.push(Card::new(suit, target_value));
    }
    targets
}

/// FNV-1a signature calculation that exactly matches the Swift implementation.
fn compute_signature(columns: &[SpiderColumn], stock: &[Card], completed_sets: usize) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;

    let mut h: u64 = FNV_OFFSET;

    for (col_idx, col) in columns.iter().enumerate() {
        // Column marker.
        h ^= (col_idx as u64).wrapping_add(0x100);
        h = h.wrapping_mul(FNV_PRIME);

        for card in &col.face_down {
            h ^= encode_card(*card);
            h = h.wrapping_mul(FNV_PRIME);
        }

        h ^= 0xFE; // Separator between face_down and face_up.
        h = h.wrapping_mul(FNV_PRIME);

        for card in &col.face_up {
            h ^= encode_card(*card);
            h = h.wrapping_mul(FNV_PRIME);
        }
    }

    h ^= 0xFD; // Separator between columns and stock.
    h = h.wrapping_mul(FNV_PRIME);

    for card in stock {
        h ^= encode_card(*card);
        h = h.wrapping_mul(FNV_PRIME);
    }

    h ^= completed_sets as u64;
    h = h.wrapping_mul(FNV_PRIME);

    h
}

#[inline]
/// Encodes a card into a compact `u64` (value in high bits + suit in low bits).
fn encode_card(card: Card) -> u64 {
    let suit_val: u64 = card.suit as u64;
    ((card.value as u64) << 4) | suit_val
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::card::Suit;

    #[test]
    fn test_empty_board() {
        let cols = (0..10).map(|_| SpiderColumn::empty()).collect();
        let board = SpiderBoard::new(cols, vec![], 0);
        assert_eq!(board.total_face_down(), 0);
        assert_eq!(board.total_face_up(), 0);
        assert_eq!(board.empty_column_count(), 10);
        assert!(!board.can_deal_from_stock());
    }

    #[test]
    fn test_signature_consistency() {
        let cols: Vec<SpiderColumn> = (0..10)
            .map(|i| {
                SpiderColumn::new(
                    vec![Card::new(Suit::Spade, (i % 13 + 1) as u8)],
                    vec![Card::new(Suit::Heart, (i % 13 + 1) as u8)],
                )
            })
            .collect();
        let board1 = SpiderBoard::new(cols.clone(), vec![], 0);
        let board2 = SpiderBoard::new(cols, vec![], 0);
        assert_eq!(board1.signature, board2.signature);
        assert_eq!(board1, board2);
    }

    #[test]
    fn test_win_condition() {
        let cols = (0..10).map(|_| SpiderColumn::empty()).collect();
        let board = SpiderBoard::new(cols, vec![], 8);
        assert_eq!(board.completed_sets, 8);
    }
}
