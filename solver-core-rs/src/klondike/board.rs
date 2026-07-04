use crate::common::card::{Card, Suit};

// ═══════════════════════════════════════════
// FastColumn
// ═══════════════════════════════════════════

/// Columna del tableau de Klondike, representada como array estático para
/// evitar allocations en el heap. Las cartas se almacenan en orden de base
/// a tope: `cards[0]` es la carta más profunda de la columna.
///
/// Layout de memoria:
/// ```text
/// cards[0 .. face_down_len]   → cartas boca abajo (ocultas)
/// cards[face_down_len .. len] → cartas boca arriba (visibles)
/// ```
/// La capacidad máxima es 21 cartas (7 iniciales + hasta 14 movidas encima).
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct FastColumn {
    pub cards: [Card; 21],
    /// Número total de cartas en la columna (boca abajo + boca arriba).
    pub len: u8,
    /// Número de cartas boca abajo. Siempre <= len.
    pub face_down_len: u8,
}

impl FastColumn {
    /// Crea una columna vacía. Las cartas se inicializan con un valor nulo (Club, 0).
    pub fn empty() -> Self {
        Self {
            cards: [Card::new(Suit::Club, 0); 21],
            len: 0,
            face_down_len: 0,
        }
    }

    /// Agrega una carta al tope de la columna.
    /// Si `face_down` es true, también incrementa `face_down_len`.
    pub fn push(&mut self, card: Card, face_down: bool) {
        self.cards[self.len as usize] = card;
        self.len += 1;
        if face_down {
            self.face_down_len += 1;
        }
    }

    /// Extrae y retorna la carta del tope. Retorna None si la columna está vacía.
    /// Si al retirar la carta el tope queda en zona boca abajo, ajusta face_down_len
    /// (caso de columna que queda con más face_down que cartas totales).
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

    /// Elimina `count` cartas del tope sin retornarlas.
    /// Usado para mover stacks completos entre columnas.
    pub fn pop_count(&mut self, count: u8) {
        if count <= self.len {
            self.len -= count;
            if self.face_down_len > self.len {
                self.face_down_len = self.len;
            }
        }
    }

    /// Retorna la carta del tope sin importar si está boca arriba o boca abajo.
    /// Útil para operaciones genéricas de inspección.
    pub fn top(&self) -> Option<Card> {
        if self.len > 0 {
            Some(self.cards[(self.len - 1) as usize])
        } else {
            None
        }
    }

    /// Retorna la carta del tope solo si está boca arriba.
    /// Retorna None si la columna está vacía o si la carta del tope es boca abajo
    /// (situación que no ocurre en un tablero válido, pero se verifica por seguridad).
    pub fn top_face_up(&self) -> Option<Card> {
        if self.len > self.face_down_len {
            Some(self.cards[(self.len - 1) as usize])
        } else {
            None
        }
    }

    /// Verifica si `run_base` puede colocarse en el tope de esta columna
    /// siguiendo las reglas de Klondike: color alternante y valor descendente.
    /// Si la columna está vacía (o toda boca abajo), solo acepta un Rey (valor 13).
    pub fn can_add_run(&self, run_base: Card) -> bool {
        if self.len == self.face_down_len {
            // Columna sin cartas boca arriba: solo acepta Rey
            return run_base.value == 13;
        }
        let top = self.cards[(self.len - 1) as usize];
        top.value == run_base.value + 1 && top.color() != run_base.color()
    }

    // ── Consultas de estado ─────────────────

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
    /// Retorna un slice con solo las cartas boca arriba (en orden base→tope).
    pub fn face_up_cards(&self) -> &[Card] {
        &self.cards[self.face_down_len as usize..self.len as usize]
    }
}

// ═══════════════════════════════════════════
// KlondikeBoard
// ═══════════════════════════════════════════

