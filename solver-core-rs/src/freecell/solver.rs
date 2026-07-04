use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::board::{FreeCellBoard, FreeCellMove};
use super::weights;
use crate::common::card::{Card, Suit};

/// Cached board features for heuristic and progress evaluation.
#[derive(Debug, Clone)]
struct BoardFeatures {
    foundation_count: usize,
    immediate_foundation_moves: usize,
    blocked_low_cards: i64,
    foundation_imbalance: i64,
    free_cells_used: usize,
    empty_tableau: usize,
    longest_run: usize,
}
// Extracts relevant board features for heuristic evaluation, progress, and local priority.
fn compute_features(board: &FreeCellBoard) -> BoardFeatures {
    let foundation_count = board.total_foundation_count();
    let immediate = immediate_foundation_count(board);
    let blocked = blocked_low_penalty(board);
    let imbalance = foundation_imbalance(board);
    let free_used = board.free_cells.iter().filter(|c| c.is_some()).count();
    let empty_tab = board.empty_tableau_columns();
    let longest = (0..8)
        .map(|i| board.max_tableau_run_length(i))
        .max()
        .unwrap_or(0);

    BoardFeatures {
        foundation_count,
        immediate_foundation_moves: immediate,
        blocked_low_cards: blocked,
        foundation_imbalance: imbalance,
        free_cells_used: free_used,
        empty_tableau: empty_tab,
        longest_run: longest,
    }
}
// Counts immediate foundation moves available in the current board state.
fn immediate_foundation_count(board: &FreeCellBoard) -> usize {
    let mut count = 0;
    for i in 0..8 {
        if let Some(card) = board.top_of_tableau(i) {
            if board.can_add_to_foundation(card) {
                count += 1;
            }
        }
    }
    for i in 0..4 {
        if let Some(card) = board.free_cells[i] {
            if board.can_add_to_foundation(card) {
                count += 1;
            }
        }
    }
    count
}
// Computes a penalty for low cards (A,2,3) blocked under other cards in the tableau.
fn blocked_low_penalty(board: &FreeCellBoard) -> i64 {
    let mut penalty: i64 = 0;
    for column in &board.tableau {
        if column.is_empty() {
            continue;
        }
        for (index, card) in column.iter().enumerate() {
            if card.value <= 3 {
                let cards_above = column.len() - index - 1;
                if cards_above > 0 {
                    penalty += cards_above as i64;
                }
            }
        }
    }
    penalty
}
// Computes a foundation imbalance index across the four suits (most advanced minus least advanced).
fn foundation_imbalance(board: &FreeCellBoard) -> i64 {
    let min_h = (0..4).map(|i| board.foundation[i].len()).min().unwrap_or(0);
    let max_h = (0..4).map(|i| board.foundation[i].len()).max().unwrap_or(0);
    (max_h - min_h) as i64
}
// Computes the longest run already formed in the tableau (descending same-suit sequence).
fn longest_run_length(board: &FreeCellBoard) -> usize {
    (0..8)
        .map(|i| board.max_tableau_run_length(i))
        .max()
        .unwrap_or(0)
}

