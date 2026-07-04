use std::collections::HashMap;
use std::time::Instant;

use super::board::{TriPeaksBoard, TriPeaksMove};
use super::weights;

/// Transición del solver de TriPeaks.
pub struct TriPeaksTransition {
    /// Tablero origen.
    pub from_board: TriPeaksBoard,
    /// Tablero destino tras aplicar el movimiento.
    pub to_board: TriPeaksBoard,
    /// Movimiento aplicado.
    pub the_move: TriPeaksMove,
    /// Movimiento previo del camino, si existe.
    pub previous_move: Option<TriPeaksMove>,
    /// Profundidad del nodo de destino.
    pub depth: usize,
}

/// Reglas de poda para TriPeaks.
#[derive(Debug, Clone)]
pub enum TriPeaksRule {
    DepthLimit { max_depth: usize },
    NoImmediateUndo,
    NoopTransition,
}

impl TriPeaksRule {
    /// Construye el conjunto de reglas por defecto.
    pub fn default_rules(max_depth: usize) -> Vec<TriPeaksRule> {
        vec![
            TriPeaksRule::DepthLimit { max_depth },
            TriPeaksRule::NoImmediateUndo,
            TriPeaksRule::NoopTransition,
        ]
    }

    /// Indica si una transición debe descartarse.
    pub fn should_prune(&self, transition: &TriPeaksTransition) -> bool {
        match self {
            TriPeaksRule::DepthLimit { max_depth } => transition.depth > *max_depth,

            TriPeaksRule::NoImmediateUndo => {
                let prev = match &transition.previous_move {
                    Some(p) => p,
                    None => return false,
                };
                match (prev, &transition.the_move) {
                    (TriPeaksMove::DrawFromStock, TriPeaksMove::TableauToWaste { .. }) => false,
                    (
                        TriPeaksMove::TableauToWaste {
                            tableau_index: idx1,
                        },
                        TriPeaksMove::TableauToWaste {
                            tableau_index: idx2,
                        },
                    ) => {
                        *idx1 == *idx2
                            && transition.from_board.tableau[*idx1].is_none()
                            && transition.to_board.tableau[*idx2].is_some()
                    }
                    _ => false,
                }
            }

            TriPeaksRule::NoopTransition => transition.from_board == transition.to_board,
        }
    }
}

/// Solver A* puro de TriPeaks con timeout y retorno parcial.
pub struct TriPeaksSolver {
    pub rules: Vec<TriPeaksRule>,
}

/// Resultado extendido de resolución.
pub struct TriPeaksSolveStats {
    /// Secuencia de movimientos encontrada (completa o parcial).
    pub moves: Option<Vec<TriPeaksMove>>,
    /// Cantidad de checkpoints adoptados.
    pub checkpoints_adopted: u32,
}

impl TriPeaksSolver {
    /// Crea el solver con max_depth hardcodeado desde weights.
    pub fn new() -> TriPeaksSolver {
        TriPeaksSolver {
            rules: TriPeaksRule::default_rules(weights::MAX_DEPTH),
        }
    }

    /// Hay victoria cuando no quedan cartas en tableau.
    pub fn is_win(board: &TriPeaksBoard) -> bool {
        board.tableau.iter().all(|c| c.is_none())
    }

    /// Genera y ordena sucesores válidos desde el estado actual.
    fn successors(
        &self,
        board: &TriPeaksBoard,
        previous_move: Option<&TriPeaksMove>,
        depth: usize,
    ) -> Vec<TriPeaksTransition> {
        let mut moves = TriPeaksMove::find_tableau_to_waste_moves(board);
        moves.extend(TriPeaksMove::find_draw_from_stock_moves(board));

        let mut transitions = Vec::with_capacity(moves.len());
        for the_move in moves {
            let next_board = match the_move.apply(board) {
                Some(b) => b,
                None => continue,
            };
            let transition = TriPeaksTransition {
                from_board: board.clone(),
                to_board: next_board,
                the_move,
                previous_move: previous_move.cloned(),
                depth: depth + 1,
            };
            let mut pruned = false;
            for rule in &self.rules {
                if rule.should_prune(&transition) {
                    pruned = true;
                    break;
                }
            }
            if !pruned {
                transitions.push(transition);
            }
        }

        // Ordena por prioridad local: primero movimientos de tableau.
        transitions.sort_by(|a, b| {
            let pa = local_priority(&a);
            let pb = local_priority(&b);
            pb.cmp(&pa) // descendente
        });

        transitions
    }

