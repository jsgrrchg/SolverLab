use std::collections::HashMap;
use std::time::Instant;

use super::board::{PyramidBoard, PyramidMove};
use super::weights;
use crate::common::card::Card;

/// Características del tablero para evaluación heurística.
struct PyBoardFeatures {
    remaining_pyramid: i64,
    exposed_count: i64,
    blocked_count: i64,
    deep_blocked_count: i64,
    exposed_kings: i64,
    pair_options: i64,
    waste_pair_options: i64,
    stock_count: i64,
    waste_count: i64,
    dead_end_penalty: i64,
    unreachable_exposed: i64,
}

fn board_features(board: &PyramidBoard) -> PyBoardFeatures {
    // Extrae métricas del estado para priorizar nodos en la búsqueda A*.
    let remaining = board.remaining_pyramid() as i64;
    let exposed = board.exposed_pyramid_indices();
    let exposed_count = exposed.len() as i64;
    let blocked_count = (remaining - exposed_count).max(0);

    let deep_blocked: i64 = board
        .pyramid
        .iter()
        .enumerate()
        .filter(|(idx, card)| {
            card.is_some() && PyramidBoard::row(*idx) <= 4 && !board.is_exposed(*idx)
        })
        .count() as i64;

    let exposed_kings: i64 = exposed
        .iter()
        .filter(|&&idx| board.pyramid[idx].map(|c| c.value == 13).unwrap_or(false))
        .count() as i64
        + if board.waste_top().map(|c| c.value == 13).unwrap_or(false) {
            1
        } else {
            0
        };

    let pair_options = (PyramidMove::find_pyramid_pyramid_pairs(board).len()
        + PyramidMove::find_king_moves(board).len()) as i64;
    let waste_pair_options = PyramidMove::find_waste_pyramid_pairs(board).len() as i64;

    let has_stock_flow = board.can_advance_stock() || board.can_reset_stock();
    let dead_end_penalty = if !has_stock_flow && pair_options == 0 && waste_pair_options == 0 {
        weights::DEAD_END_PENALTY
    } else {
        0
    };

    // Cartas expuestas cuyo complemento no está accesible (ni en otras expuestas, ni waste top, ni stock).
    let unreachable_exposed: i64 = exposed
        .iter()
        .filter(|&&idx| {
            let card = match board.pyramid[idx] {
                Some(c) => c,
                None => return false,
            };
            if card.value == 13 {
                return false; // reyes se remueven solos
            }
            let complement = 13 - card.value;
            // ¿Existe en otras cartas expuestas de la pirámide?
            for &other in &exposed {
                if other != idx {
                    if let Some(c) = board.pyramid[other] {
                        if c.value == complement {
                            return false;
                        }
                    }
                }
            }
            // ¿Existe en el tope del waste?
            if let Some(w) = board.waste_top() {
                if w.value == complement {
                    return false;
                }
            }
            // ¿Existe en el stock?
            for s in &board.stock {
                if s.value == complement {
                    return false;
                }
            }
            true // complemento enterrado en pirámide no expuesta
        })
        .count() as i64;

    PyBoardFeatures {
        remaining_pyramid: remaining,
        exposed_count,
        blocked_count,
        deep_blocked_count: deep_blocked,
        exposed_kings,
        pair_options,
        waste_pair_options,
        stock_count: board.stock.len() as i64,
        waste_count: board.waste.len() as i64,
        dead_end_penalty,
        unreachable_exposed,
    }
}

