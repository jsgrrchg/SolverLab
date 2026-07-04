pub mod common;
pub mod freecell;
pub mod klondike;
pub mod pyramid;
pub mod spider;
pub mod tripeaks;

use common::card::{Card, Suit};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

// ─── Error type ───

#[derive(Debug, thiserror::Error)]
pub enum SolverError {
    #[error("Invalid deck")]
    InvalidDeck,
    #[error("Deal failed")]
    DealFailed,
}

// ─── FFI Move descriptors ───

#[derive(Debug, Clone)]
pub struct SpiderMoveDesc {
    pub move_type: String,
    pub source: i32,
    pub destination: i32,
    pub card_count: i32,
}

#[derive(Debug, Clone)]
pub struct KlondikeMoveDesc {
    pub move_type: String,
    pub source: i32,
    pub destination: i32,
    pub card_count: i32,
    pub card_suit: u8,
    pub card_value: u8,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum KlondikeSearchMode {
    Fast,
    Strict,
}

#[derive(Debug, Clone)]
pub struct FreeCellMoveDesc {
    pub move_type: String,
    pub source: i32,
    pub destination: i32,
    pub card_count: i32,
    pub card_suit: u8,
    pub card_value: u8,
}

#[derive(Debug, Clone)]
pub struct PyramidMoveDesc {
    pub move_type: String,
    pub index_a: i32,
    pub index_b: i32,
}

#[derive(Debug, Clone)]
pub struct TriPeaksMoveDesc {
    pub move_type: String,
    pub tableau_index: i32,
}

fn nonneg_to_usize(value: i32) -> Option<usize> {
    usize::try_from(value).ok()
}

fn positive_to_usize(value: i32) -> Option<usize> {
    if value <= 0 {
        return None;
    }
    usize::try_from(value).ok()
}

fn bounded_index(value: i32, len: usize) -> Option<usize> {
    let idx = nonneg_to_usize(value)?;
    if idx < len {
        Some(idx)
    } else {
        None
    }
}

#[allow(dead_code)]
fn bounded_index_inclusive(value: i32, len: usize) -> Option<usize> {
    let idx = nonneg_to_usize(value)?;
    if idx <= len {
        Some(idx)
    } else {
        None
    }
}

// ═══════════════════════════════════════════
// Spider
// ═══════════════════════════════════════════

pub struct SpiderBoard {
    inner: spider::board::SpiderBoard,
}

impl SpiderBoard {}

pub struct SpiderEngine {
    solver: spider::solver::SpiderSolver,
    #[allow(dead_code)]
    suit_count: u32,
    last_checkpoints_adopted: AtomicU32,
}

impl SpiderEngine {
    pub fn new(suit_count: u32) -> Self {
        SpiderEngine {
            solver: spider::solver::SpiderSolver::new(suit_count),
            suit_count,
            last_checkpoints_adopted: AtomicU32::new(0),
        }
    }

    pub fn deal(&self, deck: Vec<Card>) -> Result<Arc<SpiderBoard>, SolverError> {
        if deck.len() != 104 {
            return Err(SolverError::InvalidDeck);
        }
        let board = spider::engine::deal(&deck, spider::board::SpiderBoard::NUM_COLUMNS)
            .ok_or(SolverError::DealFailed)?;
        Ok(Arc::new(SpiderBoard { inner: board }))
    }

    pub fn solve(&self, board: &SpiderBoard, allow_partial: bool) -> Option<Vec<SpiderMoveDesc>> {
        self.last_checkpoints_adopted.store(0, Ordering::Relaxed);
        let stats = self.solver.solve_with_stats(&board.inner, allow_partial);
        self.last_checkpoints_adopted
            .store(stats.checkpoints_adopted, Ordering::Relaxed);
        let moves = stats.moves?;
        Some(moves.iter().map(spider_move_to_desc).collect())
    }

    pub fn last_checkpoints_adopted(&self) -> u32 {
        self.last_checkpoints_adopted.load(Ordering::Relaxed)
    }

