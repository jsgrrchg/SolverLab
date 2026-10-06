use ahash::{AHashMap, AHashSet};
use std::time::{Duration, Instant};

use super::board::KlondikeBoard;
use super::board_eval::{BoardEval, CheckpointPolicy};
use super::moves::KlondikeMove;
use super::rules::{KlondikeRule, KlondikeTransition};
use super::weights;
use crate::common::card::Suit;

// ═══════════════════════════════════════════
// Transposition table key
// ═══════════════════════════════════════════

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct KlondikeTtKey {
    signature: u64,
    undo_bucket: u16,
}
const TT_UNDO_SENTINEL: u16 = u16::MAX;

// ═══════════════════════════════════════════
// Solver
// ═══════════════════════════════════════════

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum KlondikeSolveMode {
    Fast,
    Strict,
}

pub struct KlondikeSolver {
    pub draw_advance: u8,
    pub rules: Vec<KlondikeRule>,
    pub mode: KlondikeSolveMode,
    pub max_undos: Option<usize>,
}

pub struct KlondikeSolveStats {
    pub moves: Option<Vec<KlondikeMove>>,
    pub checkpoints_adopted: u32,
}

impl KlondikeSolver {
    pub fn new(draw_advance: i32) -> KlondikeSolver {
        Self::new_with_mode(draw_advance, KlondikeSolveMode::Fast)
    }

    pub fn new_with_mode(draw_advance: i32, mode: KlondikeSolveMode) -> KlondikeSolver {
        let da = if draw_advance > 0 {
            draw_advance as u8
        } else {
            3
        };
        let rules = match mode {
            KlondikeSolveMode::Fast => KlondikeRule::default_rules_fast(weights::MAX_DEPTH),
            KlondikeSolveMode::Strict => KlondikeRule::default_rules_strict(weights::MAX_DEPTH),
        };
        KlondikeSolver {
            draw_advance: da,
            rules,
            mode,
            max_undos: Some(weights::MAX_UNDOS),
        }
    }

    pub fn is_win(board: &KlondikeBoard) -> bool {
        Suit::ALL.iter().all(|&s| board.foundation_count(s) == 13)
    }

    fn tt_key(&self, signature: u64, undo_count: usize) -> KlondikeTtKey {
        let undo_bucket = if self.max_undos.is_some() {
            undo_count.min((u16::MAX - 1) as usize) as u16
        } else {
            TT_UNDO_SENTINEL
        };
        KlondikeTtKey {
            signature,
            undo_bucket,
        }
    }

    fn should_prune_by_tt(
        tt: &AHashMap<KlondikeTtKey, i64>,
        key: KlondikeTtKey,
        remaining_budget: i64,
    ) -> bool {
        if let Some(&stored) = tt.get(&key) {
            stored >= remaining_budget
        } else {
            false
        }
    }

    fn upsert_remaining_budget(
        tt: &mut AHashMap<KlondikeTtKey, i64>,
        key: KlondikeTtKey,
        remaining_budget: i64,
    ) {
        let entry = tt.entry(key).or_insert(i64::MIN);
        if remaining_budget > *entry {
            *entry = remaining_budget;
        }
    }

    // ── Successor generation ─────────────────

    pub fn successors(
        &self,
        board: &KlondikeBoard,
        previous_move: Option<KlondikeMove>,
        depth: usize,
    ) -> Vec<KlondikeTransition> {
        let candidate_moves = KlondikeMove::find_candidate_moves(board, self.draw_advance);
        let next_depth = depth + 1;

        // Fast path: detect safe foundation moves without copying boards.
        // If any exist, only generate transitions for those moves, saving
        // apply() + compute_signature for all other candidates.
        if candidate_moves
            .iter()
            .any(|m| is_safe_move_pre_check(m, board))
        {
            let mut transitions = Vec::new();
            for the_move in &candidate_moves {
                if !is_safe_move_pre_check(the_move, board) {
                    continue;
                }
                if let Some(next_board) = the_move.apply(*board) {
                    transitions.push(KlondikeTransition {
                        to_board: next_board,
                        the_move: *the_move,
                        previous_move,
                        depth: next_depth,
                    });
                }
            }
            return transitions;
        }

        // Normal path: generate all transitions with pre- and post-apply pruning.
        let mut transitions = Vec::with_capacity(candidate_moves.len());

        for the_move in candidate_moves {
            // Early pruning: evaluate rules that do not need to_board before
            // copying the board and computing the signature.
            if self
                .rules
                .iter()
                .any(|r| r.can_prune_early(board, the_move, previous_move, next_depth))
            {
                continue;
            }
            if let Some(next_board) = the_move.apply(*board) {
                let transition = KlondikeTransition {
                    to_board: next_board,
                    the_move,
                    previous_move,
                    depth: next_depth,
                };
                if !self
                    .rules
                    .iter()
                    .any(|r| r.should_prune(&transition, board))
                {
                    transitions.push(transition);
                }
            }
        }

        transitions.sort_by_cached_key(|t| successor_order_key(t, board));
        transitions
    }

