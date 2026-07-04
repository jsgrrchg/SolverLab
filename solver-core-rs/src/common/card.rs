/// Shared card types used across all solitaire games.

/// The four standard suits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Suit {
    Club = 0,
    Diamond = 1,
    Heart = 2,
    Spade = 3,
}

impl Suit {
    pub const ALL: [Suit; 4] = [Suit::Club, Suit::Diamond, Suit::Heart, Suit::Spade];

    /// Red = Diamond, Heart; Black = Club, Spade.
    pub fn color(self) -> Color {
        match self {
            Suit::Club | Suit::Spade => Color::Black,
            Suit::Diamond | Suit::Heart => Color::Red,
        }
    }

    pub fn from_u8(v: u8) -> Option<Suit> {
        match v {
            0 => Some(Suit::Club),
            1 => Some(Suit::Diamond),
            2 => Some(Suit::Heart),
            3 => Some(Suit::Spade),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    Black,
    Red,
}

/// A playing card with a suit and value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Card {
    pub suit: Suit,
    pub value: u8,
}

impl Card {
    pub fn new(suit: Suit, value: u8) -> Card {
        Card { suit, value }
    }

    pub fn color(self) -> Color {
        self.suit.color()
    }

    /// Encode as a single byte: suit * 16 + value. Used for FFI.
    pub fn encode(self) -> u8 {
        (self.suit as u8) * 16 + self.value
    }

    /// Decode from the single-byte encoding.
    pub fn decode(byte: u8) -> Option<Card> {
        let suit_idx = byte / 16;
        let value = byte % 16;
        let suit = Suit::from_u8(suit_idx)?;
        let v = value;
        if v >= 1 && v <= 13 {
            Some(Card { suit, value: v })
        } else {
            None
        }
    }

    /// Build a standard 52-card deck (unshuffled).
    pub fn standard_deck() -> Vec<Card> {
        let mut deck = Vec::with_capacity(52);
        for &suit in &Suit::ALL {
            for v in 1..=13u8 {
                deck.push(Card::new(suit, v));
            }
        }
        deck
    }

    /// Build a Spider deck: `suit_count` suits repeated to fill 104 cards.
    /// suit_count=1 → all spades, suit_count=2 → spades+hearts, suit_count=4 → all suits.
    pub fn spider_deck(suit_count: u32) -> Vec<Card> {
        let suits: Vec<Suit> = match suit_count {
            1 => vec![Suit::Spade; 4],
            2 => vec![Suit::Spade, Suit::Spade, Suit::Heart, Suit::Heart],
            _ => vec![Suit::Club, Suit::Diamond, Suit::Heart, Suit::Spade],
        };
        let mut deck = Vec::with_capacity(104);
        for &suit in &suits {
            for v in 1..=13u8 {
                deck.push(Card::new(suit, v));
            }
        }
        // 4 suits × 13 values = 52, need 104, so duplicate
        let first_half = deck.clone();
        deck.extend(first_half);
        // Actually spider uses 2 full decks of the chosen suits
        // For suit_count=1: 8 copies of 13 cards = 104
        // For suit_count=4: 2 copies of 52 cards = 104
        // Let me redo this properly:
        deck.clear();
        let repeats = match suit_count {
            1 => 8u32,
            2 => 4,
            _ => 2,
        };
        let active_suits: Vec<Suit> = match suit_count {
            1 => vec![Suit::Spade],
            2 => vec![Suit::Spade, Suit::Heart],
            _ => Suit::ALL.to_vec(),
        };
        for _ in 0..repeats {
            for &suit in &active_suits {
                for v in 1..=13u8 {
                    deck.push(Card::new(suit, v));
                }
            }
        }
        assert_eq!(deck.len(), 104);
        deck
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_deck() {
        let deck = Card::standard_deck();
        assert_eq!(deck.len(), 52);
        // All unique
        let mut set = std::collections::HashSet::new();
        for c in &deck {
            assert!(set.insert(*c));
        }
    }

    #[test]
    fn test_spider_deck_1suit() {
        let deck = Card::spider_deck(1);
        assert_eq!(deck.len(), 104);
        assert!(deck.iter().all(|c| c.suit == Suit::Spade));
    }

    #[test]
    fn test_spider_deck_2suit() {
        let deck = Card::spider_deck(2);
        assert_eq!(deck.len(), 104);
        let spades = deck.iter().filter(|c| c.suit == Suit::Spade).count();
        let hearts = deck.iter().filter(|c| c.suit == Suit::Heart).count();
        assert_eq!(spades, 52);
        assert_eq!(hearts, 52);
    }

    #[test]
    fn test_spider_deck_4suit() {
        let deck = Card::spider_deck(4);
        assert_eq!(deck.len(), 104);
    }

    #[test]
    fn test_encode_decode() {
        for &suit in &Suit::ALL {
            for v in 1..=13u8 {
                let c = Card::new(suit, v);
                let encoded = c.encode();
                let decoded = Card::decode(encoded).unwrap();
                assert_eq!(c, decoded);
            }
        }
    }

    #[test]
    fn test_color() {
        assert_eq!(Suit::Club.color(), Color::Black);
        assert_eq!(Suit::Spade.color(), Color::Black);
        assert_eq!(Suit::Diamond.color(), Color::Red);
        assert_eq!(Suit::Heart.color(), Color::Red);
    }
}