    pub fn apply_move(
        &self,
        board: &SpiderBoard,
        move_desc: SpiderMoveDesc,
    ) -> Option<Arc<SpiderBoard>> {
        let the_move = desc_to_spider_move(&move_desc, &board.inner)?;
        let new_board = the_move.apply(&board.inner)?;
        Some(Arc::new(SpiderBoard { inner: new_board }))
    }

    pub fn is_win(&self, board: &SpiderBoard) -> bool {
        spider::engine::SpiderEngine::is_win(&board.inner)
    }

    pub fn progress_token(&self, board: &SpiderBoard) -> String {
        spider::solver::progress_token(&board.inner)
    }

    pub fn completed_sets(&self, board: &SpiderBoard) -> i32 {
        board.inner.completed_sets as i32
    }
}

fn spider_move_to_desc(m: &spider::moves::SpiderMove) -> SpiderMoveDesc {
    match m {
        spider::moves::SpiderMove::DealFromStock => SpiderMoveDesc {
            move_type: "dealFromStock".into(),
            source: -1,
            destination: -1,
            card_count: 0,
        },
        spider::moves::SpiderMove::ColumnToColumn {
            source,
            destination,
            card_count,
        } => SpiderMoveDesc {
            move_type: "columnToColumn".into(),
            source: *source as i32,
            destination: *destination as i32,
            card_count: *card_count as i32,
        },
        spider::moves::SpiderMove::Deal { .. } => SpiderMoveDesc {
            move_type: "deal".into(),
            source: -1,
            destination: -1,
            card_count: 0,
        },
    }
}

fn desc_to_spider_move(
    desc: &SpiderMoveDesc,
    board: &spider::board::SpiderBoard,
) -> Option<spider::moves::SpiderMove> {
    match desc.move_type.as_str() {
        "dealFromStock" => Some(spider::moves::SpiderMove::DealFromStock),
        "columnToColumn" => {
            let src = bounded_index(desc.source, board.columns.len())?;
            let dest = bounded_index(desc.destination, board.columns.len())?;
            let count = positive_to_usize(desc.card_count)?;
            Some(spider::moves::SpiderMove::ColumnToColumn {
                source: src,
                destination: dest,
                card_count: count,
            })
        }
        _ => None,
    }
}

// ═══════════════════════════════════════════
// Klondike
// ═══════════════════════════════════════════

pub struct KlondikeBoard {
    inner: klondike::board::KlondikeBoard,
}

impl KlondikeBoard {}

pub struct KlondikeEngine {
    solver: klondike::solver::KlondikeSolver,
    last_checkpoints_adopted: AtomicU32,
}

impl KlondikeEngine {
    pub fn new(draw_advance: i32) -> Self {
        Self::new_with_mode(draw_advance, KlondikeSearchMode::Fast)
    }

    pub fn new_with_mode(draw_advance: i32, mode: KlondikeSearchMode) -> Self {
        let solve_mode = match mode {
            KlondikeSearchMode::Fast => klondike::solver::KlondikeSolveMode::Fast,
            KlondikeSearchMode::Strict => klondike::solver::KlondikeSolveMode::Strict,
        };
        KlondikeEngine {
            solver: klondike::solver::KlondikeSolver::new_with_mode(draw_advance, solve_mode),
            last_checkpoints_adopted: AtomicU32::new(0),
        }
    }

    pub fn deal(&self, deck: Vec<Card>) -> Result<Arc<KlondikeBoard>, SolverError> {
        if deck.len() != 52 {
            return Err(SolverError::InvalidDeck);
        }
        let board = klondike::solver::deal(&deck).ok_or(SolverError::DealFailed)?;
        Ok(Arc::new(KlondikeBoard { inner: board }))
    }

    pub fn solve(
        &self,
        board: &KlondikeBoard,
        allow_partial: bool,
    ) -> Option<Vec<KlondikeMoveDesc>> {
        self.last_checkpoints_adopted.store(0, Ordering::Relaxed);
        let stats = self.solver.solve_with_stats(&board.inner, allow_partial);
        self.last_checkpoints_adopted
            .store(stats.checkpoints_adopted, Ordering::Relaxed);
        let moves = stats.moves?;
        Some(moves.iter().map(klondike_move_to_desc).collect())
    }