    // ── Public solver entry point ──

    pub fn solve(
        &self,
        initial_board: &KlondikeBoard,
        allow_partial: bool,
    ) -> Option<Vec<KlondikeMove>> {
        self.solve_with_stats(initial_board, allow_partial).moves
    }

    pub fn solve_with_stats(
        &self,
        initial_board: &KlondikeBoard,
        allow_partial: bool,
    ) -> KlondikeSolveStats {
        if Self::is_win(initial_board) {
            return KlondikeSolveStats {
                moves: Some(vec![]),
                checkpoints_adopted: 0,
            };
        }

        let start = Instant::now();
        let timeout = Some(Duration::from_secs_f64(weights::TIMEOUT_SECS));

        let mut unified_moves = Vec::new();
        let mut current_board = *initial_board;
        current_board.compute_signature();
        let eval = BoardEval::from_board(&current_board, self.draw_advance);
        let mut context = IdaContext::new(start, timeout, eval.progress_score(), self.draw_advance);

        loop {
            if context.timeout_reached() {
                break;
            }

            // Search chunk: explore from the current board until finding a solution,
            // running out of time, or reaching an adoptable checkpoint.
            let chunk_eval = BoardEval::from_board(&current_board, self.draw_advance);
            let start_score = chunk_eval.progress_score();
            context.reset_chunk_locals(&current_board, start_score);

            let mut tt: AHashMap<KlondikeTtKey, i64> = AHashMap::new();
            let mut bound =
                BoardEval::from_board_fast(&current_board, self.draw_advance).heuristic_cost();
            let mut chunk_solution = None;
            let mut triggered_checkpoint = false;

            loop {
                if context.timeout_reached() {
                    break;
                }

                // Restart the IDA* frontier: generate successors from the current root.
                let root_transitions = self.successors(&current_board, None, 0);
                if root_transitions.is_empty() {
                    break;
                }

                let mut path_signatures = AHashSet::new();
                path_signatures.insert(current_board.signature);
                Self::upsert_remaining_budget(
                    &mut tt,
                    self.tt_key(current_board.signature, 0),
                    bound,
                );

                let mut min_next_bound = i64::MAX;
                let mut chunk_loop_break = false;

                for transition in root_transitions {
                    let mut path = vec![transition.the_move];
                    let next_board = transition.to_board;
                    let next_signature = next_board.signature;
                    let initial_undo = if transition.the_move.is_from_foundation() {
                        1
                    } else {
                        0
                    };

                    if let Some(max) = self.max_undos {
                        if initial_undo > max {
                            continue;
                        }
                    }

                    let next_remaining_budget = bound - 1;
                    let next_tt_key = self.tt_key(next_signature, initial_undo);
                    if Self::should_prune_by_tt(&tt, next_tt_key, next_remaining_budget) {
                        continue;
                    }

                    path_signatures.insert(next_signature);
                    Self::upsert_remaining_budget(&mut tt, next_tt_key, next_remaining_budget);
                    context.consider_progress(&next_board, &path);

                    let result = self.search_with_bound(
                        &next_board,
                        1,
                        bound,
                        &mut path,
                        initial_undo,
                        &mut path_signatures,
                        &mut tt,
                        &mut context,
                    );

                    match result {
                        IdaSearchResult::Found => {
                            chunk_solution = context.solution_path.take();
                            chunk_loop_break = true;
                            break;
                        }
                        IdaSearchResult::Timeout => {
                            chunk_loop_break = true;
                            break;
                        }
                        IdaSearchResult::CheckpointTriggered => {
                            triggered_checkpoint = true;
                            chunk_loop_break = true;
                            break;
                        }
                        IdaSearchResult::NextBound(nb) => {
                            if nb < min_next_bound {
                                min_next_bound = nb;
                            }
                        }
                    }
                    path_signatures.remove(&next_signature);
                }

                // If no solution was found, increase the bound to the next observed minimum.
                if chunk_solution.is_some() || chunk_loop_break || triggered_checkpoint {
                    break;
                }
                if min_next_bound == i64::MAX {
                    break;
                }
                if min_next_bound <= bound {
                    bound += 1;
                } else {
                    bound = min_next_bound;
                }
            }

            // Solution found.
            if let Some(solution) = chunk_solution {
                unified_moves.extend(solution);
                return KlondikeSolveStats {
                    moves: Some(unified_moves),
                    checkpoints_adopted: context.checkpoints_adopted,
                };
            }

            if context.timeout_reached() {
                break;
            }

            // Checkpoint adoption: if useful progress was made, advance the base state.
            if triggered_checkpoint && context.is_progress_adoptable(start_score, &current_board) {
                let best_board_result = apply_path(current_board, &context.best_progress_path);
                if let Some(best_board) = best_board_result {
                    if !context
                        .adopted_checkpoint_hashes
                        .contains(&best_board.signature)
                    {
                        context
                            .adopted_checkpoint_hashes
                            .insert(best_board.signature);
                        context.checkpoints_adopted += 1;
                        unified_moves.extend(context.best_progress_path.clone());
                        current_board = best_board;

                        if context.checkpoints_adopted >= context.policy.max_adoptions {
                            break;
                        }
                        continue;
                    }
                }
            }

            // Dead end: no adoptable checkpoint.
            break;
        }

        if allow_partial && (!unified_moves.is_empty() || !context.best_progress_path.is_empty()) {
            if !context.best_progress_path.is_empty() {
                let cur_score =
                    BoardEval::from_board(&current_board, self.draw_advance).progress_score();
                if context.best_progress > cur_score {
                    unified_moves.extend(context.best_progress_path.clone());
                }
            }
            if !unified_moves.is_empty() {
                return KlondikeSolveStats {
                    moves: Some(unified_moves),
                    checkpoints_adopted: context.checkpoints_adopted,
                };
            }
        }
        KlondikeSolveStats {
            moves: None,
            checkpoints_adopted: context.checkpoints_adopted,
        }
    }

