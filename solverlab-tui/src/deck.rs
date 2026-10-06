//! Deck builders matching the Swift console, plus shuffling with an optional
//! reproducible seed so runs can be compared across platforms.

use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;
use solver_core::common::card::{Card, Suit};

const STANDARD_SUITS: [Suit; 4] = [Suit::Club, Suit::Diamond, Suit::Heart, Suit::Spade];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShuffleSource {
    #[default]
    Random,
    Seeded(u64),
}

fn build(suits: &[Suit], repeats: usize) -> Vec<Card> {
    let mut deck = Vec::with_capacity(suits.len() * repeats * 13);
    for _ in 0..repeats {
        for &suit in suits {
            for value in 1..=13u8 {
                deck.push(Card::new(suit, value));
            }
        }
    }
    deck
}

pub fn standard_deck() -> Vec<Card> {
    build(&STANDARD_SUITS, 1)
}

/// Spider deck in the Swift console order (not `Card::spider_deck`, whose order
/// differs and would change seeded deals).
pub fn spider_deck(suit_count: u32) -> Vec<Card> {
    match suit_count {
        1 => build(&[Suit::Spade], 8),
        2 => build(&[Suit::Spade, Suit::Heart], 4),
        _ => build(&STANDARD_SUITS, 2),
    }
}

fn splitmix64(mut state: u64) -> u64 {
    state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub fn shuffled(mut deck: Vec<Card>, source: ShuffleSource, game_id: u32) -> Vec<Card> {
    match source {
        ShuffleSource::Random => deck.shuffle(&mut rand::rng()),
        ShuffleSource::Seeded(seed) => {
            let mut rng = ChaCha8Rng::seed_from_u64(splitmix64(seed ^ u64::from(game_id)));
            deck.shuffle(&mut rng);
        }
    }
    deck
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn counts(deck: &[Card]) -> HashMap<(u8, u8), usize> {
        let mut map = HashMap::new();
        for card in deck {
            *map.entry((card.suit as u8, card.value)).or_default() += 1;
        }
        map
    }

    #[test]
    fn standard_deck_has_52_unique_cards_in_swift_order() {
        let deck = standard_deck();
        assert_eq!(deck.len(), 52);
        assert!(counts(&deck).values().all(|&c| c == 1));
        assert_eq!(deck[0], Card::new(Suit::Club, 1));
        assert_eq!(deck[13], Card::new(Suit::Diamond, 1));
        assert_eq!(deck[51], Card::new(Suit::Spade, 13));
    }

    #[test]
    fn spider_decks_have_expected_composition() {
        let one = spider_deck(1);
        assert_eq!(one.len(), 104);
        assert!(one.iter().all(|c| c.suit == Suit::Spade));
        assert!(counts(&one).values().all(|&c| c == 8));

        let two = spider_deck(2);
        assert_eq!(two.len(), 104);
        assert_eq!(two.iter().filter(|c| c.suit == Suit::Heart).count(), 52);
        assert!(counts(&two).values().all(|&c| c == 4));
        assert_eq!(two[13], Card::new(Suit::Heart, 1));

        let four = spider_deck(4);
        assert_eq!(four.len(), 104);
        assert_eq!(counts(&four).len(), 52);
        assert!(counts(&four).values().all(|&c| c == 2));
    }

    #[test]
    fn seeded_shuffle_is_reproducible_per_game() {
        let a = shuffled(standard_deck(), ShuffleSource::Seeded(42), 7);
        let b = shuffled(standard_deck(), ShuffleSource::Seeded(42), 7);
        let c = shuffled(standard_deck(), ShuffleSource::Seeded(42), 8);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(counts(&a), counts(&standard_deck()));
    }

    #[test]
    fn random_shuffle_preserves_cards() {
        let deck = shuffled(spider_deck(2), ShuffleSource::Random, 1);
        assert_eq!(counts(&deck), counts(&spider_deck(2)));
    }

    /// Golden values: if this passes on every platform, seeded runs deal the
    /// same boards everywhere.
    #[test]
    fn seeded_shuffle_golden_values() {
        let deck = shuffled(standard_deck(), ShuffleSource::Seeded(1), 1);
        let head: Vec<u8> = deck.iter().take(5).map(|c| c.encode()).collect();
        assert_eq!(head, GOLDEN_HEAD);
    }

    const GOLDEN_HEAD: [u8; 5] = [51, 55, 41, 19, 35];
}