    pub fn last_checkpoints_adopted(&self) -> u32 {
        self.last_checkpoints_adopted.load(Ordering::Relaxed)
    }

    pub fn apply_move(
        &self,
        board: &KlondikeBoard,
        m: KlondikeMoveDesc,
    ) -> Option<Arc<KlondikeBoard>> {
        let the_move = desc_to_klondike_move(&m, &board.inner)?;
        let new_board = the_move.apply(board.inner)?;
        Some(Arc::new(KlondikeBoard { inner: new_board }))
    }

    pub fn is_win(&self, board: &KlondikeBoard) -> bool {
        klondike::solver::KlondikeSolver::is_win(&board.inner)
    }

    pub fn progress_token(&self, board: &KlondikeBoard) -> String {
        klondike::solver::progress_token(&board.inner)
    }

    pub fn find_foundation_moves(&self, board: &KlondikeBoard) -> Vec<KlondikeMoveDesc> {
        klondike::moves::find_column_to_foundation_moves(&board.inner)
            .iter()
            .chain(klondike::moves::find_stock_to_foundation_moves(&board.inner).iter())
            .map(klondike_move_to_desc)
            .collect()
    }

    pub fn is_from_foundation(&self, m: KlondikeMoveDesc) -> bool {
        m.move_type == "foundationToColumn"
    }

    pub fn is_to_foundation(&self, m: KlondikeMoveDesc) -> bool {
        m.move_type == "columnToFoundation" || m.move_type == "stockToFoundation"
    }

    pub fn is_to_column(&self, m: KlondikeMoveDesc) -> bool {
        m.move_type == "columnToColumn"
            || m.move_type == "stockToColumn"
            || m.move_type == "foundationToColumn"
    }