fn heuristic_cost(f: &PyBoardFeatures) -> i64 {
    // Costo estimado restante (h): menor es mejor.
    let scarce_pair = (weights::HEURISTIC_SCARCE_PAIR_TARGET - f.pair_options).max(0)
        * weights::HEURISTIC_SCARCE_PAIR_WEIGHT;
    let king_pressure = (weights::HEURISTIC_KING_PRESSURE_TARGET - f.exposed_kings).max(0)
        * weights::HEURISTIC_KING_PRESSURE_WEIGHT;
    f.remaining_pyramid * weights::HEURISTIC_REMAINING_PYRAMID_WEIGHT
        + f.blocked_count * weights::HEURISTIC_BLOCKED_WEIGHT
        + f.deep_blocked_count * weights::HEURISTIC_DEEP_BLOCKED_WEIGHT
        + f.stock_count * weights::HEURISTIC_STOCK_WEIGHT
        + f.waste_count * weights::HEURISTIC_WASTE_WEIGHT
        + scarce_pair
        + king_pressure
        + f.dead_end_penalty
        + f.unreachable_exposed * weights::HEURISTIC_UNREACHABLE_WEIGHT
        - f.waste_pair_options
            .min(weights::HEURISTIC_WASTE_PAIR_BONUS_CAP)
            * weights::HEURISTIC_WASTE_PAIR_BONUS
}

fn progress_score(f: &PyBoardFeatures) -> i64 {
    // Puntaje de progreso para conservar el mejor estado parcial.
    let removed =
        weights::PROGRESS_DECK_SIZE - (f.remaining_pyramid + f.stock_count + f.waste_count);
    removed * weights::PROGRESS_REMOVED_WEIGHT
        + f.exposed_count * weights::PROGRESS_EXPOSED_WEIGHT
        + f.exposed_kings * weights::PROGRESS_EXPOSED_KING_WEIGHT
        + f.pair_options * weights::PROGRESS_PAIR_OPTIONS_WEIGHT
        + f.waste_pair_options * weights::PROGRESS_WASTE_PAIR_OPTIONS_WEIGHT
        - f.blocked_count * weights::PROGRESS_BLOCKED_PENALTY
        - f.deep_blocked_count * weights::PROGRESS_DEEP_BLOCKED_PENALTY
        - f.dead_end_penalty * weights::PROGRESS_DEAD_END_MULTIPLIER
}

fn unlock_score(f: &PyBoardFeatures) -> i64 {
    // Desempate: favorece estados con más opciones de desbloqueo.
    f.exposed_count * weights::UNLOCK_EXPOSED_WEIGHT
        + f.exposed_kings * weights::UNLOCK_EXPOSED_KING_WEIGHT
        + f.pair_options * weights::UNLOCK_PAIR_OPTIONS_WEIGHT
        + f.waste_pair_options * weights::UNLOCK_WASTE_PAIR_OPTIONS_WEIGHT
        - f.blocked_count * weights::UNLOCK_BLOCKED_PENALTY
        - f.deep_blocked_count * weights::UNLOCK_DEEP_BLOCKED_PENALTY
}

/// Solver A* puro de Pirámide con timeout y retorno parcial.
pub struct PyramidSolver {
    _private: (),
}

pub struct PyramidSolveStats {
    pub moves: Option<Vec<PyramidMove>>,
    pub checkpoints_adopted: u32,
}

impl PyramidSolver {
    pub fn new() -> PyramidSolver {
        PyramidSolver { _private: () }
    }

    pub fn is_win(board: &PyramidBoard) -> bool {
        // Victoria cuando toda la pirámide está vacía.
        board.pyramid.iter().all(|c| c.is_none())
    }

    /// Ejecuta A* con límite opcional de tiempo y retorno parcial opcional.
    pub fn solve(
        &self,
        board: &PyramidBoard,
        timeout_secs: f64,
        allow_partial: bool,
    ) -> Option<Vec<PyramidMove>> {
        self.solve_with_stats(board, timeout_secs, allow_partial)
            .moves
    }