/// Estado completo de un tablero de Klondike. Diseñado para ser `Copy`
/// y caber en el stack, permitiendo que IDA* clone estados sin allocations.
///
/// # Modelo del stock
/// Las cartas del stock se almacenan en `stock[0..stock_len]`.
/// `stock_index` es un índice 1-based que apunta a la posición actual
/// del waste pile (la carta accesible es `stock[stock_index - 1]`).
///
/// - `stock_index == 0`: no hay carta accesible (estado inicial o post-recycle).
/// - Avanzar el stock incrementa `stock_index` en `draw_advance` (1 o 3).
/// - Reciclar resetea `stock_index` a 0.
/// - Cuando se juega la carta del pile, se extrae del array y el resto se compacta.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct KlondikeBoard {
    /// Las 7 columnas del tableau.
    pub columns: [FastColumn; 7],
    /// Rango más alto depositado en cada foundation, indexado por Suit as usize.
    /// Valor 0 = foundation vacía; valor 13 = foundation completa.
    pub foundation: [u8; 4],
    /// Array compacto de cartas restantes en el stock (waste incluido).
    pub stock: [Card; 24],
    /// Número de cartas actualmente en el stock array.
    pub stock_len: u8,
    /// Posición 1-based del tope del waste pile. La carta jugable es stock[stock_index-1].
    pub stock_index: u8,
    /// Número de veces que el stock ha sido reciclado. Usado por reglas opcionales.
    pub stock_recycles: u8,
    /// Hash FNV-1a del estado completo. Usado como clave en la transposition table.
    pub signature: u64,
}

// El hash del tablero delega directamente en la firma precomputada.
impl std::hash::Hash for KlondikeBoard {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.signature);
    }
}

impl KlondikeBoard {
    pub const NUM_COLUMNS: usize = 7;

    /// Crea un tablero vacío sin cartas. Usado como punto de partida para `deal`.
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

    // ── Consultas de foundation ─────────────

    /// Retorna la carta más alta de la foundation de `suit`, o None si está vacía.
    pub fn top_of_foundation(&self, suit: Suit) -> Option<Card> {
        let val = self.foundation[suit as usize];
        if val == 0 {
            None
        } else {
            Some(Card::new(suit, val))
        }
    }

    /// Número de cartas en la foundation de `suit` (0–13).
    pub fn foundation_count(&self, suit: Suit) -> usize {
        self.foundation[suit as usize] as usize
    }

    /// Total de cartas en todas las foundations (0–52). Valor 52 = victoria.
    pub fn total_foundation_count(&self) -> usize {
        self.foundation.iter().map(|&x| x as usize).sum()
    }

    /// True si `card` puede colocarse en su foundation (es exactamente el siguiente valor).
    pub fn can_add_to_foundation(&self, card: Card) -> bool {
        self.foundation[card.suit as usize] + 1 == card.value
    }

    // ── Operaciones de stock ────────────────

    /// Extrae la carta del tope del waste pile (stock[stock_index-1]) y compacta el array.
    /// Retorna la carta extraída junto con el nuevo estado del tablero.
    /// Retorna None si no hay carta accesible en el pile.
    pub fn extract_stock_pile_card(&self) -> Option<(Card, KlondikeBoard)> {
        if self.stock_index == 0 || self.stock_index > self.stock_len {
            return None;
        }
        let real_idx = (self.stock_index - 1) as usize;
        let card = self.stock[real_idx];

        let mut n = *self;
        // Compactar: desplazar las cartas restantes una posición hacia atrás
        for i in real_idx..n.stock_len as usize - 1 {
            n.stock[i] = n.stock[i + 1];
        }
        n.stock_len -= 1;
        n.stock_index -= 1;
        Some((card, n))
    }

    /// True si hay cartas sin voltear en el stock (se puede avanzar).
    pub fn can_advance_stock(&self) -> bool {
        self.stock_index < self.stock_len
    }

    /// True si el stock fue completamente avanzado y puede reciclarse.
    pub fn can_recycle_stock(&self) -> bool {
        self.stock_len > 0 && self.stock_index >= self.stock_len
    }

