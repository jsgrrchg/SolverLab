use std::collections::HashMap;
use std::time::Instant;

use super::board::{TriPeaksBoard, TriPeaksMove};
use super::weights;

/// TriPeaks solver transition.
pub struct TriPeaksTransition {
    /// Source board.
    pub from_board: TriPeaksBoard,
    /// Destination board after applying the move.
    pub to_board: TriPeaksBoard,
    /// Applied move.
    pub the_move: TriPeaksMove,
    /// Previous move in the path, if any.
    pub previous_move: Option<TriPeaksMove>,
    /// Destination node depth.
    pub depth: usize,
}

/// Pruning rules for TriPeaks.
#[derive(Debug, Clone)]
pub enum TriPeaksRule {
    DepthLimit { max_depth: usize },
    NoImmediateUndo,
    NoopTransition,
}

impl TriPeaksRule {
    /// Builds the default rule set.
    pub fn default_rules(max_depth: usize) -> Vec<TriPeaksRule> {
        vec![
            TriPeaksRule::DepthLimit { max_depth },
            TriPeaksRule::NoImmediateUndo,
            TriPeaksRule::NoopTransition,
        ]
    }

    /// Indicates whether a transition should be discarded.
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

/// Pure TriPeaks A* solver with timeout and partial return.
pub struct TriPeaksSolver {
    pub rules: Vec<TriPeaksRule>,
}

/// Extended solve result.
pub struct TriPeaksSolveStats {
    /// Found move sequence (complete or partial).
    pub moves: Option<Vec<TriPeaksMove>>,
    /// Number of adopted checkpoints.
    pub checkpoints_adopted: u32,
}

impl TriPeaksSolver {
    /// Creates the solver with max_depth hardcoded from weights.
    pub fn new() -> TriPeaksSolver {
        TriPeaksSolver {
            rules: TriPeaksRule::default_rules(weights::MAX_DEPTH),
        }
    }

    /// Victory occurs when no tableau cards remain.
    pub fn is_win(board: &TriPeaksBoard) -> bool {
        board.tableau.iter().all(|c| c.is_none())
    }

    /// Generates and orders valid successors from the current state.
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

        // Order by local priority: tableau moves first.
        transitions.sort_by(|a, b| {
            let pa = local_priority(&a);
            let pb = local_priority(&b);
            pb.cmp(&pa) // descending
        });

        transitions
    }

    /// Simple solve API, without statistics.
    pub fn solve(&self, board: &TriPeaksBoard, allow_partial: bool) -> Option<Vec<TriPeaksMove>> {
        self.solve_with_stats(board, allow_partial).moves
    }

    /// Runs A* with node limit, timeout, and optional partial return.
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

        // Node arena for path reconstruction.
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

        // Pure A* search: expands until solution, timeout, node limit, or exhausted frontier.
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

        // No solution: return best partial progress if requested.
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

/// Deals a deck into the initial TriPeaks state.
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

/// Compact token for detecting/measuring progress between states.
pub fn progress_token(board: &TriPeaksBoard) -> String {
    let remaining = board.remaining_tableau();
    let stock = board.stock.len();
    let waste = board.waste.len();
    format!("r:{}|s:{}|w:{}", remaining, stock, waste)
}

/// Local priority used to order transitions.
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

/// A* heuristic: remaining tableau cards.
fn heuristic_cost(board: &TriPeaksBoard) -> i64 {
    board.remaining_tableau() as i64 * weights::HEURISTIC_REMAINING_TABLEAU_WEIGHT
}

/// Progress score: removed tableau cards.
fn progress_score(board: &TriPeaksBoard) -> i64 {
    (TriPeaksBoard::TABLEAU_SIZE as i64 - board.remaining_tableau() as i64)
        * weights::PROGRESS_REMOVED_TABLEAU_WEIGHT
}

/// Node stored in the search arena.
struct TpSearchNode {
    /// Parent index in `nodes`.
    parent_index: Option<usize>,
    /// Move used to reach this node.
    the_move: Option<TriPeaksMove>,
    /// Node depth.
    depth: usize,
    /// Accumulated g cost.
    path_cost: usize,
}

/// Frontier entry for the priority heap.
struct TpFrontierItem {
    /// Associated node index in `nodes`.
    node_index: usize,
    /// Board copy for expansion.
    board: TriPeaksBoard,
    /// Accumulated g cost.
    g_score: usize,
    /// Heuristic h.
    h_score: i64,
}

impl TpFrontierItem {
    /// Total score f = g + h.
    fn f_score(&self) -> i64 {
        self.g_score as i64 + self.h_score
    }
}

/// Reconstructs the move path from a node back to the root.
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

/// Min-heap for prioritizing the A* frontier.
struct MinHeap {
    /// Binary heap storage.
    storage: Vec<TpFrontierItem>,
}

impl MinHeap {
    /// Creates an empty heap.
    fn new() -> MinHeap {
        MinHeap {
            storage: Vec::new(),
        }
    }

    /// Inserts an item and rebalances upward.
    fn push(&mut self, item: TpFrontierItem) {
        self.storage.push(item);
        self.sift_up(self.storage.len() - 1);
    }

    /// Extracts the highest-priority item.
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

    /// Priority comparator (lower f/h first, then stable order).
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

    /// Rebalances upward from `child`.
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

    /// Rebalances downward from `parent`.
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
