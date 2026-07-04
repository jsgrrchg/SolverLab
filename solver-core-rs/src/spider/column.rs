use crate::common::card::Card;

/// A Spider Solitaire column: face-down cards and face-up cards.
#[derive(Debug, Clone)]
pub struct SpiderColumn {
    pub face_down: Vec<Card>,
    pub face_up: Vec<Card>,
    /// Cache: longest descending same-suit run from the top of `face_up`.
    pub longest_run: usize,
}

impl SpiderColumn {
    /// Creates a column and computes the descending same-suit run from the top.
    pub fn new(face_down: Vec<Card>, face_up: Vec<Card>) -> SpiderColumn {
        // Automatic flip: if `face_down` has cards but `face_up` is empty, flip the last card.
        let mut fd = face_down;
        let mut fu = face_up;
        if !fd.is_empty() && fu.is_empty() {
            let top = fd.pop().unwrap();
            fu.push(top);
        }
        let longest_run = compute_longest_same_suit_run(&fu);
        SpiderColumn {
            face_down: fd,
            face_up: fu,
            longest_run,
        }
    }

    /// Builds an empty column.
    pub fn empty() -> SpiderColumn {
        SpiderColumn {
            face_down: vec![],
            face_up: vec![],
            longest_run: 0,
        }
    }

    /// Indicates whether the column has no cards.
    pub fn is_empty(&self) -> bool {
        self.face_down.is_empty() && self.face_up.is_empty()
    }

    /// Indicates whether the column has face-down cards.
    pub fn has_face_down(&self) -> bool {
        !self.face_down.is_empty()
    }

    /// Indicates whether the column has face-up cards.
    pub fn has_face_up(&self) -> bool {
        !self.face_up.is_empty()
    }

    /// Number of face-down cards.
    pub fn num_face_down(&self) -> usize {
        self.face_down.len()
    }

    /// Number of face-up cards.
    pub fn num_face_up(&self) -> usize {
        self.face_up.len()
    }

    /// Total number of cards in the column.
    pub fn total_cards(&self) -> usize {
        self.face_down.len() + self.face_up.len()
    }

    /// Top `face_up` card, if any.
    pub fn top_card(&self) -> Option<Card> {
        self.face_up.last().copied()
    }

    /// Returns the top N cards (the last N cards in `face_up`).
    pub fn top_cards(&self, count: usize) -> Option<Vec<Card>> {
        if count < 1 || count > self.face_up.len() {
            return None;
        }
        let start = self.face_up.len() - count;
        Some(self.face_up[start..].to_vec())
    }

    /// Extracts a descending same-suit run from the top.
    pub fn extract_same_suit_run(&self, count: usize) -> Option<(Vec<Card>, SpiderColumn)> {
        let run = self.top_cards(count)?;
        if !is_same_suit_descending(&run) {
            return None;
        }
        let new_face_up = self.face_up[..self.face_up.len() - count].to_vec();
        Some((run, SpiderColumn::new(self.face_down.clone(), new_face_up)))
    }

    /// Extracts a single card from the top.
    pub fn extract_card(&self) -> Option<(Card, SpiderColumn)> {
        let card = *self.face_up.last()?;
        let new_face_up = self.face_up[..self.face_up.len() - 1].to_vec();
        Some((card, SpiderColumn::new(self.face_down.clone(), new_face_up)))
    }

    /// Adds several cards to the top.
    pub fn with_cards(&self, new_cards: &[Card]) -> Option<SpiderColumn> {
        if new_cards.is_empty() {
            return None;
        }
        let mut fu = self.face_up.clone();
        fu.extend_from_slice(new_cards);
        Some(SpiderColumn::new(self.face_down.clone(), fu))
    }

    /// Adds a single card to the top.
    pub fn with_card(&self, card: Card) -> SpiderColumn {
        let mut fu = self.face_up.clone();
        fu.push(card);
        SpiderColumn::new(self.face_down.clone(), fu)
    }

    /// Checks whether a card can be placed in this column.
    /// An empty column accepts any card; otherwise, the card must be exactly
    /// 1 value lower than the top card.
    #[inline]
    pub fn can_add_card(&self, card: Card) -> bool {
        if self.face_up.is_empty() && self.face_down.is_empty() {
            return true; // An empty column accepts any card.
        }
        match self.face_up.last() {
            Some(top) => top.value == card.value + 1,
            None => false,
        }
    }

    /// Checks whether a run can be placed in this column.
    pub fn can_add_run(&self, run: &[Card]) -> bool {
        match run.first() {
            Some(first) => self.can_add_card(*first),
            None => false,
        }
    }

    /// Counts suit transitions in face_up (adjacent cards with different suits).
    /// In 1-suit this always returns 0. Useful for fragmentation penalties.
    pub fn suit_transitions(&self) -> usize {
        self.face_up
            .windows(2)
            .filter(|w| w[0].suit != w[1].suit)
            .count()
    }

