use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use super::board::SpiderBoard;
use super::engine::SpiderEngine;
use super::moves::SpiderMove;
use super::weights;

/// Solver DFS Chunked con Checkpoints y Multi-Attempt para Spider Solitaire.
///
/// Cada "chunk" es un DFS a profundidad fija que busca la mejor mejora
/// incremental. Los chunks se encadenan mediante checkpoints adoptados.
/// Multi-attempt con perturbación determinista del ordenamiento de movimientos
/// proporciona diversidad entre intentos.
pub struct SpiderSolver {
    engine: SpiderEngine,
}

/// Estadísticas del proceso de resolución.
pub struct SpiderSolveStats {
    pub moves: Option<Vec<SpiderMove>>,
    pub checkpoints_adopted: u32,
}

/// Resultado de un chunk individual de DFS.
enum ChunkResult {
    Solution(Vec<SpiderMove>),
    BestProgress {
        board: SpiderBoard,
        path: Vec<SpiderMove>,
    },
    Exhausted,
}

/// Contexto mutable del DFS recursivo.
struct DfsContext {
    nodes: u64,
    node_limit: u64,
    best_score: i64,
    best_board: Option<SpiderBoard>,
    best_path: Vec<SpiderMove>,
    solution: Option<Vec<SpiderMove>>,
}

// ── Solver Multi-Attempt con DFS Chunked ────────────────────────────

impl SpiderSolver {
    pub fn new(suit_count: u32) -> SpiderSolver {
        SpiderSolver {
            engine: SpiderEngine::new(suit_count),
        }
    }

    /// Acceso rápido al suit_count del engine.
    fn suit_count(&self) -> u32 {
        self.engine.suit_count
    }

    pub fn solve(&self, board: &SpiderBoard, allow_partial: bool) -> Option<Vec<SpiderMove>> {
        self.solve_with_stats(board, allow_partial).moves
    }

    /// Punto de entrada principal: ejecuta múltiples intentos con perturbación
    /// determinista del ordenamiento de movimientos.
    /// Solver determinista: mismos inputs → mismos outputs, siempre.
    pub fn solve_with_stats(&self, board: &SpiderBoard, allow_partial: bool) -> SpiderSolveStats {
        if SpiderEngine::is_win(board) {
            return SpiderSolveStats {
                moves: Some(vec![]),
                checkpoints_adopted: 0,
            };
        }

        let sc = self.suit_count();
        let deadline = Instant::now() + Duration::from_secs_f64(weights::timeout_secs(sc));
        let mut best_partial: Option<(Vec<SpiderMove>, u32, i64)> = None;

        for attempt in 0..weights::max_attempts(sc) {
            if Instant::now() >= deadline {
                break;
            }
            let result = self.solve_attempt(board, attempt as u64);

            match &result.moves {
                Some(moves) => {
                    if let Some(final_board) = replay_moves(board, moves) {
                        if SpiderEngine::is_win(&final_board) {
                            return result;
                        }
                    }
                    if allow_partial {
                        let final_score = replay_moves(board, moves)
                            .map(|b| progress_score(&b, sc))
                            .unwrap_or(i64::MIN);
                        let is_better = match &best_partial {
                            None => true,
                            Some((_, _, prev_score)) => final_score > *prev_score,
                        };
                        if is_better {
                            best_partial =
                                Some((moves.clone(), result.checkpoints_adopted, final_score));
                        }
                    }
                }
                None => {}
            }
        }

        match best_partial {
            Some((path, checkpoints, _)) => SpiderSolveStats {
                moves: Some(path),
                checkpoints_adopted: checkpoints,
            },
            None => SpiderSolveStats {
                moves: None,
                checkpoints_adopted: 0,
            },
        }
    }