    /// API simple de resolución, sin estadísticas.
    pub fn solve(&self, board: &TriPeaksBoard, allow_partial: bool) -> Option<Vec<TriPeaksMove>> {
        self.solve_with_stats(board, allow_partial).moves
    }

    /// Ejecuta A* con límite de nodos, timeout y retorno parcial opcional.
    pub fn solve_with_stats(
        &self,
        board: &TriPeaksBoard,
        allow_partial: bool,
    ) -> TriPeaksSolveStats {
        if Self::is_win(board) {
            return TriPeaksSolveStats {
                moves: Some(vec![]),
                checkpoints_adopted: 0,
            };
        }

        let start = Instant::now();
        let timeout = Some(std::time::Duration::from_secs_f64(weights::TIMEOUT_SECS));

        // Arena de nodos para reconstrucción de caminos.
        let mut nodes: Vec<TpSearchNode> = vec![TpSearchNode {
            parent_index: None,
            the_move: None,
            depth: 0,
            path_cost: 0,
        }];

        let mut frontier = MinHeap::new();
        let root_h = heuristic_cost(board);
        frontier.push(TpFrontierItem {
            node_index: 0,
            board: board.clone(),
            g_score: 0,
            h_score: root_h,
        });

        let mut best_cost: HashMap<u64, usize> = HashMap::new();
        best_cost.insert(board.signature, 0);

        let mut best_progress_idx: usize = 0;
        let mut best_progress = progress_score(board);
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

            let node = &nodes[current.node_index];
            let node_cost = node.path_cost;
            let node_depth = node.depth;
            let node_move = node.the_move.clone();
            let current_board = current.board;

            if let Some(&known) = best_cost.get(&current_board.signature) {
                if known != node_cost {
                    continue;
                }
            }

            if Self::is_win(&current_board) {
                return TriPeaksSolveStats {
                    moves: Some(reconstruct_moves(current.node_index, &nodes)),
                    checkpoints_adopted: 0,
                };
            }

            let transitions = self.successors(&current_board, node_move.as_ref(), node_depth);

            for transition in transitions {
                let next_board = transition.to_board;
                let next_g = node_cost + 1;

                if let Some(&known) = best_cost.get(&next_board.signature) {
                    if known <= next_g {
                        continue;
                    }
                }

                let child_idx = nodes.len();
                nodes.push(TpSearchNode {
                    parent_index: Some(current.node_index),
                    the_move: Some(transition.the_move),
                    depth: transition.depth,
                    path_cost: next_g,
                });
                best_cost.insert(next_board.signature, next_g);

                let score = progress_score(&next_board);
                if score > best_progress {
                    best_progress = score;
                    best_progress_idx = child_idx;
                }

                let h = heuristic_cost(&next_board);
                frontier.push(TpFrontierItem {
                    node_index: child_idx,
                    board: next_board,
                    g_score: next_g,
                    h_score: h,
                });
            }
        }

        // Sin solución: retorna mejor progreso parcial si se solicitó.
        if allow_partial && best_progress_idx != 0 {
            return TriPeaksSolveStats {
                moves: Some(reconstruct_moves(best_progress_idx, &nodes)),
                checkpoints_adopted: 0,
            };
        }

        TriPeaksSolveStats {
            moves: None,
            checkpoints_adopted: 0,
        }
    }
}

/// Reparte un mazo en el estado inicial de TriPeaks.
pub fn deal(deck: &[crate::common::card::Card]) -> Option<TriPeaksBoard> {
    let deal_move = TriPeaksMove::Deal {
        deck: deck.to_vec(),
    };
    deal_move.apply(&TriPeaksBoard::new(
        vec![None; TriPeaksBoard::TABLEAU_SIZE],
        vec![],
        vec![],
    ))
}