    pub fn face_down_count(&self, board: &KlondikeBoard, column: i32) -> i32 {
        if column < 0 || column as usize >= board.inner.columns.len() {
            return 0;
        }
        board.inner.columns[column as usize].num_face_down() as i32
    }
}

fn klondike_move_to_desc(m: &klondike::moves::KlondikeMove) -> KlondikeMoveDesc {
    match m {
        klondike::moves::KlondikeMove::ColumnToColumn {
            source,
            destination,
            count,
        } => KlondikeMoveDesc {
            move_type: "columnToColumn".into(),
            source: *source as i32,
            destination: *destination as i32,
            card_count: *count as i32,
            card_suit: 0,
            card_value: 0,
        },
        klondike::moves::KlondikeMove::ColumnToFoundation { source, card } => KlondikeMoveDesc {
            move_type: "columnToFoundation".into(),
            source: *source as i32,
            destination: -1,
            card_count: 1,
            card_suit: card.suit as u8,
            card_value: card.value,
        },
        klondike::moves::KlondikeMove::StockPileToColumn { destination, card } => {
            KlondikeMoveDesc {
                move_type: "stockToColumn".into(),
                source: -1,
                destination: *destination as i32,
                card_count: 1,
                card_suit: card.suit as u8,
                card_value: card.value,
            }
        }
        klondike::moves::KlondikeMove::StockPileToFoundation { card } => KlondikeMoveDesc {
            move_type: "stockToFoundation".into(),
            source: -1,
            destination: -1,
            card_count: 1,
            card_suit: card.suit as u8,
            card_value: card.value,
        },
        klondike::moves::KlondikeMove::FoundationToColumn { destination, card } => {
            KlondikeMoveDesc {
                move_type: "foundationToColumn".into(),
                source: card.suit as i32,
                destination: *destination as i32,
                card_count: 1,
                card_suit: card.suit as u8,
                card_value: card.value,
            }
        }
        klondike::moves::KlondikeMove::StockPileAdvance {
            beginning_index,
            increment,
        } => KlondikeMoveDesc {
            move_type: "stockAdvance".into(),
            source: *beginning_index as i32,
            destination: *increment as i32,
            card_count: 0,
            card_suit: 0,
            card_value: 0,
        },
        klondike::moves::KlondikeMove::StockPileRecycle { source_index } => KlondikeMoveDesc {
            move_type: "stockRecycle".into(),
            source: *source_index as i32,
            destination: -1,
            card_count: 0,
            card_suit: 0,
            card_value: 0,
        },
        klondike::moves::KlondikeMove::Deal => KlondikeMoveDesc {
            move_type: "deal".into(),
            source: -1,
            destination: -1,
            card_count: 0,
            card_suit: 0,
            card_value: 0,
        },
    }
}

fn desc_to_klondike_move(
    desc: &KlondikeMoveDesc,
    board: &klondike::board::KlondikeBoard,
) -> Option<klondike::moves::KlondikeMove> {
    match desc.move_type.as_str() {
        "columnToColumn" => {
            let src = desc.source as u8;
            let dest = desc.destination as u8;
            let count = desc.card_count as u8;
            Some(klondike::moves::KlondikeMove::ColumnToColumn {
                source: src,
                destination: dest,
                count,
            })
        }
        "columnToFoundation" => {
            let src = desc.source as u8;
            let card = board.columns[src as usize].top_face_up()?;
            Some(klondike::moves::KlondikeMove::ColumnToFoundation { source: src, card })
        }
        "stockToColumn" => {
            let dest = desc.destination as u8;
            let card = board.stock_pile_card()?;
            Some(klondike::moves::KlondikeMove::StockPileToColumn {
                card,
                destination: dest,
            })
        }
        "stockToFoundation" => {
            let card = board.stock_pile_card()?;
            Some(klondike::moves::KlondikeMove::StockPileToFoundation { card })
        }
        "foundationToColumn" => {
            let suit = Suit::from_u8(desc.card_suit)?;
            let card = board.top_of_foundation(suit)?;
            let dest = desc.destination as u8;
            Some(klondike::moves::KlondikeMove::FoundationToColumn {
                destination: dest,
                card,
            })
        }
        "stockAdvance" => {
            let beginning_index = desc.source as u8;
            let increment = desc.destination as u8;
            Some(klondike::moves::KlondikeMove::StockPileAdvance {
                beginning_index,
                increment,
            })
        }
        "stockRecycle" => {
            let source_index = desc.source as u8;
            Some(klondike::moves::KlondikeMove::StockPileRecycle { source_index })
        }
        _ => None,
    }
}

// ═══════════════════════════════════════════
// FreeCell
// ═══════════════════════════════════════════

pub struct FreeCellBoard {
    inner: freecell::board::FreeCellBoard,
}

impl FreeCellBoard {}

pub struct FreeCellEngine {
    solver: freecell::solver::FreeCellSolver,
    last_checkpoints_adopted: AtomicU32,
}

impl FreeCellEngine {
    pub fn new() -> Self {
        FreeCellEngine {
            solver: freecell::solver::FreeCellSolver::new(),
            last_checkpoints_adopted: AtomicU32::new(0),
        }
    }

    pub fn deal(&self, deck: Vec<Card>) -> Result<Arc<FreeCellBoard>, SolverError> {
        if deck.len() != 52 {
            return Err(SolverError::InvalidDeck);
        }
        let board = freecell::solver::deal(&deck).ok_or(SolverError::DealFailed)?;
        Ok(Arc::new(FreeCellBoard { inner: board }))
    }

    pub fn solve(
        &self,
        board: &FreeCellBoard,
        timeout_secs: f64,
        allow_partial: bool,
    ) -> Option<Vec<FreeCellMoveDesc>> {
        self.last_checkpoints_adopted.store(0, Ordering::Relaxed);
        let stats = self
            .solver
            .solve_with_stats(&board.inner, timeout_secs, allow_partial);
        self.last_checkpoints_adopted
            .store(stats.checkpoints_adopted, Ordering::Relaxed);
        let moves = stats.moves?;
        Some(moves.iter().map(freecell_move_to_desc).collect())
    }