    /// Un intento individual con perturbación específica.
    fn solve_attempt(&self, board: &SpiderBoard, attempt: u64) -> SpiderSolveStats {
        let sc = self.suit_count();
        let mut unified_path: Vec<SpiderMove> = Vec::new();
        let mut current_board = board.clone();
        let mut adopted_sigs: HashSet<u64> = HashSet::new();
        adopted_sigs.insert(current_board.signature);
        let mut checkpoints_adopted: u32 = 0;

        let deal_eager = attempt as u32 >= weights::deal_eager_start(sc);
        let mut checkpoints_since_deal: u32 = 0;
        let mut stall_count: u32 = 0;
        let mut tt: HashMap<u64, usize> = HashMap::new();

        for _chunk in 0..weights::max_checkpoints(sc) {
            // Deal-eager: repartir proactivamente cada N checkpoints.
            if deal_eager
                && checkpoints_since_deal >= weights::deal_eager_frequency(sc)
                && current_board.can_deal_from_stock()
            {
                if let Some(new_board) = SpiderMove::DealFromStock.apply(&current_board) {
                    if !adopted_sigs.contains(&new_board.signature) {
                        adopted_sigs.insert(new_board.signature);
                        unified_path.push(SpiderMove::DealFromStock);
                        current_board = new_board;
                        checkpoints_adopted += 1;
                        checkpoints_since_deal = 0;
                        continue;
                    }
                }
            }

            let start_progress = progress_score(&current_board, sc);

            let chunk_result = self.dfs_chunk(&current_board, attempt, &mut tt);

            let mut adopted = false;
            match chunk_result {
                ChunkResult::Solution(path) => {
                    unified_path.extend(path);
                    return SpiderSolveStats {
                        moves: Some(unified_path),
                        checkpoints_adopted,
                    };
                }
                ChunkResult::BestProgress {
                    board: best_board,
                    path,
                } => {
                    let best_score = progress_score(&best_board, sc);
                    if !adopted_sigs.contains(&best_board.signature)
                        && is_adoptable(start_progress, best_score, sc)
                    {
                        adopted_sigs.insert(best_board.signature);
                        unified_path.extend(path);
                        current_board = best_board;
                        checkpoints_adopted += 1;
                        adopted = true;
                        stall_count = 0;
                        checkpoints_since_deal += 1;
                    }
                }
                ChunkResult::Exhausted => {}
            }

            if adopted {
                continue;
            }

            stall_count += 1;

            // Fallback: intentar repartir del stock.
            if let Some(new_board) = SpiderMove::DealFromStock.apply(&current_board) {
                if !adopted_sigs.contains(&new_board.signature) {
                    adopted_sigs.insert(new_board.signature);
                    unified_path.push(SpiderMove::DealFromStock);
                    current_board = new_board;
                    checkpoints_adopted += 1;
                    stall_count = 0;
                    checkpoints_since_deal = 0;
                    continue;
                }
            }

            if stall_count >= 3 {
                break;
            }
        }

        if !unified_path.is_empty() {
            SpiderSolveStats {
                moves: Some(unified_path),
                checkpoints_adopted,
            }
        } else {
            SpiderSolveStats {
                moves: None,
                checkpoints_adopted,
            }
        }
    }

    /// Chunk DFS: un solo DFS a profundidad fija. Simple y rápido.
    fn dfs_chunk(
        &self,
        board: &SpiderBoard,
        attempt: u64,
        tt: &mut HashMap<u64, usize>,
    ) -> ChunkResult {
        let sc = self.suit_count();
        let initial_score = progress_score(board, sc);

        let mut ctx = DfsContext {
            nodes: 0,
            node_limit: weights::scaled_chunk_budget(sc, attempt as u32),
            best_score: initial_score,
            best_board: None,
            best_path: Vec::new(),
            solution: None,
        };

        let mut path: Vec<SpiderMove> = Vec::new();
        let mut path_sigs: HashSet<u64> = HashSet::new();
        path_sigs.insert(board.signature);

        self.dfs(
            board,
            0,
            weights::DFS_MAX_DEPTH,
            &mut path,
            &mut path_sigs,
            tt,
            &mut ctx,
            attempt,
        );

        if let Some(solution_path) = ctx.solution {
            return ChunkResult::Solution(solution_path);
        }

        if let Some(best_board) = ctx.best_board {
            if ctx.best_score > initial_score {
                ChunkResult::BestProgress {
                    board: best_board,
                    path: ctx.best_path,
                }
            } else {
                ChunkResult::Exhausted
            }
        } else {
            ChunkResult::Exhausted
        }
    }