    // ── Main IDA* recursion ─────────────

    fn search_with_bound(
        &self,
        board: &KlondikeBoard,
        depth: usize,
        bound: i64,
        path: &mut Vec<KlondikeMove>,
        undo_count: usize,
        path_signatures: &mut AHashSet<u64>,
        tt: &mut AHashMap<KlondikeTtKey, i64>,
        context: &mut IdaContext,
    ) -> IdaSearchResult {
        if context.should_timeout() {
            return IdaSearchResult::Timeout;
        }
        if context.should_checkpoint() {
            return IdaSearchResult::CheckpointTriggered;
        }

        // Limit memory: clear the TT when it exceeds the configured threshold.
        if tt.len() >= weights::TT_MAX_ENTRIES {
            tt.clear();
        }

        let h_cost = BoardEval::from_board_fast(board, self.draw_advance).heuristic_cost();
        let f_score = depth as i64 + h_cost;
        // Typical IDA* pruning: the node exceeds the f = g + h bound.
        if f_score > bound {
            return IdaSearchResult::NextBound(f_score);
        }

        if Self::is_win(board) {
            context.solution_path = Some(path.clone());
            return IdaSearchResult::Found;
        }

        let prev_move = path.last().copied();
        let transitions = self.successors(board, prev_move, depth);
        let mut min_next_bound = i64::MAX;

        for transition in transitions {
            let next_board = transition.to_board;
            let next_signature = next_board.signature;
            let next_depth = depth + 1;
            let next_remaining_budget = bound - next_depth as i64;
            let next_undo_count = if transition.the_move.is_from_foundation() {
                undo_count + 1
            } else {
                undo_count
            };

            if let Some(max) = self.max_undos {
                if next_undo_count > max {
                    continue;
                }
            }
            // Avoid cycles in the current path (depth-first search).
            if path_signatures.contains(&next_signature) {
                continue;
            }

            let next_tt_key = self.tt_key(next_signature, next_undo_count);
            if Self::should_prune_by_tt(tt, next_tt_key, next_remaining_budget) {
                continue;
            }

            Self::upsert_remaining_budget(tt, next_tt_key, next_remaining_budget);
            path_signatures.insert(next_signature);
            path.push(transition.the_move);
            context.consider_progress(&next_board, path);

            match self.search_with_bound(
                &next_board,
                next_depth,
                bound,
                path,
                next_undo_count,
                path_signatures,
                tt,
                context,
            ) {
                IdaSearchResult::Found => return IdaSearchResult::Found,
                IdaSearchResult::Timeout => return IdaSearchResult::Timeout,
                IdaSearchResult::CheckpointTriggered => {
                    return IdaSearchResult::CheckpointTriggered
                }
                IdaSearchResult::NextBound(nb) => {
                    if nb < min_next_bound {
                        min_next_bound = nb;
                    }
                }
            }

            path.pop();
            path_signatures.remove(&next_signature);
        }
        IdaSearchResult::NextBound(min_next_bound)
    }
}

