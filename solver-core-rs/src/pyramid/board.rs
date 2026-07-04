use crate::common::card::Card;
use std::collections::HashSet;

/// Tablero de Pirámide: 28 posiciones en la pirámide, stock, descarte y fundación.
#[derive(Debug, Clone)]
pub struct PyramidBoard {
    /// 28 posiciones. Some = carta presente, None = retirada.
    pub pyramid: Vec<Option<Card>>,
    /// Stock boca abajo (tope = último).
    pub stock: Vec<Card>,
    /// Descarte boca arriba (tope = último).
    pub waste: Vec<Card>,
    /// Cartas retiradas.
    pub foundation: Vec<Card>,
    /// Firma hash precalculada del estado.
    pub signature: u64,
}

impl PyramidBoard {
    /// Cantidad total de posiciones de la pirámide (7 filas = 28 cartas).
    pub const PYRAMID_SIZE: usize = 28;

    /// Crea un tablero de Pirámide y calcula su firma hash inicial.
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

    /// Fila para el índice dado (0-6).
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

    /// Índice del hijo izquierdo, si existe.
    pub fn left_child(index: usize) -> Option<usize> {
        let row = Self::row(index);
        if row >= 6 {
            return None;
        }
        let column = index - (row * (row + 1)) / 2;
        Some(((row + 1) * (row + 2)) / 2 + column)
    }

    /// Índice del hijo derecho, si existe.
    pub fn right_child(index: usize) -> Option<usize> {
        Self::left_child(index).map(|l| l + 1)
    }

    /// Verdadero si la posición tiene carta y ambos hijos están vacíos.
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

    /// Devuelve los índices actualmente expuestos de la pirámide.
    pub fn exposed_pyramid_indices(&self) -> Vec<usize> {
        (0..self.pyramid.len())
            .filter(|&i| self.is_exposed(i))
            .collect()
    }

    /// Carta del tope del descarte.
    pub fn waste_top(&self) -> Option<Card> {
        self.waste.last().copied()
    }
    /// Carta del tope del stock.
    pub fn stock_top(&self) -> Option<Card> {
        self.stock.last().copied()
    }
    /// Indica si se puede avanzar del stock al descarte.
    pub fn can_advance_stock(&self) -> bool {
        !self.stock.is_empty()
    }
    /// Indica si se puede reciclar el descarte al stock.
    pub fn can_reset_stock(&self) -> bool {
        self.stock.is_empty() && !self.waste.is_empty()
    }

    /// Cantidad de cartas restantes en la pirámide.
    pub fn remaining_pyramid(&self) -> usize {
        self.pyramid.iter().filter(|c| c.is_some()).count()
    }
}

/// Movimientos de Pirámide.
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
    /// Verdadero si el movimiento es reciclar stock.
    pub fn is_stock_reset(&self) -> bool {
        matches!(self, PyramidMove::StockReset)
    }
    /// Verdadero si el movimiento es avanzar stock.
    pub fn is_stock_advance(&self) -> bool {
        matches!(self, PyramidMove::StockAdvance)
    }
    /// Verdadero si el movimiento retira un rey.
    pub fn is_king_move(&self) -> bool {
        matches!(
            self,
            PyramidMove::KingToFoundationPyramid { .. } | PyramidMove::KingToFoundationWaste
        )
    }
    /// Verdadero si el movimiento retira una pareja que suma 13.
    pub fn is_pair_move(&self) -> bool {
        matches!(
            self,
            PyramidMove::RemovePairWastePyramid { .. }
                | PyramidMove::RemovePairPyramidPyramid { .. }
        )
    }

    /// Aplica el movimiento a un tablero.
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
    // Reparte 28 cartas a la pirámide y deja el resto en el stock.
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

/// Permite retirar una pareja donde una carta puede estar cubierta por la otra.
fn can_remove_pair(board: &PyramidBoard, first: usize, second: usize) -> bool {
    if board.is_exposed(first) && board.is_exposed(second) {
        return true;
    }
    if can_use_covered_when_source_removed(board, first, second) {
        return true;
    }
    can_use_covered_when_source_removed(board, second, first)
}

/// Regla especial: puede retirarse una carta cubierta si su única bloqueadora es la carta origen.
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

// -- Generación de movimientos --

impl PyramidMove {
    /// Genera el movimiento de avance de stock si está disponible.
    pub fn find_stock_advance_moves(board: &PyramidBoard) -> Vec<PyramidMove> {
        if board.can_advance_stock() {
            vec![PyramidMove::StockAdvance]
        } else {
            vec![]
        }
    }

    /// Genera el movimiento de reciclado de stock si está disponible.
    pub fn find_stock_reset_moves(board: &PyramidBoard) -> Vec<PyramidMove> {
        if board.can_reset_stock() {
            vec![PyramidMove::StockReset]
        } else {
            vec![]
        }
    }

    /// Genera movimientos para retirar reyes expuestos (pirámide o descarte).
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

    /// Genera parejas válidas entre descarte y pirámide que sumen 13.
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

    /// Genera parejas válidas dentro de la pirámide que sumen 13.
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

    /// Encuentra todos los movimientos disponibles.
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

// Igualdad personalizada que ignora la firma.
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
    // FNV-1a de 64 bits para firma estable y rápida del estado del tablero.
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;

    let mut h: u64 = FNV_OFFSET;

    for (idx, card) in pyramid.iter().enumerate() {
        h ^= (idx as u64).wrapping_add(0x100);
        h = h.wrapping_mul(FNV_PRIME);
        h ^= encode_opt_card(*card);
        h = h.wrapping_mul(FNV_PRIME);
    }

    h ^= 0xFD; // separador
    h = h.wrapping_mul(FNV_PRIME);

    for card in stock {
        h ^= encode_card(*card);
        h = h.wrapping_mul(FNV_PRIME);
    }

    h ^= 0xFE; // separador
    h = h.wrapping_mul(FNV_PRIME);

    for card in waste {
        h ^= encode_card(*card);
        h = h.wrapping_mul(FNV_PRIME);
    }

    h ^= 0xFC; // separador
    h = h.wrapping_mul(FNV_PRIME);

    for card in foundation {
        h ^= encode_card(*card);
        h = h.wrapping_mul(FNV_PRIME);
    }

    h
}

#[inline]
fn encode_card(card: Card) -> u64 {
    // Codificación compacta: valor en bits altos, palo en bajos.
    let suit_val: u64 = card.suit as u64;
    ((card.value as u64) << 4) | suit_val
}

#[inline]
fn encode_opt_card(card: Option<Card>) -> u64 {
    // 0xFF representa ausencia de carta.
    match card {
        Some(c) => encode_card(c),
        None => 0xFF,
    }
}