    pub fn last_checkpoints_adopted(&self) -> u32 {
        self.last_checkpoints_adopted.load(Ordering::Relaxed)
    }

    pub fn apply_move(
        &self,
        board: &FreeCellBoard,
        m: FreeCellMoveDesc,
    ) -> Option<Arc<FreeCellBoard>> {
        let the_move = desc_to_freecell_move(&m, &board.inner)?;
        let new_board = the_move.apply(&board.inner)?;
        Some(Arc::new(FreeCellBoard { inner: new_board }))
    }

    pub fn is_win(&self, board: &FreeCellBoard) -> bool {
        freecell::solver::is_win(&board.inner)
    }

    pub fn progress_token(&self, board: &FreeCellBoard) -> String {
        freecell::solver::progress_token(&board.inner)
    }

    pub fn is_foundation_move(&self, m: FreeCellMoveDesc) -> bool {
        m.move_type == "tableauToFoundation" || m.move_type == "freeCellToFoundation"
    }
}

fn freecell_move_to_desc(m: &freecell::board::FreeCellMove) -> FreeCellMoveDesc {
    match m {
        freecell::board::FreeCellMove::TableauToTableau {
            source,
            destination,
            cards,
        } => FreeCellMoveDesc {
            move_type: "tableauToTableau".into(),
            source: *source as i32,
            destination: *destination as i32,
            card_count: cards.len() as i32,
            card_suit: 0,
            card_value: 0,
        },
        freecell::board::FreeCellMove::TableauToFoundation { source, card } => FreeCellMoveDesc {
            move_type: "tableauToFoundation".into(),
            source: *source as i32,
            destination: -1,
            card_count: 1,
            card_suit: card.suit as u8,
            card_value: card.value,
        },
        freecell::board::FreeCellMove::TableauToFreeCell {
            source,
            card,
            cell_index,
        } => FreeCellMoveDesc {
            move_type: "tableauToFreeCell".into(),
            source: *source as i32,
            destination: *cell_index as i32,
            card_count: 1,
            card_suit: card.suit as u8,
            card_value: card.value,
        },
        freecell::board::FreeCellMove::FreeCellToTableau {
            cell_index,
            destination,
            card,
        } => FreeCellMoveDesc {
            move_type: "freeCellToTableau".into(),
            source: *cell_index as i32,
            destination: *destination as i32,
            card_count: 1,
            card_suit: card.suit as u8,
            card_value: card.value,
        },
        freecell::board::FreeCellMove::FreeCellToFoundation { cell_index, card } => {
            FreeCellMoveDesc {
                move_type: "freeCellToFoundation".into(),
                source: *cell_index as i32,
                destination: -1,
                card_count: 1,
                card_suit: card.suit as u8,
                card_value: card.value,
            }
        }
        freecell::board::FreeCellMove::FoundationToTableau {
            suit,
            destination,
            card,
        } => FreeCellMoveDesc {
            move_type: "foundationToTableau".into(),
            source: *suit as i32,
            destination: *destination as i32,
            card_count: 1,
            card_suit: card.suit as u8,
            card_value: card.value,
        },
        freecell::board::FreeCellMove::Deal { .. } => FreeCellMoveDesc {
            move_type: "deal".into(),
            source: -1,
            destination: -1,
            card_count: 0,
            card_suit: 0,
            card_value: 0,
        },
    }
}

fn desc_to_freecell_move(
    desc: &FreeCellMoveDesc,
    board: &freecell::board::FreeCellBoard,
) -> Option<freecell::board::FreeCellMove> {
    match desc.move_type.as_str() {
        "tableauToTableau" => {
            let src = bounded_index(desc.source, board.tableau.len())?;
            let dest = bounded_index(desc.destination, board.tableau.len())?;
            let count = positive_to_usize(desc.card_count)?;
            let run = board.tableau_run_from_top(src, count)?;
            Some(freecell::board::FreeCellMove::TableauToTableau {
                source: src,
                destination: dest,
                cards: run,
            })
        }
        "tableauToFoundation" => {
            let src = bounded_index(desc.source, board.tableau.len())?;
            let card = board.top_of_tableau(src)?;
            Some(freecell::board::FreeCellMove::TableauToFoundation { source: src, card })
        }
        "tableauToFreeCell" => {
            let src = bounded_index(desc.source, board.tableau.len())?;
            let ci = bounded_index(desc.destination, board.free_cells.len())?;
            let card = board.top_of_tableau(src)?;
            Some(freecell::board::FreeCellMove::TableauToFreeCell {
                source: src,
                card,
                cell_index: ci,
            })
        }
        "freeCellToTableau" => {
            let ci = bounded_index(desc.source, board.free_cells.len())?;
            let dest = bounded_index(desc.destination, board.tableau.len())?;
            let card = board.free_cells[ci]?;
            Some(freecell::board::FreeCellMove::FreeCellToTableau {
                cell_index: ci,
                destination: dest,
                card,
            })
        }
        "freeCellToFoundation" => {
            let ci = bounded_index(desc.source, board.free_cells.len())?;
            let card = board.free_cells[ci]?;
            Some(freecell::board::FreeCellMove::FreeCellToFoundation {
                cell_index: ci,
                card,
            })
        }
        "foundationToTableau" => {
            let suit = Suit::from_u8(desc.card_suit)?;
            let dest = bounded_index(desc.destination, board.tableau.len())?;
            let card = board.top_of_foundation(suit)?;
            Some(freecell::board::FreeCellMove::FoundationToTableau {
                suit,
                destination: dest,
                card,
            })
        }
        _ => None,
    }
}

// ═══════════════════════════════════════════
// Pyramid
// ═══════════════════════════════════════════

pub struct PyramidBoard {
    inner: pyramid::board::PyramidBoard,
}

impl PyramidBoard {}

pub struct PyramidEngine {
    solver: pyramid::solver::PyramidSolver,
    last_checkpoints_adopted: AtomicU32,
}

impl PyramidEngine {
    pub fn new() -> Self {
        PyramidEngine {
            solver: pyramid::solver::PyramidSolver::new(),
            last_checkpoints_adopted: AtomicU32::new(0),
        }
    }