// ═══════════════════════════════════════════
// IDA* result and context
// ═══════════════════════════════════════════

pub enum IdaSearchResult {
    Found,
    NextBound(i64),
    Timeout,
    CheckpointTriggered,
}

pub struct IdaContext {
    start: Instant,
    timeout: Option<Duration>,
    checked_nodes: u64,
    progress_sample_counter: u64,
    pub best_progress: i64,
    pub best_progress_path: Vec<KlondikeMove>,
    pub solution_path: Option<Vec<KlondikeMove>>,
    pub checkpoint_nodes: u64,
    pub checkpoint_node_limit: u64,
    pub adopted_checkpoint_hashes: AHashSet<u64>,
    pub checkpoints_adopted: u32,
    pub policy: CheckpointPolicy,
    pub draw_advance: u8,
}

impl IdaContext {
    pub fn new(
        start: Instant,
        timeout: Option<Duration>,
        initial_progress: i64,
        draw_advance: u8,
    ) -> Self {
        Self {
            start,
            timeout,
            checked_nodes: 0,
            progress_sample_counter: 0,
            checkpoint_nodes: 0,
            checkpoint_node_limit: weights::CHECKPOINT_FALLBACK_LIMIT,
            best_progress: initial_progress,
            best_progress_path: Vec::new(),
            solution_path: None,
            adopted_checkpoint_hashes: AHashSet::new(),
            checkpoints_adopted: 0,
            policy: CheckpointPolicy::default_policy(),
            draw_advance,
        }
    }

    pub fn reset_chunk_locals(&mut self, board: &KlondikeBoard, initial_progress: i64) {
        self.checkpoint_nodes = 0;
        self.checkpoint_node_limit = self.policy.base_limit_for(board);
        self.best_progress = initial_progress;
        self.best_progress_path.clear();
    }

    pub fn timeout_reached(&self) -> bool {
        match self.timeout {
            Some(t) => self.start.elapsed() > t,
            None => false,
        }
    }

    pub fn should_timeout(&mut self) -> bool {
        self.checked_nodes += 1;
        if self.checked_nodes & 2047 != 0 {
            return false;
        }
        self.timeout_reached()
    }

    pub fn is_progress_adoptable(&self, start_score_check: i64, board: &KlondikeBoard) -> bool {
        if self.best_progress_path.is_empty() {
            return false;
        }
        let fc = board.total_foundation_count();
        self.policy
            .is_adoptable(start_score_check, self.best_progress, fc)
    }