    pub fn solve_with_stats(
        &self,
        board: &PyramidBoard,
        timeout_secs: f64,
        allow_partial: bool,
    ) -> PyramidSolveStats {
        if Self::is_win(board) {
            return PyramidSolveStats {
                moves: Some(vec![]),
                checkpoints_adopted: 0,
            };
        }

        let start = Instant::now();
        let timeout = if timeout_secs > 0.0 {
            Some(std::time::Duration::from_secs_f64(timeout_secs))
        } else {
            None
        };

        // Arena de nodos para reconstrucción de camino por índices.
        let mut nodes: Vec<PySearchNode> = vec![PySearchNode {
            parent_index: None,
            the_move: None,
            path_cost: 0,
        }];

        let mut frontier = MinHeap::new();
        let root_features = board_features(board);
        let root_h = heuristic_cost(&root_features);
        frontier.push(PyFrontierItem {
            node_index: 0,
            board: board.clone(),
            g_score: 0,
            h_score: root_h,
            unlock_score: unlock_score(&root_features),
            dead_end_penalty: root_features.dead_end_penalty,
        });

        let mut best_cost: HashMap<u64, usize> = HashMap::new();
        best_cost.insert(board.signature, 0);

        let mut best_progress_idx: usize = 0;
        let mut best_progress = progress_score(&root_features);
        let mut expanded: usize = 0;

        // Búsqueda A* pura: expande hasta solución, timeout, límite de nodos o frontera agotada.
        while let Some(current) = frontier.pop() {
            if let Some(t) = timeout {
                if start.elapsed() > t {
                    break;
                }
            }
            if expanded >= weights::MAX_NODES {
                break;
            }
            expanded += 1;

            let node_cost = nodes[current.node_index].path_cost;
            let current_board = current.board;

            if let Some(&known) = best_cost.get(&current_board.signature) {
                if known != node_cost {
                    continue;
                }
            }

            if Self::is_win(&current_board) {
                return PyramidSolveStats {
                    moves: Some(reconstruct_moves(current.node_index, &nodes)),
                    checkpoints_adopted: 0,
                };
            }

            let node_prev_move = nodes[current.node_index].the_move.clone();
            let moves = PyramidMove::find_all_moves(&current_board);

            for the_move in moves {
                // Filtro: tras un StockReset, el siguiente movimiento DEBE ser StockAdvance.
                if let Some(ref prev) = node_prev_move {
                    if prev.is_stock_reset() && !the_move.is_stock_advance() {
                        continue;
                    }
                }

                let next_board = match the_move.apply(&current_board) {
                    Some(b) => b,
                    None => continue,
                };

                let next_g = node_cost + 1;
                if let Some(&known) = best_cost.get(&next_board.signature) {
                    if known <= next_g {
                        continue;
                    }
                }

                let features = board_features(&next_board);
                let score = progress_score(&features);
                let child_idx = nodes.len();

                if score > best_progress {
                    best_progress = score;
                    best_progress_idx = child_idx;
                }

                nodes.push(PySearchNode {
                    parent_index: Some(current.node_index),
                    the_move: Some(the_move),
                    path_cost: next_g,
                });
                best_cost.insert(next_board.signature, next_g);

                let h = heuristic_cost(&features);
                frontier.push(PyFrontierItem {
                    node_index: child_idx,
                    board: next_board,
                    g_score: next_g,
                    h_score: h,
                    unlock_score: unlock_score(&features),
                    dead_end_penalty: features.dead_end_penalty,
                });
            }
        }

        // Sin solución: retorna mejor progreso parcial si se solicitó.
        if allow_partial && best_progress_idx != 0 {
            return PyramidSolveStats {
                moves: Some(reconstruct_moves(best_progress_idx, &nodes)),
                checkpoints_adopted: 0,
            };
        }

        PyramidSolveStats {
            moves: None,
            checkpoints_adopted: 0,
        }
    }
}

pub fn deal(deck: &[Card]) -> Option<PyramidBoard> {
    // Inicializa una partida desde un mazo completo de 52 cartas.
    let m = PyramidMove::Deal {
        deck: deck.to_vec(),
    };
    m.apply(&PyramidBoard::new(
        vec![None; PyramidBoard::PYRAMID_SIZE],
        vec![],
        vec![],
        vec![],
    ))
}