    pub fn deal(&self, deck: Vec<Card>) -> Result<Arc<PyramidBoard>, SolverError> {
        if deck.len() != 52 {
            return Err(SolverError::InvalidDeck);
        }
        let board = pyramid::solver::deal(&deck).ok_or(SolverError::DealFailed)?;
        Ok(Arc::new(PyramidBoard { inner: board }))
    }

    pub fn solve(
        &self,
        board: &PyramidBoard,
        timeout_secs: f64,
        allow_partial: bool,
    ) -> Option<Vec<PyramidMoveDesc>> {
        self.last_checkpoints_adopted.store(0, Ordering::Relaxed);
        let stats = self
            .solver
            .solve_with_stats(&board.inner, timeout_secs, allow_partial);
        self.last_checkpoints_adopted
            .store(stats.checkpoints_adopted, Ordering::Relaxed);
        let moves = stats.moves?;
        Some(moves.iter().map(pyramid_move_to_desc).collect())
    }

    pub fn last_checkpoints_adopted(&self) -> u32 {
        self.last_checkpoints_adopted.load(Ordering::Relaxed)
    }

    pub fn apply_move(
        &self,
        board: &PyramidBoard,
        m: PyramidMoveDesc,
    ) -> Option<Arc<PyramidBoard>> {
        let the_move = desc_to_pyramid_move(&m)?;
        let new_board = the_move.apply(&board.inner)?;
        Some(Arc::new(PyramidBoard { inner: new_board }))
    }

    pub fn is_win(&self, board: &PyramidBoard) -> bool {
        pyramid::solver::PyramidSolver::is_win(&board.inner)
    }

    pub fn progress_token(&self, board: &PyramidBoard) -> String {
        let remaining = board.inner.remaining_pyramid();
        let stock = board.inner.stock.len();
        let waste = board.inner.waste.len();
        let exposed = board.inner.exposed_pyramid_indices().len();
        format!("r:{}|s:{}|w:{}|e:{}", remaining, stock, waste, exposed)
    }