    /// DFS recursivo con detección de ciclos, TT y tracking de progreso.
    fn dfs(
        &self,
        board: &SpiderBoard,
        depth: usize,
        max_depth: usize,
        path: &mut Vec<SpiderMove>,
        path_sigs: &mut HashSet<u64>,
        tt: &mut HashMap<u64, usize>,
        ctx: &mut DfsContext,
        attempt: u64,
    ) {
        ctx.nodes += 1;

        if ctx.nodes >= ctx.node_limit {
            return;
        }

        if depth >= max_depth {
            return;
        }

        if SpiderEngine::is_win(board) {
            ctx.solution = Some(path.clone());
            return;
        }

        let score = progress_score(board, self.suit_count());
        if score > ctx.best_score {
            ctx.best_score = score;
            ctx.best_board = Some(board.clone());
            ctx.best_path = path.clone();
        }

        let sig = board.signature;
        if let Some(&prev_depth) = tt.get(&sig) {
            if prev_depth <= depth {
                return;
            }
        }
        tt.insert(sig, depth);

        if tt.len() >= weights::tt_max_entries(self.suit_count()) {
            // Eviction parcial: conservar entradas con depth bajo (más poder de poda).
            // Una entrada depth=2 poda visitas a depth>=2; una depth=12 solo poda depth>=12.
            let cutoff = weights::DFS_MAX_DEPTH / 2;
            tt.retain(|_, depth| *depth <= cutoff);
            // Fallback de seguridad: si retain no liberó suficiente, limpiar todo.
            if tt.len() >= weights::tt_max_entries(self.suit_count()) {
                tt.clear();
            }
        }

        let prev_move = path.last();
        let transitions = self.engine.successors(board, prev_move, depth, attempt);

        for transition in transitions {
            if ctx.solution.is_some() || ctx.nodes >= ctx.node_limit {
                return;
            }

            let child_sig = transition.to_board.signature;

            if path_sigs.contains(&child_sig) {
                continue;
            }

            path_sigs.insert(child_sig);
            path.push(transition.the_move);

            self.dfs(
                &transition.to_board,
                depth + 1,
                max_depth,
                path,
                path_sigs,
                tt,
                ctx,
                attempt,
            );

            path.pop();
            path_sigs.remove(&child_sig);
        }
    }
}

// ── Checkpoint helpers ──────────────────────────────────────────────

fn is_adoptable(start_progress: i64, best_progress: i64, suit_count: u32) -> bool {
    let delta = best_progress - start_progress;
    delta >= weights::checkpoint_min_gain(suit_count)
}

fn replay_moves(board: &SpiderBoard, moves: &[SpiderMove]) -> Option<SpiderBoard> {
    let mut current = board.clone();
    for m in moves {
        current = m.apply(&current)?;
    }
    Some(current)
}

// ── Progress score ──────────────────────────────────────────────────

