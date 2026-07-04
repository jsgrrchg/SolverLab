use crate::common::card::Card;

/// Una columna de Spider Solitaire: cartas boca abajo y cartas boca arriba.
#[derive(Debug, Clone)]
pub struct SpiderColumn {
    pub face_down: Vec<Card>,
    pub face_up: Vec<Card>,
    /// Cache: racha descendente más larga del mismo palo desde el tope de `face_up`.
    pub longest_run: usize,
}

impl SpiderColumn {
    /// Crea una columna y calcula la racha descendente del mismo palo desde el tope.
    pub fn new(face_down: Vec<Card>, face_up: Vec<Card>) -> SpiderColumn {
        // Volteo automático: si `face_down` tiene cartas pero `face_up` está vacío, voltea la última carta.
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

    /// Construye una columna vacía.
    pub fn empty() -> SpiderColumn {
        SpiderColumn {
            face_down: vec![],
            face_up: vec![],
            longest_run: 0,
        }
    }

    /// Indica si la columna no tiene cartas.
    pub fn is_empty(&self) -> bool {
        self.face_down.is_empty() && self.face_up.is_empty()
    }

    /// Indica si la columna tiene cartas boca abajo.
    pub fn has_face_down(&self) -> bool {
        !self.face_down.is_empty()
    }

    /// Indica si la columna tiene cartas boca arriba.
    pub fn has_face_up(&self) -> bool {
        !self.face_up.is_empty()
    }

    /// Cantidad de cartas boca abajo.
    pub fn num_face_down(&self) -> usize {
        self.face_down.len()
    }

    /// Cantidad de cartas boca arriba.
    pub fn num_face_up(&self) -> usize {
        self.face_up.len()
    }

    /// Cantidad total de cartas en la columna.
    pub fn total_cards(&self) -> usize {
        self.face_down.len() + self.face_up.len()
    }

    /// Carta superior de `face_up`, si existe.
    pub fn top_card(&self) -> Option<Card> {
        self.face_up.last().copied()
    }

    /// Devuelve las N cartas superiores (las últimas N de `face_up`).
    pub fn top_cards(&self, count: usize) -> Option<Vec<Card>> {
        if count < 1 || count > self.face_up.len() {
            return None;
        }
        let start = self.face_up.len() - count;
        Some(self.face_up[start..].to_vec())
    }

    /// Extrae desde el tope una racha descendente del mismo palo.
    pub fn extract_same_suit_run(&self, count: usize) -> Option<(Vec<Card>, SpiderColumn)> {
        let run = self.top_cards(count)?;
        if !is_same_suit_descending(&run) {
            return None;
        }
        let new_face_up = self.face_up[..self.face_up.len() - count].to_vec();
        Some((run, SpiderColumn::new(self.face_down.clone(), new_face_up)))
    }

    /// Extrae una sola carta del tope.
    pub fn extract_card(&self) -> Option<(Card, SpiderColumn)> {
        let card = *self.face_up.last()?;
        let new_face_up = self.face_up[..self.face_up.len() - 1].to_vec();
        Some((card, SpiderColumn::new(self.face_down.clone(), new_face_up)))
    }

    /// Agrega varias cartas al tope.
    pub fn with_cards(&self, new_cards: &[Card]) -> Option<SpiderColumn> {
        if new_cards.is_empty() {
            return None;
        }
        let mut fu = self.face_up.clone();
        fu.extend_from_slice(new_cards);
        Some(SpiderColumn::new(self.face_down.clone(), fu))
    }

    /// Agrega una sola carta al tope.
    pub fn with_card(&self, card: Card) -> SpiderColumn {
        let mut fu = self.face_up.clone();
        fu.push(card);
        SpiderColumn::new(self.face_down.clone(), fu)
    }

    /// Verifica si una carta puede colocarse en esta columna.
    /// Una columna vacía acepta cualquier carta; si no, la carta debe ser exactamente 1 valor menor al tope.
    #[inline]
    pub fn can_add_card(&self, card: Card) -> bool {
        if self.face_up.is_empty() && self.face_down.is_empty() {
            return true; // Una columna vacía acepta cualquier carta.
        }
        match self.face_up.last() {
            Some(top) => top.value == card.value + 1,
            None => false,
        }
    }

    /// Verifica si una racha puede colocarse en esta columna.
    pub fn can_add_run(&self, run: &[Card]) -> bool {
        match run.first() {
            Some(first) => self.can_add_card(*first),
            None => false,
        }
    }

    /// Cuenta transiciones de palo en face_up (cartas adyacentes de distinto palo).
    /// En 1-suit siempre devuelve 0. Útil para penalización de fragmentación.
    pub fn suit_transitions(&self) -> usize {
        self.face_up
            .windows(2)
            .filter(|w| w[0].suit != w[1].suit)
            .count()
    }

    /// Revisa y elimina del tope secuencias completas K→A del mismo palo.
    /// Devuelve `(nueva_columna, cantidad_de_secuencias_eliminadas)`.
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

            // Si `face_up` queda vacío, voltea una carta desde `face_down`.
            if current_face_up.is_empty() && !current_face_down.is_empty() {
                let top = current_face_down.pop().unwrap();
                current_face_up.push(top);
            }
        }

        let new_col = SpiderColumn::new(current_face_down, current_face_up);
        (new_col, removed_sets)
    }
}

// `PartialEq` y `Hash` personalizados que excluyen el campo cacheado `longest_run`.
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

/// Calcula la racha descendente más larga del mismo palo desde el tope (final) de `face_up`.
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

/// Verifica si las cartas forman una secuencia descendente del mismo palo.
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

/// Verifica in-place si 13 cartas desde `start` forman una secuencia K→A del mismo palo.
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
        // K, Q, J de espadas = racha de 3.
        let col = SpiderColumn::new(
            vec![],
            vec![
                card(Suit::Heart, 5), // No forma parte de la racha.
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
        assert!(col.can_add_card(card(Suit::Heart, 4))); // Palo distinto permitido en Spider.
        assert!(col.can_add_card(card(Suit::Spade, 4))); // Mismo palo también permitido.
        assert!(!col.can_add_card(card(Suit::Spade, 3))); // Valor incorrecto.
        assert!(!col.can_add_card(card(Suit::Spade, 5))); // Mismo valor.
    }

    #[test]
    fn test_complete_sequence() {
        // Construye una secuencia K→A de espadas.
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