    pub fn find_greedy_moves(&self, board: &PyramidBoard) -> Vec<PyramidMoveDesc> {
        let mut moves = Vec::new();
        moves.extend(
            pyramid::board::PyramidMove::find_king_moves(&board.inner)
                .iter()
                .map(pyramid_move_to_desc),
        );
        moves.extend(
            pyramid::board::PyramidMove::find_waste_pyramid_pairs(&board.inner)
                .iter()
                .map(pyramid_move_to_desc),
        );
        moves.extend(
            pyramid::board::PyramidMove::find_pyramid_pyramid_pairs(&board.inner)
                .iter()
                .map(pyramid_move_to_desc),
        );
        moves
    }

    pub fn is_king_move(&self, m: PyramidMoveDesc) -> bool {
        m.move_type == "kingPyramid" || m.move_type == "kingWaste"
    }

    pub fn is_pair_move(&self, m: PyramidMoveDesc) -> bool {
        m.move_type == "pairWP" || m.move_type == "pairPP"
    }
}

fn pyramid_move_to_desc(m: &pyramid::board::PyramidMove) -> PyramidMoveDesc {
    match m {
        pyramid::board::PyramidMove::StockAdvance => PyramidMoveDesc {
            move_type: "stockAdvance".into(),
            index_a: -1,
            index_b: -1,
        },
        pyramid::board::PyramidMove::StockReset => PyramidMoveDesc {
            move_type: "stockReset".into(),
            index_a: -1,
            index_b: -1,
        },
        pyramid::board::PyramidMove::RemovePairWastePyramid { pyramid_index } => PyramidMoveDesc {
            move_type: "pairWP".into(),
            index_a: *pyramid_index as i32,
            index_b: -1,
        },
        pyramid::board::PyramidMove::RemovePairPyramidPyramid { i, j } => PyramidMoveDesc {
            move_type: "pairPP".into(),
            index_a: *i as i32,
            index_b: *j as i32,
        },
        pyramid::board::PyramidMove::KingToFoundationPyramid { index } => PyramidMoveDesc {
            move_type: "kingPyramid".into(),
            index_a: *index as i32,
            index_b: -1,
        },
        pyramid::board::PyramidMove::KingToFoundationWaste => PyramidMoveDesc {
            move_type: "kingWaste".into(),
            index_a: -1,
            index_b: -1,
        },
        pyramid::board::PyramidMove::Deal { .. } => PyramidMoveDesc {
            move_type: "deal".into(),
            index_a: -1,
            index_b: -1,
        },
    }
}

fn desc_to_pyramid_move(desc: &PyramidMoveDesc) -> Option<pyramid::board::PyramidMove> {
    match desc.move_type.as_str() {
        "stockAdvance" => Some(pyramid::board::PyramidMove::StockAdvance),
        "stockReset" => Some(pyramid::board::PyramidMove::StockReset),
        "pairWP" => {
            let index = bounded_index(desc.index_a, pyramid::board::PyramidBoard::PYRAMID_SIZE)?;
            Some(pyramid::board::PyramidMove::RemovePairWastePyramid {
                pyramid_index: index,
            })
        }
        "pairPP" => {
            let i = bounded_index(desc.index_a, pyramid::board::PyramidBoard::PYRAMID_SIZE)?;
            let j = bounded_index(desc.index_b, pyramid::board::PyramidBoard::PYRAMID_SIZE)?;
            Some(pyramid::board::PyramidMove::RemovePairPyramidPyramid { i, j })
        }
        "kingPyramid" => {
            let index = bounded_index(desc.index_a, pyramid::board::PyramidBoard::PYRAMID_SIZE)?;
            Some(pyramid::board::PyramidMove::KingToFoundationPyramid { index })
        }
        "kingWaste" => Some(pyramid::board::PyramidMove::KingToFoundationWaste),
        _ => None,
    }
}

// ═══════════════════════════════════════════
// TriPeaks
// ═══════════════════════════════════════════

pub struct TriPeaksBoard {
    inner: tripeaks::board::TriPeaksBoard,
}

impl TriPeaksBoard {}

pub struct TriPeaksEngine {
    solver: tripeaks::solver::TriPeaksSolver,
    last_checkpoints_adopted: AtomicU32,
}

impl TriPeaksEngine {
    pub fn new() -> Self {
        TriPeaksEngine {
            solver: tripeaks::solver::TriPeaksSolver::new(),
            last_checkpoints_adopted: AtomicU32::new(0),
        }
    }