/// Determines whether moving to foundation is "safe" (auto-play with no tactical risk).
fn is_safe_foundation_move(m: &FreeCellMove, board: &FreeCellBoard) -> bool {
    let card = match m {
        FreeCellMove::TableauToFoundation { card, .. } => *card,
        FreeCellMove::FreeCellToFoundation { card, .. } => *card,
        _ => return false,
    };

    let rank = card.value;
    if rank <= 2 {
        return true;
    }

    let min_opposite = Suit::ALL
        .iter()
        .filter(|&&s| s.color() != card.color())
        .map(|s| board.foundation[*s as usize].len())
        .min()
        .unwrap_or(0);
    let same_color_other = Suit::ALL
        .iter()
        .find(|&&s| s.color() == card.color() && s != card.suit)
        .map(|s| board.foundation[*s as usize].len())
        .unwrap_or(0);

    min_opposite >= (rank as usize).saturating_sub(1)
        && same_color_other >= (rank as usize).saturating_sub(2)
}
// Determines whether a rollback move from foundation should be pruned as counterproductive.
fn should_prune_rollback(
    m: &FreeCellMove,
    before_f: &BoardFeatures,
    after: &FreeCellBoard,
) -> bool {
    if !m.is_from_foundation() {
        return false;
    }
    if immediate_foundation_count(after) > before_f.immediate_foundation_moves {
        return false;
    }
    if blocked_low_penalty(after) < before_f.blocked_low_cards {
        return false;
    }
    if longest_run_length(after) > before_f.longest_run {
        return false;
    }
    true
}
// Computes local priority for candidate move ordering, combining net feature changes and tactical lookahead.
fn local_priority(m: &FreeCellMove, before_f: &BoardFeatures, after: &FreeCellBoard) -> i64 {
    let after_immediate = immediate_foundation_count(after) as i64;
    let after_empty_col = after.empty_tableau_columns() as i64;
    let after_free_used = after.free_cells.iter().filter(|c| c.is_some()).count();
    let after_longest = longest_run_length(after) as i64;
    let after_blocked = blocked_low_penalty(after);

    let foundation_gain = after.total_foundation_count() as i64 - before_f.foundation_count as i64;
    let options_gain = after_immediate - before_f.immediate_foundation_moves as i64;
    let empty_col_gain = after_empty_col - before_f.empty_tableau as i64;
    let free_freed = after_free_used as i64 - before_f.free_cells_used as i64;
    let run_gain = after_longest - before_f.longest_run as i64;
    let low_unblock = before_f.blocked_low_cards - after_blocked;

    // Tactical lookahead (uses values already computed from the after board).
    let tactical = after_immediate * weights::LOOKAHEAD_FOUNDATION_OPTIONS_WEIGHT
        + after_empty_col * weights::LOOKAHEAD_EMPTY_COLUMNS_WEIGHT
        - after_free_used as i64 * weights::LOOKAHEAD_FREE_USED_PENALTY
        - after_blocked * weights::LOOKAHEAD_LOW_BLOCKED_PENALTY;

    let base: i64 = match m {
        FreeCellMove::TableauToFoundation { .. } | FreeCellMove::FreeCellToFoundation { .. } => {
            weights::PRIORITY_BASE_TO_FOUNDATION
        }
        FreeCellMove::FreeCellToTableau { .. } => weights::PRIORITY_BASE_FREE_TO_TABLEAU,
        FreeCellMove::TableauToTableau { .. } => weights::PRIORITY_BASE_TABLEAU_TO_TABLEAU,
        FreeCellMove::TableauToFreeCell { .. } => weights::PRIORITY_BASE_TABLEAU_TO_FREE,
        FreeCellMove::FoundationToTableau { .. } => weights::PRIORITY_BASE_FROM_FOUNDATION,
        FreeCellMove::Deal { .. } => weights::PRIORITY_BASE_DEAL,
    };

    base + foundation_gain * weights::PRIORITY_FOUNDATION_GAIN_WEIGHT
        + options_gain * weights::PRIORITY_OPTIONS_GAIN_WEIGHT
        + empty_col_gain * weights::PRIORITY_EMPTY_COL_GAIN_WEIGHT
        + free_freed * weights::PRIORITY_FREE_FREED_WEIGHT
        + run_gain * weights::PRIORITY_RUN_GAIN_WEIGHT
        + low_unblock * weights::PRIORITY_LOW_UNBLOCK_WEIGHT
        + tactical
}