    /// Retorna la carta actualmente accesible del waste pile, sin modificar el estado.
    pub fn stock_pile_card(&self) -> Option<Card> {
        if self.stock_index > 0 && self.stock_index <= self.stock_len {
            Some(self.stock[(self.stock_index - 1) as usize])
        } else {
            None
        }
    }

    // ── Firma / Hash ────────────────────────

    /// Recalcula la firma FNV-1a del tablero y la almacena en `self.signature`.
    /// Debe llamarse después de cualquier modificación al estado del tablero
    /// que no sea producida por `KlondikeMove::apply` (que lo hace automáticamente).
    pub fn compute_signature(&mut self) {
        self.signature = klondike_signature(self);
    }
}

// ═══════════════════════════════════════════
// Firma del tablero (FNV-1a)
// ═══════════════════════════════════════════

/// Calcula un hash FNV-1a (Fowler–Noll–Vo) del estado completo del tablero.
///
/// El hash incluye:
/// 1. Cada columna: primero las cartas boca abajo (en orden), luego las boca arriba.
///    Un separador (0xFE) distingue la frontera entre ambas zonas.
/// 2. El stock completo (cartas en orden) más la posición actual (stock_index).
/// 3. El valor más alto de cada foundation por suit.
///
/// Se usan prefijos distintos por sección (0x100 para columnas, 0x200 para stock,
/// 0x300 para foundations) para evitar colisiones entre estados con los mismos
/// bytes en distinto contexto.
fn klondike_signature(board: &KlondikeBoard) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;
    let mut h = FNV_OFFSET;

    // Columnas del tableau
    for (col_idx, col) in board.columns.iter().enumerate() {
        h ^= (col_idx as u64).wrapping_add(0x100);
        h = h.wrapping_mul(FNV_PRIME);
        // Cartas boca abajo
        for i in 0..col.face_down_len {
            h ^= encode_card(col.cards[i as usize]);
            h = h.wrapping_mul(FNV_PRIME);
        }
        // Separador de zona boca abajo / boca arriba
        h ^= 0xFE;
        h = h.wrapping_mul(FNV_PRIME);
        // Cartas boca arriba
        for i in col.face_down_len..col.len {
            h ^= encode_card(col.cards[i as usize]);
            h = h.wrapping_mul(FNV_PRIME);
        }
    }

    // Separador entre columnas y stock
    h ^= 0xFD;
    h = h.wrapping_mul(FNV_PRIME);

    // Stock (cartas en orden + posición actual)
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

/// Codifica una carta en 8 bits: [value (4 bits) | suit (4 bits)].
/// Versión inline para maximizar rendimiento en el loop de firma.
#[inline(always)]
fn encode_card(card: Card) -> u64 {
    let suit_val: u64 = card.suit as u64;
    ((card.value as u64) << 4) | suit_val
}

// ═══════════════════════════════════════════
// Deal
// ═══════════════════════════════════════════

/// Construye el tablero inicial a partir de un deck de 52 cartas.
///
/// Distribución estándar de Klondike:
/// - Columna 0: 1 carta boca arriba
/// - Columna 1: 1 boca abajo + 1 boca arriba
/// - Columna k: k boca abajo + 1 boca arriba (en el tope)
/// - Las 24 cartas restantes van al stock (boca abajo, listas para avanzar).
///
/// Retorna None si el deck no tiene exactamente 52 cartas.
pub fn deal(deck: &[Card]) -> Option<KlondikeBoard> {
    if deck.len() != 52 {
        return None;
    }
    let mut board = KlondikeBoard::new();
    let mut d_idx = 0;

    // Repartir 28 cartas al tableau (1+2+3+4+5+6+7)
    for col in 0..7 {
        for row in 0..=col {
            let is_down = row < col; // Solo la última carta de cada columna queda boca arriba
            board.columns[col].push(deck[d_idx], is_down);
            d_idx += 1;
        }
    }

    // Las 24 cartas restantes forman el stock inicial
    while d_idx < 52 {
        board.stock[board.stock_len as usize] = deck[d_idx];
        board.stock_len += 1;
        d_idx += 1;
    }

    board.compute_signature();
    Some(board)
}