    pub fn deal(&self, deck: Vec<Card>) -> Result<Arc<TriPeaksBoard>, SolverError> {
        if deck.len() != 52 {
            return Err(SolverError::InvalidDeck);
        }
        let board = tripeaks::solver::deal(&deck).ok_or(SolverError::DealFailed)?;
        Ok(Arc::new(TriPeaksBoard { inner: board }))
    }

    pub fn solve(
        &self,
        board: &TriPeaksBoard,
        allow_partial: bool,
    ) -> Option<Vec<TriPeaksMoveDesc>> {
        self.last_checkpoints_adopted.store(0, Ordering::Relaxed);
        let stats = self.solver.solve_with_stats(&board.inner, allow_partial);
        self.last_checkpoints_adopted
            .store(stats.checkpoints_adopted, Ordering::Relaxed);
        let moves = stats.moves?;
        Some(moves.iter().map(tripeaks_move_to_desc).collect())
    }

    pub fn last_checkpoints_adopted(&self) -> u32 {
        self.last_checkpoints_adopted.load(Ordering::Relaxed)
    }

    pub fn apply_move(
        &self,
        board: &TriPeaksBoard,
        m: TriPeaksMoveDesc,
    ) -> Option<Arc<TriPeaksBoard>> {
        let the_move = desc_to_tripeaks_move(&m)?;
        let new_board = the_move.apply(&board.inner)?;
        Some(Arc::new(TriPeaksBoard { inner: new_board }))
    }

    pub fn is_win(&self, board: &TriPeaksBoard) -> bool {
        tripeaks::solver::TriPeaksSolver::is_win(&board.inner)
    }

    pub fn is_tableau_move(&self, m: TriPeaksMoveDesc) -> bool {
        m.move_type == "tableauToWaste"
    }

    pub fn progress_token(&self, board: &TriPeaksBoard) -> String {
        tripeaks::solver::progress_token(&board.inner)
    }

    pub fn remaining_tableau(&self, board: &TriPeaksBoard) -> i32 {
        board.inner.remaining_tableau() as i32
    }
}

fn tripeaks_move_to_desc(m: &tripeaks::board::TriPeaksMove) -> TriPeaksMoveDesc {
    match m {
        tripeaks::board::TriPeaksMove::TableauToWaste { tableau_index } => TriPeaksMoveDesc {
            move_type: "tableauToWaste".into(),
            tableau_index: *tableau_index as i32,
        },
        tripeaks::board::TriPeaksMove::DrawFromStock => TriPeaksMoveDesc {
            move_type: "drawFromStock".into(),
            tableau_index: -1,
        },
        tripeaks::board::TriPeaksMove::Deal { .. } => TriPeaksMoveDesc {
            move_type: "deal".into(),
            tableau_index: -1,
        },
    }
}

fn desc_to_tripeaks_move(desc: &TriPeaksMoveDesc) -> Option<tripeaks::board::TriPeaksMove> {
    match desc.move_type.as_str() {
        "tableauToWaste" => {
            let tableau_index = bounded_index(
                desc.tableau_index,
                tripeaks::board::TriPeaksBoard::TABLEAU_SIZE,
            )?;
            Some(tripeaks::board::TriPeaksMove::TableauToWaste { tableau_index })
        }
        "drawFromStock" => Some(tripeaks::board::TriPeaksMove::DrawFromStock),
        _ => None,
    }
}

// ─── UniFFI scaffolding ───
uniffi::include_scaffolding!("solver_core");