/// Safe auto-play chain to foundation:
/// after each move, cascades all available safe moves.
fn auto_play_safe_chain(board: &FreeCellBoard) -> Option<(FreeCellBoard, Vec<FreeCellMove>)> {
    let first = first_safe_foundation_move(board)?;
    let mut current = match first.apply(board) {
        Some(next) => next,
        None => return None,
    };
    let mut moves = vec![first];
    if is_win(&current) {
        return Some((current, moves));
    }
    loop {
        let m = match first_safe_foundation_move(&current) {
            Some(m) => m,
            None => break,
        };
        match m.apply(&current) {
            Some(next) => {
                moves.push(m);
                current = next;
                if is_win(&current) {
                    break;
                }
            }
            None => break,
        }
    }
    Some((current, moves))
}
// Finds the first safe foundation move (auto-play with no tactical risk), or None if none exists.
fn first_safe_foundation_move(board: &FreeCellBoard) -> Option<FreeCellMove> {
    for m in FreeCellMove::find_tableau_to_foundation(board) {
        if is_safe_foundation_move(&m, board) {
            return Some(m);
        }
    }
    for m in FreeCellMove::find_freecell_to_foundation(board) {
        if is_safe_foundation_move(&m, board) {
            return Some(m);
        }
    }
    None
}
// Checks whether the board is a winning position (all cards in foundation).
pub fn is_win(board: &FreeCellBoard) -> bool {
    Suit::ALL.iter().all(|&s| board.foundation_count(s) == 13)
}
// Generates a string with board features relevant to progress evaluation, useful for debugging and analysis.
pub fn progress_token(board: &FreeCellBoard) -> String {
    let f = compute_features(board);
    format!(
        "f:{}|i:{}|u:{}|e:{}|r:{}|b:{}|m:{}",
        f.foundation_count,
        f.immediate_foundation_moves,
        f.free_cells_used,
        f.empty_tableau,
        f.longest_run,
        f.blocked_low_cards,
        f.foundation_imbalance
    )
}

/// Pure FreeCell A* solver with node limit, timeout, and partial return.
pub struct FreeCellSolver {
    _private: (),
}

pub struct FreeCellSolveStats {
    pub moves: Option<Vec<FreeCellMove>>,
    pub checkpoints_adopted: u32,
}

impl FreeCellSolver {
    pub fn new() -> FreeCellSolver {
        FreeCellSolver { _private: () }
    }

    pub fn solve(
        &self,
        board: &FreeCellBoard,
        timeout_secs: f64,
        allow_partial: bool,
    ) -> Option<Vec<FreeCellMove>> {
        self.solve_with_stats(board, timeout_secs, allow_partial)
            .moves
    }

    /// Runs A* with node limit, timeout, and optional partial return.
    pub fn solve_with_stats(
        &self,
        board: &FreeCellBoard,
        timeout_secs: f64,
        allow_partial: bool,
    ) -> FreeCellSolveStats {
        if is_win(board) {
            return FreeCellSolveStats {
                moves: Some(vec![]),
                checkpoints_adopted: 0,
            };
        }

        let start = Instant::now();
        let timeout = if timeout_secs > 0.0 {
            Some(Duration::from_secs_f64(timeout_secs))
        } else {
            None
        };

        // Node arena for path reconstruction by index.
        let mut nodes: Vec<FcSearchNode> = vec![FcSearchNode {
            parent_index: None,
            move_sequence: None,
            depth: 0,
            path_cost: 0,
        }];

        let mut frontier = MinHeap::new();
        let root_features = compute_features(board);
        let root_h = heuristic_cost(&root_features);
        frontier.push(FcFrontierItem {
            node_index: 0,
            board: board.clone(),
            g_score: 0,
            h_score: root_h,
        });

        // Best known g-cost by board signature (dominance table).
        let mut best_cost: HashMap<u64, usize> = HashMap::new();
        best_cost.insert(board.signature, 0);

        // Best-progress tracking for partial return.
        let mut best_progress_idx: usize = 0;
        let mut best_progress = progress_score(&root_features);
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
            let current_board = current.board;

            // Dominance pruning: ignores paths with worse cost than the best known.
            if let Some(&known) = best_cost.get(&current_board.signature) {
                if known != node_cost {
                    continue;
                }
            }

            // Complete solution.
            if is_win(&current_board) {
                return FreeCellSolveStats {
                    moves: Some(reconstruct_moves(current.node_index, &nodes)),
                    checkpoints_adopted: 0,
                };
            }

            // Respect depth limit.
            if node_depth >= weights::MAX_DEPTH {
                continue;
            }

            // Generate and expand successors of the current node.
            let candidates = find_all_moves(&current_board);
            let expansions = expand_moves(node_cost, candidates);

            for expansion in expansions {
                let next_depth = node_depth + expansion.move_sequence.len();
                if next_depth > weights::MAX_DEPTH {
                    continue;
                }
                let next_g = expansion.path_cost;
                // Cost pruning if a better route to the same state already exists.
                if let Some(&known) = best_cost.get(&expansion.board.signature) {
                    if known <= next_g {
                        continue;
                    }
                }

                let child_idx = nodes.len();
                nodes.push(FcSearchNode {
                    parent_index: Some(current.node_index),
                    move_sequence: Some(expansion.move_sequence),
                    depth: next_depth,
                    path_cost: next_g,
                });
                best_cost.insert(expansion.board.signature, next_g);

                // Update best partial progress.
                let features = compute_features(&expansion.board);
                let score = progress_score(&features);
                if score > best_progress {
                    best_progress = score;
                    best_progress_idx = child_idx;
                }

                let h = heuristic_cost(&features);
                frontier.push(FcFrontierItem {
                    node_index: child_idx,
                    board: expansion.board,
                    g_score: next_g,
                    h_score: h,
                });
            }
        }