    pub fn should_checkpoint(&mut self) -> bool {
        self.checkpoint_nodes += 1;
        self.checkpoint_nodes >= self.checkpoint_node_limit
    }

    pub fn consider_progress(&mut self, board: &KlondikeBoard, path: &[KlondikeMove]) {
        self.progress_sample_counter += 1;
        if self.progress_sample_counter & 3 != 0 {
            return;
        }

        let score = BoardEval::from_board(board, self.draw_advance).progress_score();

        if score > self.best_progress {
            self.best_progress = score;
            self.best_progress_path = path.to_vec();
        }
    }
}

// ═══════════════════════════════════════════
// Helper functions
// ═══════════════════════════════════════════

fn apply_path(mut board: KlondikeBoard, path: &[KlondikeMove]) -> Option<KlondikeBoard> {
    for m in path {
        board = m.apply(board)?;
    }
    Some(board)
}

pub fn progress_token(board: &KlondikeBoard) -> String {
    let fc = board.total_foundation_count();
    let face_up: usize = board.columns.iter().map(|c| c.num_face_up()).sum();
    let stock_remaining = board.stock_len.saturating_sub(board.stock_index);
    let waste = board.stock_index;
    format!("f:{}|u:{}|s:{}|w:{}", fc, face_up, stock_remaining, waste)
}

// ═══════════════════════════════════════════
// Successor ordering helpers
// ═══════════════════════════════════════════

fn successor_order_key(
    transition: &KlondikeTransition,
    from_board: &KlondikeBoard,
) -> (u8, std::cmp::Reverse<i64>) {
    // Lexicographic order: group first (move type), then local priority.
    let revealed_delta = revealed_hidden_delta(transition, from_board);
    let exposed_bonus = revealed_delta as i64 * weights::EXPOSED_BONUS_MULTIPLIER;
    let thoughtful_bonus = thoughtful_reveal_value(transition, from_board);
    let crit_bonus = critical_path_bonus(transition, from_board);
    let bucket = successor_bucket(transition, from_board, revealed_delta);
    let secondary = local_secondary_priority(
        transition,
        from_board,
        exposed_bonus + thoughtful_bonus + crit_bonus,
    );
    (bucket, std::cmp::Reverse(secondary))
}

fn successor_bucket(
    transition: &KlondikeTransition,
    from_board: &KlondikeBoard,
    revealed_delta: isize,
) -> u8 {
    // Groups with lower indexes are explored first.
    if revealed_delta > 0 {
        return 0;
    }
    if is_safe_foundation_move(transition, from_board) {
        return 1;
    }
    if creates_empty_column(transition, from_board) {
        return 2;
    }
    if moves_large_stack(transition) {
        return 3;
    }
    if transition.the_move.is_stock_advance() {
        return 4;
    }
    if transition.the_move.is_from_foundation() {
        return 5;
    }
    if transition.the_move.is_stock_recycle() {
        return 6;
    }
    5
}

fn local_secondary_priority(
    transition: &KlondikeTransition,
    from_board: &KlondikeBoard,
    exposed_bonus: i64,
) -> i64 {
    let m = &transition.the_move;
    if is_safe_foundation_move(transition, from_board) {
        return weights::PRIORITY_SAFE_FOUNDATION + exposed_bonus;
    }
    if m.is_to_foundation() {
        return weights::PRIORITY_TO_FOUNDATION + exposed_bonus;
    }
    if m.is_from_foundation() {
        return weights::PRIORITY_FROM_FOUNDATION;
    }
    if m.is_stock_advance() {
        return weights::PRIORITY_STOCK_ADVANCE + exposed_bonus;
    }
    if m.is_stock_recycle() {
        return weights::PRIORITY_STOCK_RECYCLE;
    }

    let mut score = weights::PRIORITY_COLUMN_BASE + exposed_bonus;
    if let KlondikeMove::ColumnToColumn { count, .. } = m {
        score += (*count as i64).saturating_sub(1) * weights::PRIORITY_STACK_BONUS;
        if creates_empty_column(transition, from_board) {
            score += weights::PRIORITY_EMPTY_COL_BONUS;
        }
    }
    score
}