    /// Checks and removes complete same-suit K-to-A sequences from the top.
    /// Returns `(new_column, removed_sequence_count)`.
    pub fn check_and_remove_complete_sequences(&self) -> (SpiderColumn, usize) {
        let mut current_face_up = self.face_up.clone();
        let mut current_face_down = self.face_down.clone();
        let mut removed_sets = 0;

        while current_face_up.len() >= 13 {
            let start = current_face_up.len() - 13;
            if !is_complete_sequence_in_place(&current_face_up, start) {
                break;
            }
            current_face_up.truncate(start);
            removed_sets += 1;

            // If `face_up` becomes empty, flip one card from `face_down`.
            if current_face_up.is_empty() && !current_face_down.is_empty() {
                let top = current_face_down.pop().unwrap();
                current_face_up.push(top);
            }
        }

        let new_col = SpiderColumn::new(current_face_down, current_face_up);
        (new_col, removed_sets)
    }
}

// Custom `PartialEq` and `Hash` that exclude the cached `longest_run` field.
impl PartialEq for SpiderColumn {
    fn eq(&self, other: &Self) -> bool {
        self.face_down == other.face_down && self.face_up == other.face_up
    }
}

impl Eq for SpiderColumn {}

impl std::hash::Hash for SpiderColumn {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.face_down.hash(state);
        self.face_up.hash(state);
    }
}

/// Computes the longest descending same-suit run from the top (end) of `face_up`.
fn compute_longest_same_suit_run(face_up: &[Card]) -> usize {
    if face_up.is_empty() {
        return 0;
    }
    let mut length = 1;
    let len = face_up.len();
    for i in (0..len - 1).rev() {
        let current = &face_up[i];
        let below = &face_up[i + 1];
        if current.suit == below.suit && current.value == below.value + 1 {
            length += 1;
        } else {
            break;
        }
    }
    length
}

/// Checks whether the cards form a descending same-suit sequence.
fn is_same_suit_descending(cards: &[Card]) -> bool {
    if cards.is_empty() {
        return false;
    }
    if cards.len() == 1 {
        return true;
    }
    let suit = cards[0].suit;
    for i in 1..cards.len() {
        if cards[i].suit != suit || cards[i - 1].value != cards[i].value + 1 {
            return false;
        }
    }
    true
}

/// Checks in place whether 13 cards from `start` form a same-suit K-to-A sequence.
fn is_complete_sequence_in_place(cards: &[Card], start: usize) -> bool {
    if cards.len() - start < 13 {
        return false;
    }
    let suit = cards[start].suit;
    for i in 0..13 {
        let card = &cards[start + i];
        if card.suit != suit || card.value != (13 - i) as u8 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::card::Suit;

    fn card(suit: Suit, value: u8) -> Card {
        Card::new(suit, value)
    }

    #[test]
    fn test_empty_column() {
        let col = SpiderColumn::empty();
        assert!(col.is_empty());
        assert_eq!(col.longest_run, 0);
        assert!(col.can_add_card(card(Suit::Spade, 5)));
    }

    #[test]
    fn test_auto_flip() {
        let col = SpiderColumn::new(vec![card(Suit::Spade, 5), card(Suit::Heart, 3)], vec![]);
        assert_eq!(col.face_down.len(), 1);
        assert_eq!(col.face_up.len(), 1);
        assert_eq!(col.face_up[0], card(Suit::Heart, 3));
    }

    #[test]
    fn test_longest_run() {
        // K, Q, J of Spades = run of 3.
        let col = SpiderColumn::new(
            vec![],
            vec![
                card(Suit::Heart, 5), // Not part of the run.
                card(Suit::Spade, 13),
                card(Suit::Spade, 12),
                card(Suit::Spade, 11),
            ],
        );
        assert_eq!(col.longest_run, 3);
    }

    #[test]
    fn test_can_add_card() {
        let col = SpiderColumn::new(vec![], vec![card(Suit::Spade, 5)]);
        assert!(col.can_add_card(card(Suit::Heart, 4))); // Different suit allowed in Spider.
        assert!(col.can_add_card(card(Suit::Spade, 4))); // Same suit also allowed.
        assert!(!col.can_add_card(card(Suit::Spade, 3))); // Incorrect value.
        assert!(!col.can_add_card(card(Suit::Spade, 5))); // Same value.
    }

    #[test]
    fn test_complete_sequence() {
        // Build a Spades K-to-A sequence.
        let mut fu = vec![];
        for v in (1..=13).rev() {
            fu.push(card(Suit::Spade, v));
        }
        let col = SpiderColumn::new(vec![], fu);
        let (new_col, removed) = col.check_and_remove_complete_sequences();
        assert_eq!(removed, 1);
        assert!(new_col.is_empty());
    }
}