fn progress_score(board: &SpiderBoard, suit_count: u32) -> i64 {
    let columns = &board.columns;
    let completed_bonus = board.completed_sets as i64 * weights::P_COMPLETED_SET_BONUS;

    let face_up_score: i64 =
        columns.iter().map(|c| c.num_face_up() as i64).sum::<i64>() * weights::P_FACE_UP_MULT;
    let hidden_penalty = board.total_face_down() as i64 * weights::P_HIDDEN_PENALTY_MULT;

    let suit_run_bonus: i64 = columns.iter().map(|c| c.longest_run as i64).sum::<i64>()
        * weights::suit_run_mult(suit_count);

    let stock_bonus = (50i64 - board.stock.len() as i64) * weights::P_STOCK_BONUS_MULT;
    let empty_bonus = board.empty_column_count() as i64 * weights::empty_column_bonus(suit_count);

    let mut burial_penalty: i64 = 0;
    for col in columns {
        let fu = &col.face_up;
        let run_len = col.longest_run;
        if run_len < fu.len() {
            let buried_count = fu.len() - run_len;
            for i in 0..buried_count {
                if fu[i].value == 13 {
                    burial_penalty += weights::P_KING_BURIAL_PENALTY;
                }
            }
        }
    }

    let near_empty_bonus = columns
        .iter()
        .filter(|c| !c.is_empty() && !c.has_face_down())
        .count() as i64
        * weights::P_NEAR_EMPTY_BONUS;

    let targets = board.suit_targets();
    let mut target_burial_penalty: i64 = 0;
    for col in columns {
        for (idx, card) in col.face_down.iter().enumerate() {
            if targets
                .iter()
                .any(|t| t.suit == card.suit && t.value == card.value)
            {
                let fd_above = col.face_down.len() - 1 - idx;
                let total_above = fd_above + col.face_up.len();
                target_burial_penalty +=
                    (total_above as i64 + 1) * weights::burial_depth_mult(suit_count);
            }
        }
    }

    let mut deadlock_penalty: i64 = 0;
    for col in columns {
        let fd = &col.face_down;
        if fd.len() < 2 {
            continue;
        }
        for i in 0..fd.len() - 1 {
            if fd[i].suit == fd[i + 1].suit && fd[i].value > fd[i + 1].value {
                deadlock_penalty += weights::deadlock_penalty(suit_count);
            }
        }
    }

    // Nueva señal: penalización por fragmentación de palos en face_up.
    let frag_penalty: i64 = columns
        .iter()
        .map(|c| c.suit_transitions() as i64)
        .sum::<i64>()
        * weights::suit_frag_penalty(suit_count);

    completed_bonus + face_up_score - hidden_penalty
        + suit_run_bonus
        + stock_bonus
        + empty_bonus
        + near_empty_bonus
        - burial_penalty
        - target_burial_penalty
        - deadlock_penalty
        - frag_penalty
}

pub fn progress_token(board: &SpiderBoard) -> String {
    let total_fu: usize = board.columns.iter().map(|c| c.num_face_up()).sum();
    let total_fd = board.total_face_down();
    let suit_run: usize = board.columns.iter().map(|c| c.longest_run).sum();
    let empty = board.empty_column_count();
    format!(
        "c:{}|u:{}|d:{}|r:{}|s:{}|e:{}",
        board.completed_sets,
        total_fu,
        total_fd,
        suit_run,
        board.stock.len(),
        empty
    )
}
// ── Tests ───────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_win_detection() {
        use super::super::column::SpiderColumn;
        let cols = (0..10).map(|_| SpiderColumn::empty()).collect();
        let board = SpiderBoard::new(cols, vec![], 8);
        assert!(SpiderEngine::is_win(&board));
    }

    #[test]
    fn test_is_adoptable_suit1() {
        // suit 1: CHECKPOINT_MIN_GAIN = 65
        assert!(!is_adoptable(1000, 1064, 1));
        assert!(is_adoptable(1000, 1065, 1));
        assert!(is_adoptable(1000, 2000, 1));
    }

    #[test]
    fn test_is_adoptable_suit4() {
        // suit 4: CHECKPOINT_MIN_GAIN = 35
        assert!(!is_adoptable(1000, 1034, 4));
        assert!(is_adoptable(1000, 1035, 4));
        assert!(is_adoptable(1000, 1050, 4));
    }
}