/// Counts newly revealed cards, excluding transfers into the tableau.
fn revealed_hidden_delta(transition: &KlondikeTransition, from_board: &KlondikeBoard) -> isize {
    let source = match transition.the_move {
        KlondikeMove::ColumnToColumn { source, .. }
        | KlondikeMove::ColumnToFoundation { source, .. } => source as usize,
        _ => return 0,
    };
    let before = from_board.columns[source].face_down_len;
    let after = transition.to_board.columns[source].face_down_len;
    before.saturating_sub(after) as isize
}

fn thoughtful_reveal_value(transition: &KlondikeTransition, from_board: &KlondikeBoard) -> i64 {
    let from = from_board;
    let to = &transition.to_board;
    let mut bonus = 0i64;

    for i in 0..7 {
        let from_fd = from.columns[i].face_down_len;
        let to_fd = to.columns[i].face_down_len;

        if from_fd > 0 && to_fd < from_fd && to.columns[i].has_face_up() {
            let revealed = to.columns[i].cards[to_fd as usize];
            let target_val = to.foundation[revealed.suit as usize] + 1;

            if target_val <= 13 && revealed.value == target_val {
                bonus += weights::THOUGHTFUL_REVEAL_TARGET;
            } else if revealed.value == 13 {
                let empty_cols = to.columns.iter().filter(|c| c.len == 0).count();
                if empty_cols > 0 {
                    bonus += weights::THOUGHTFUL_REVEAL_KING;
                }
            } else if target_val <= 13 && revealed.value <= target_val + 2 {
                bonus += weights::THOUGHTFUL_REVEAL_NEAR_TARGET;
            }
        }
    }
    bonus
}

/// Bonus for moves that remove cards from a column with a buried target.
/// The solver knows which cards are face-down; prioritizing blockers from columns
/// containing the next target of some suit speeds up the critical path to the solution.
fn critical_path_bonus(transition: &KlondikeTransition, from_board: &KlondikeBoard) -> i64 {
    let board = from_board;
    let source_col = match transition.the_move {
        KlondikeMove::ColumnToColumn { source, .. } => source as usize,
        KlondikeMove::ColumnToFoundation { source, .. } => source as usize,
        _ => return 0,
    };

    let col = &board.columns[source_col];
    if col.face_down_len == 0 {
        return 0;
    }

    // Check whether any face-down card in this column is the next target.
    for pos in 0..(col.face_down_len as usize) {
        let card = col.cards[pos];
        let target_val = board.foundation[card.suit as usize] + 1;
        if card.value == target_val {
            return weights::CRITICAL_PATH_MOVE_BONUS;
        }
    }
    0
}

fn creates_empty_column(transition: &KlondikeTransition, from_board: &KlondikeBoard) -> bool {
    // Detects whether the move completely empties the source column.
    // This is usually valuable because it enables moving Kings and restructuring piles.
    match transition.the_move {
        KlondikeMove::ColumnToColumn { source, count, .. } => {
            let src = &from_board.columns[source as usize];
            count > 0 && src.len == count
        }
        _ => false,
    }
}

fn moves_large_stack(transition: &KlondikeTransition) -> bool {
    // Prioritizes moving a "large" stack (2+ cards), which tends to unlock
    // more moves than a single-card move.
    matches!(transition.the_move, KlondikeMove::ColumnToColumn { count, .. } if count >= 2)
}

/// Pre-apply version of safe foundation move detection.
/// Only needs the move and foundation state, without copying the board.
fn is_safe_move_pre_check(m: &KlondikeMove, board: &KlondikeBoard) -> bool {
    let card = match m {
        KlondikeMove::ColumnToFoundation { card, .. } => *card,
        KlondikeMove::StockPileToFoundation { card } => *card,
        _ => return false,
    };
    if card.value <= 2 {
        return true;
    }
    let f = &board.foundation;
    let (opp_a, opp_b) = match card.suit {
        Suit::Club | Suit::Spade => (f[Suit::Diamond as usize], f[Suit::Heart as usize]),
        Suit::Diamond | Suit::Heart => (f[Suit::Club as usize], f[Suit::Spade as usize]),
    };
    let needed = card.value.saturating_sub(1);
    opp_a >= needed && opp_b >= needed
}