        // No solution: return best partial progress if requested.
        if allow_partial && best_progress_idx != 0 {
            return FreeCellSolveStats {
                moves: Some(reconstruct_moves(best_progress_idx, &nodes)),
                checkpoints_adopted: 0,
            };
        }

        FreeCellSolveStats {
            moves: None,
            checkpoints_adopted: 0,
        }
    }
}

pub fn deal(deck: &[Card]) -> Option<FreeCellBoard> {
    let m = FreeCellMove::Deal {
        deck: deck.to_vec(),
    };
    m.apply(&FreeCellBoard::new(
        [None; 4],
        [vec![], vec![], vec![], vec![]],
        Default::default(),
    ))
}

fn find_all_moves(board: &FreeCellBoard) -> Vec<(FreeCellMove, FreeCellBoard)> {
    let before_f = compute_features(board);
    let tab_to_found = FreeCellMove::find_tableau_to_foundation(board);
    let fc_to_found = FreeCellMove::find_freecell_to_foundation(board);

    let mut candidates: Vec<(FreeCellMove, FreeCellBoard)> = Vec::new();

    let mut ordered_moves: Vec<FreeCellMove> = tab_to_found
        .iter()
        .chain(fc_to_found.iter())
        .filter(|m| is_safe_foundation_move(m, board))
        .cloned()
        .collect();
    ordered_moves.extend(FreeCellMove::find_foundation_to_tableau(board));
    ordered_moves.extend(FreeCellMove::find_tableau_to_tableau(board));
    ordered_moves.extend(FreeCellMove::find_freecell_to_tableau(board));
    ordered_moves.extend(FreeCellMove::find_tableau_to_freecell(board));

    for m in &ordered_moves {
        if let Some(next_board) = m.apply(board) {
            if should_prune_rollback(m, &before_f, &next_board) {
                continue;
            }
            candidates.push((m.clone(), next_board));
        }
    }

    // Endgame: allow unsafe foundation moves if enough progress has already been made.
    if before_f.foundation_count >= weights::ENDGAME_UNSAFE_FOUNDATION_THRESHOLD {
        for m in tab_to_found.iter().chain(fc_to_found.iter()) {
            if is_safe_foundation_move(m, board) {
                continue;
            }
            if candidates.iter().any(|(cm, _)| cm == m) {
                continue;
            }
            if let Some(next_board) = m.apply(board) {
                candidates.push((m.clone(), next_board));
            }
        }
    }

    candidates.sort_by_cached_key(|(m, b)| std::cmp::Reverse(local_priority(m, &before_f, b)));

    candidates
}