/// Token compacto para detectar/medir progreso entre estados.
pub fn progress_token(board: &TriPeaksBoard) -> String {
    let remaining = board.remaining_tableau();
    let stock = board.stock.len();
    let waste = board.waste.len();
    format!("r:{}|s:{}|w:{}", remaining, stock, waste)
}

/// Prioridad local usada para ordenar transiciones.
fn local_priority(transition: &TriPeaksTransition) -> i64 {
    match &transition.the_move {
        TriPeaksMove::TableauToWaste { .. } => {
            let before = transition.from_board.remaining_tableau() as i64;
            let after = transition.to_board.remaining_tableau() as i64;
            (before - after) * weights::LOCAL_PRIORITY_TABLEAU_REMOVAL_WEIGHT
        }
        TriPeaksMove::DrawFromStock => 0,
        TriPeaksMove::Deal { .. } => 0,
    }
}

/// Heurística A*: cartas restantes en tableau.
fn heuristic_cost(board: &TriPeaksBoard) -> i64 {
    board.remaining_tableau() as i64 * weights::HEURISTIC_REMAINING_TABLEAU_WEIGHT
}

/// Puntaje de progreso: cartas removidas del tableau.
fn progress_score(board: &TriPeaksBoard) -> i64 {
    (TriPeaksBoard::TABLEAU_SIZE as i64 - board.remaining_tableau() as i64)
        * weights::PROGRESS_REMOVED_TABLEAU_WEIGHT
}

/// Nodo almacenado en la arena de búsqueda.
struct TpSearchNode {
    /// Índice del padre en `nodes`.
    parent_index: Option<usize>,
    /// Movimiento usado para llegar a este nodo.
    the_move: Option<TriPeaksMove>,
    /// Profundidad del nodo.
    depth: usize,
    /// Costo acumulado g.
    path_cost: usize,
}

/// Entrada de frontera para el heap de prioridad.
struct TpFrontierItem {
    /// Índice del nodo asociado en `nodes`.
    node_index: usize,
    /// Copia del tablero para expansión.
    board: TriPeaksBoard,
    /// Costo acumulado g.
    g_score: usize,
    /// Heurística h.
    h_score: i64,
}

impl TpFrontierItem {
    /// Puntaje total f = g + h.
    fn f_score(&self) -> i64 {
        self.g_score as i64 + self.h_score
    }
}

/// Reconstruye el camino de movimientos desde un nodo hasta la raíz.
fn reconstruct_moves(from: usize, nodes: &[TpSearchNode]) -> Vec<TriPeaksMove> {
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

// -- MinHeap --

/// Heap mínimo para priorizar la frontera A*.
struct MinHeap {
    /// Almacenamiento del heap binario.
    storage: Vec<TpFrontierItem>,
}

impl MinHeap {
    /// Crea un heap vacío.
    fn new() -> MinHeap {
        MinHeap {
            storage: Vec::new(),
        }
    }

    /// Inserta un elemento y reequilibra hacia arriba.
    fn push(&mut self, item: TpFrontierItem) {
        self.storage.push(item);
        self.sift_up(self.storage.len() - 1);
    }

    /// Extrae el elemento de mayor prioridad.
    fn pop(&mut self) -> Option<TpFrontierItem> {
        if self.storage.is_empty() {
            return None;
        }
        if self.storage.len() == 1 {
            return Some(self.storage.pop().unwrap());
        }
        let last = self.storage.len() - 1;
        self.storage.swap(0, last);
        let min = self.storage.pop().unwrap();
        self.sift_down(0);
        Some(min)
    }

    /// Comparador de prioridad (menor f/h primero, luego orden estable).
    fn is_higher(a: &TpFrontierItem, b: &TpFrontierItem) -> bool {
        let fa = a.f_score();
        let fb = b.f_score();
        if fa != fb {
            return fa < fb;
        }
        if a.h_score != b.h_score {
            return a.h_score < b.h_score;
        }
        a.node_index < b.node_index
    }

    /// Reequilibra subiendo desde `child`.
    fn sift_up(&mut self, mut child: usize) {
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

    /// Reequilibra bajando desde `parent`.
    fn sift_down(&mut self, mut parent: usize) {
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