fn reconstruct_moves(from: usize, nodes: &[PySearchNode]) -> Vec<PyramidMove> {
    // Reconstruye el camino desde un nodo hasta la raíz.
    let mut moves = Vec::new();
    let mut cursor: Option<usize> = Some(from);
    while let Some(idx) = cursor {
        if let Some(ref m) = nodes[idx].the_move {
            moves.push(m.clone());
        }
        cursor = nodes[idx].parent_index;
    }
    moves.reverse();
    moves
}

struct PySearchNode {
    // Índice del padre en `nodes`.
    parent_index: Option<usize>,
    // Movimiento usado para llegar a este nodo desde su padre.
    the_move: Option<PyramidMove>,
    // Costo acumulado desde la raíz (g).
    path_cost: usize,
}

struct PyFrontierItem {
    // Referencia al nodo real almacenado en `nodes`.
    node_index: usize,
    // Copia del tablero para expansión rápida.
    board: PyramidBoard,
    // Costo acumulado g.
    g_score: usize,
    // Heurística h.
    h_score: i64,
    // Puntaje de desbloqueo para desempate.
    unlock_score: i64,
    // Penalización de callejón sin salida para desempate.
    dead_end_penalty: i64,
}

impl PyFrontierItem {
    // f = g + h
    fn f_score(&self) -> i64 {
        self.g_score as i64 + self.h_score
    }
}

// MinHeap con comparador de 5 campos, igual que Swift:
// fScore → hScore → deadEndPenalty → unlockScore(desc) → nodeIndex
struct MinHeap {
    storage: Vec<PyFrontierItem>,
}

impl MinHeap {
    /// Crea un heap mínimo vacío.
    fn new() -> MinHeap {
        MinHeap {
            storage: Vec::new(),
        }
    }

    /// Inserta un elemento y restaura propiedad de heap hacia arriba.
    fn push(&mut self, item: PyFrontierItem) {
        self.storage.push(item);
        self.sift_up(self.storage.len() - 1);
    }

    /// Extrae el mejor elemento (mínimo según comparador).
    fn pop(&mut self) -> Option<PyFrontierItem> {
        if self.storage.is_empty() {
            return None;
        }
        if self.storage.len() == 1 {
            return Some(self.storage.pop().unwrap());
        }
        let last = self.storage.len() - 1;
        self.storage.swap(0, last);
        let min = self.storage.pop().unwrap(); // Extrae el mínimo actual.
        self.sift_down(0);
        Some(min)
    }

    fn is_higher(a: &PyFrontierItem, b: &PyFrontierItem) -> bool {
        // "higher" significa mayor prioridad para salir antes del heap.
        let fa = a.f_score();
        let fb = b.f_score();
        if fa != fb {
            return fa < fb;
        }
        if a.h_score != b.h_score {
            return a.h_score < b.h_score;
        }
        if a.dead_end_penalty != b.dead_end_penalty {
            return a.dead_end_penalty < b.dead_end_penalty;
        }
        if a.unlock_score != b.unlock_score {
            return a.unlock_score > b.unlock_score;
        } // descendente
        a.node_index < b.node_index
    }

    fn sift_up(&mut self, mut child: usize) {
        // Propaga un nodo hacia la raíz mientras tenga mayor prioridad.
        while child > 0 {
            let parent = (child - 1) / 2;
            if Self::is_higher(&self.storage[child], &self.storage[parent]) {
                self.storage.swap(child, parent);
                child = parent;
            } else {
                return;
            }
        }
    }

    fn sift_down(&mut self, mut parent: usize) {
        // Empuja un nodo hacia abajo hasta restaurar el orden del heap.
        loop {
            let left = 2 * parent + 1;
            let right = left + 1;
            let mut candidate = parent;
            if left < self.storage.len()
                && Self::is_higher(&self.storage[left], &self.storage[candidate])
            {
                candidate = left;
            }
            if right < self.storage.len()
                && Self::is_higher(&self.storage[right], &self.storage[candidate])
            {
                candidate = right;
            }
            if candidate == parent {
                return;
            }
            self.storage.swap(parent, candidate);
            parent = candidate;
        }
    }
}