fn expand_moves(
    base_cost: usize,
    candidates: Vec<(FreeCellMove, FreeCellBoard)>,
) -> Vec<FcExpansion> {
    candidates
        .into_iter()
        .map(|(m, move_board)| {
            let mut move_seq: Vec<FreeCellMove> = vec![m];
            let mut final_board = move_board;

            // After each move, try chaining safe auto-play to foundation.
            if let Some((chain_board, chain_moves)) = auto_play_safe_chain(&final_board) {
                move_seq.extend(chain_moves);
                final_board = chain_board;
            }

            let cost = base_cost + move_seq.len();
            FcExpansion {
                board: final_board,
                move_sequence: move_seq,
                path_cost: cost,
            }
        })
        .collect()
}

fn heuristic_cost(f: &BoardFeatures) -> i64 {
    (52 - f.foundation_count) as i64 * weights::HEURISTIC_FOUNDATION_REMAINING_WEIGHT
        + f.free_cells_used as i64 * weights::HEURISTIC_FREE_USED_PENALTY
        + f.blocked_low_cards * weights::HEURISTIC_LOW_BLOCKED_PENALTY
        + f.foundation_imbalance * weights::HEURISTIC_FOUNDATION_IMBALANCE_PENALTY
        - f.empty_tableau as i64 * weights::HEURISTIC_EMPTY_TABLEAU_BONUS
        - f.longest_run as i64 * weights::HEURISTIC_LONGEST_RUN_BONUS
        - f.immediate_foundation_moves as i64 * weights::HEURISTIC_IMMEDIATE_FOUNDATION_BONUS
}

fn progress_score(f: &BoardFeatures) -> i64 {
    f.foundation_count as i64 * weights::PROGRESS_FOUNDATION_WEIGHT
        + f.immediate_foundation_moves as i64 * weights::PROGRESS_IMMEDIATE_FOUNDATION_WEIGHT
        + f.empty_tableau as i64 * weights::PROGRESS_EMPTY_TABLEAU_BONUS
        + f.longest_run as i64 * weights::PROGRESS_LONGEST_RUN_BONUS
        - f.free_cells_used as i64 * weights::PROGRESS_FREE_USED_PENALTY
        - f.blocked_low_cards * weights::PROGRESS_LOW_BLOCKED_PENALTY
}

fn reconstruct_moves(from: usize, nodes: &[FcSearchNode]) -> Vec<FreeCellMove> {
    let mut moves = Vec::new();
    let mut cursor: Option<usize> = Some(from);
    while let Some(idx) = cursor {
        if let Some(ref seq) = nodes[idx].move_sequence {
            for m in seq.iter().rev() {
                moves.push(m.clone());
            }
        }
        cursor = nodes[idx].parent_index;
    }
    moves.reverse();
    moves
}

// Search arena node for path reconstruction.
struct FcSearchNode {
    parent_index: Option<usize>,
    move_sequence: Option<Vec<FreeCellMove>>,
    depth: usize,
    path_cost: usize,
}

// Result of expanding a candidate move.
struct FcExpansion {
    board: FreeCellBoard,
    move_sequence: Vec<FreeCellMove>,
    path_cost: usize,
}

// Item stored in the A* frontier.
struct FcFrontierItem {
    node_index: usize,
    board: FreeCellBoard,
    g_score: usize,
    h_score: i64,
}

impl FcFrontierItem {
    fn f_score(&self) -> i64 {
        self.g_score as i64 + self.h_score
    }
}

// Min-heap for prioritizing the A* frontier.
struct MinHeap {
    storage: Vec<FcFrontierItem>,
}

impl MinHeap {
    fn new() -> MinHeap {
        MinHeap {
            storage: Vec::new(),
        }
    }

    fn push(&mut self, item: FcFrontierItem) {
        self.storage.push(item);
        self.sift_up(self.storage.len() - 1);
    }

    fn pop(&mut self) -> Option<FcFrontierItem> {
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

    fn is_higher(a: &FcFrontierItem, b: &FcFrontierItem) -> bool {
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