fn is_safe_foundation_move(transition: &KlondikeTransition, from_board: &KlondikeBoard) -> bool {
    // Only applies to moves toward foundation.
    // If the move does not end in foundation, it is not considered "safe".
    let card = match transition.the_move {
        KlondikeMove::ColumnToFoundation { card, .. } => card,
        KlondikeMove::StockPileToFoundation { card } => card,
        _ => return false,
    };

    // Aces and twos are always safe: they do not block relevant progression.
    if card.value <= 2 {
        return true;
    }

    // Classic safety rule:
    // moving up a card of one color is safe when both foundations of the
    // opposite color have already reached (value - 1).
    let f = &from_board.foundation;
    let (opp_a, opp_b) = match card.suit {
        Suit::Club | Suit::Spade => (f[Suit::Diamond as usize], f[Suit::Heart as usize]),
        Suit::Diamond | Suit::Heart => (f[Suit::Club as usize], f[Suit::Spade as usize]),
    };
    let needed = card.value.saturating_sub(1);
    opp_a >= needed && opp_b >= needed
}

pub fn deal(deck: &[crate::common::card::Card]) -> Option<KlondikeBoard> {
    // Re-exposes the board module deal function to keep a high-level
    // solver-centered API.
    super::board::deal(deck)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::card::Card;

    // Small legal move fixtures exercise ordering without running a full search.
    fn card(suit: Suit, value: u8) -> Card {
        Card::new(suit, value)
    }

    #[test]
    fn successors_prioritize_reveals_over_stock_and_foundation_transfers() {
        let mut board = KlondikeBoard::new();
        board.foundation[Suit::Heart as usize] = 4;
        board.columns[0].push(card(Suit::Diamond, 9), true);
        board.columns[0].push(card(Suit::Heart, 5), false);
        board.columns[1].push(card(Suit::Club, 6), false);
        board.columns[2].push(card(Suit::Spade, 5), false);
        board.stock[0] = card(Suit::Diamond, 5);
        board.stock[1] = card(Suit::Spade, 9);
        board.stock_len = 2;
        board.stock_index = 1;
        board.compute_signature();

        let foundation_reveal = KlondikeMove::ColumnToFoundation {
            source: 0,
            card: card(Suit::Heart, 5),
        };
        let column_reveal = KlondikeMove::ColumnToColumn {
            source: 0,
            destination: 1,
            count: 1,
        };
        let stock_transfer = KlondikeMove::StockPileToColumn {
            destination: 1,
            card: card(Suit::Diamond, 5),
        };
        let foundation_transfer = KlondikeMove::FoundationToColumn {
            destination: 2,
            card: card(Suit::Heart, 4),
        };

        for draw in [1, 3] {
            let solver = KlondikeSolver::new(draw);
            let transitions = solver.successors(&board, None, 0);
            let position = |m| transitions.iter().position(|t| t.the_move == m).unwrap();
            let advance = KlondikeMove::StockPileAdvance {
                beginning_index: 1,
                increment: draw as u8,
            };

            assert!(position(foundation_reveal) < position(advance));
            assert!(position(column_reveal) < position(advance));
            assert!(position(advance) < position(stock_transfer));
            assert!(position(advance) < position(foundation_transfer));

            for m in [foundation_reveal, column_reveal] {
                assert_eq!(revealed_hidden_delta(&transitions[position(m)], &board), 1);
            }
            for m in [stock_transfer, foundation_transfer, advance] {
                assert_eq!(revealed_hidden_delta(&transitions[position(m)], &board), 0);
            }

            // None of this fixture's candidates is pruned: ordering preserves them all.
            let candidates = KlondikeMove::find_candidate_moves(&board, draw as u8);
            assert_eq!(transitions.len(), candidates.len());
            for m in candidates {
                assert!(transitions.iter().any(|t| t.the_move == m));
            }
        }
    }

    #[test]
    fn column_moves_without_hidden_cards_are_not_reveals() {
        let mut board = KlondikeBoard::new();
        board.foundation[Suit::Heart as usize] = 4;
        board.columns[0].push(card(Suit::Club, 6), false);
        board.columns[0].push(card(Suit::Heart, 5), false);
        board.columns[1].push(card(Suit::Spade, 6), false);
        board.compute_signature();

        let transitions = KlondikeSolver::new(1).successors(&board, None, 0);
        for m in [
            KlondikeMove::ColumnToFoundation {
                source: 0,
                card: card(Suit::Heart, 5),
            },
            KlondikeMove::ColumnToColumn {
                source: 0,
                destination: 1,
                count: 1,
            },
        ] {
            let transition = transitions.iter().find(|t| t.the_move == m).unwrap();
            assert_eq!(revealed_hidden_delta(transition, &board), 0);
            assert_ne!(successor_order_key(transition, &board).0, 0);
        }
    }

    #[test]
    fn foundation_moves_preserve_critical_path_priority_without_a_reveal() {
        let mut board = KlondikeBoard::new();
        board.foundation[Suit::Diamond as usize] = 4;
        board.foundation[Suit::Heart as usize] = 4;
        board.columns[0].push(card(Suit::Club, 9), true);
        board.columns[0].push(card(Suit::Club, 6), false);
        board.columns[0].push(card(Suit::Diamond, 5), false);
        board.columns[1].push(card(Suit::Club, 1), true);
        board.columns[1].push(card(Suit::Spade, 6), false);
        board.columns[1].push(card(Suit::Heart, 5), false);
        board.compute_signature();

        for draw in [1, 3] {
            let transitions = KlondikeSolver::new(draw).successors(&board, None, 0);
            assert_eq!(transitions.len(), 2);
            assert_eq!(
                transitions[0].the_move,
                KlondikeMove::ColumnToFoundation {
                    source: 1,
                    card: card(Suit::Heart, 5),
                }
            );
            for transition in &transitions {
                assert!(!is_safe_foundation_move(transition, &board));
                assert_eq!(revealed_hidden_delta(transition, &board), 0);
                assert_eq!(thoughtful_reveal_value(transition, &board), 0);
            }
        }
    }

    #[test]
    fn foundation_moves_prioritize_revealing_the_next_foundation_card() {
        let mut board = KlondikeBoard::new();
        board.foundation[Suit::Diamond as usize] = 4;
        board.foundation[Suit::Heart as usize] = 4;
        board.columns[0].push(card(Suit::Club, 9), true);
        board.columns[0].push(card(Suit::Diamond, 5), false);
        board.columns[1].push(card(Suit::Heart, 6), true);
        board.columns[1].push(card(Suit::Heart, 5), false);
        board.compute_signature();

        for draw in [1, 3] {
            let transitions = KlondikeSolver::new(draw).successors(&board, None, 0);
            assert_eq!(transitions.len(), 2);
            assert_eq!(
                transitions[0].the_move,
                KlondikeMove::ColumnToFoundation {
                    source: 1,
                    card: card(Suit::Heart, 5),
                }
            );
            for transition in &transitions {
                assert!(!is_safe_foundation_move(transition, &board));
                assert_eq!(revealed_hidden_delta(transition, &board), 1);
                assert_eq!(critical_path_bonus(transition, &board), 0);
            }
        }
    }

    #[test]
    fn safe_foundation_fast_path_preserves_its_order_and_candidates() {
        let mut board = KlondikeBoard::new();
        board.columns[0].push(card(Suit::Spade, 1), false);
        board.columns[1].push(card(Suit::Heart, 9), true);
        board.columns[1].push(card(Suit::Club, 1), false);
        board.stock[0] = card(Suit::Diamond, 13);
        board.stock_len = 1;
        board.compute_signature();

        for draw in [1, 3] {
            let transitions = KlondikeSolver::new(draw).successors(&board, None, 0);
            let moves: Vec<_> = transitions.iter().map(|t| t.the_move).collect();
            assert_eq!(
                moves,
                vec![
                    KlondikeMove::ColumnToFoundation {
                        source: 0,
                        card: card(Suit::Spade, 1),
                    },
                    KlondikeMove::ColumnToFoundation {
                        source: 1,
                        card: card(Suit::Club, 1),
                    },
                ]
            );
        }
    }
}
