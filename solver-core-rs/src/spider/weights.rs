//! Spider solver weights and thresholds.
//! Centralizes all numeric constants used by the heuristic, progress score,
//! and checkpoint control.

// ── Chunked DFS with Checkpoints ─────────────────────────────────────

/// Fixed DFS depth per chunk; in practice the global max depth is not needed.
pub const DFS_MAX_DEPTH: usize = 15;

// -- Node budget per chunk --
pub const CHUNK_NODE_BUDGET_1: u64 = 150_000; // Original 300_000; allows 98-99 winrate, but trades quality for speed.
pub const CHUNK_NODE_BUDGET_2: u64 = 300_000; // Original 500_000; 300_000 is giving good results.
pub const CHUNK_NODE_BUDGET_4: u64 = 300_000; // original 1_000_000

pub fn chunk_node_budget(suit_count: u32) -> u64 {
    match suit_count {
        1 => CHUNK_NODE_BUDGET_1,
        2 => CHUNK_NODE_BUDGET_2,
        _ => CHUNK_NODE_BUDGET_4,
    }
}

/// Adaptive budget: scales the budget by attempt number.
/// Early attempts use a reduced budget (discard dead paths quickly);
/// late attempts use an expanded budget (more exploration for difficult games).
pub fn scaled_chunk_budget(suit_count: u32, attempt: u32) -> u64 {
    let base = chunk_node_budget(suit_count);
    let scale = if attempt <= 2 {
        0.4
    } else if attempt <= 9 {
        1.0
    } else {
        1.5
    };
    (base as f64 * scale) as u64
}

// -- Maximum checkpoints --
pub const MAX_CHECKPOINTS_1: u32 = 60; // Uses no more than 10-15.
pub const MAX_CHECKPOINTS_2: u32 = 60; // Uses no more than 20-30.
pub const MAX_CHECKPOINTS_4: u32 = 100;
// The maximum checkpoint count controls memory used to store intermediate states.
pub fn max_checkpoints(suit_count: u32) -> u32 {
    match suit_count {
        1 => MAX_CHECKPOINTS_1,
        2 => MAX_CHECKPOINTS_2,
        _ => MAX_CHECKPOINTS_4,
    }
}

// -- Minimum gain for checkpoint adoption --
pub const CHECKPOINT_MIN_GAIN_1: i64 = 65;
pub const CHECKPOINT_MIN_GAIN_2: i64 = 50;
pub const CHECKPOINT_MIN_GAIN_4: i64 = 35;

pub fn checkpoint_min_gain(suit_count: u32) -> i64 {
    match suit_count {
        1 => CHECKPOINT_MIN_GAIN_1,
        2 => CHECKPOINT_MIN_GAIN_2,
        _ => CHECKPOINT_MIN_GAIN_4,
    }
}

// ── Progress score (progress_score) ────────────────────────────

/// Bonus for a completed set (K-to-A), the dominant signal. Same for all variants.
pub const P_COMPLETED_SET_BONUS: i64 = 5000;
/// Multiplier per face-up card. Same for all variants.
pub const P_FACE_UP_MULT: i64 = 25;
/// Penalty per face-down card. Same for all variants.
pub const P_HIDDEN_PENALTY_MULT: i64 = 40;
/// Multiplier per stock card already dealt. Same for all variants.
pub const P_STOCK_BONUS_MULT: i64 = 8;
/// Penalty for a buried King. Same for all variants.
pub const P_KING_BURIAL_PENALTY: i64 = 60;
/// Bonus for a column with no face-down cards. Same for all variants.
pub const P_NEAR_EMPTY_BONUS: i64 = 70;

// -- Suit run multiplier (by variant) --
// In 1-suit, every sequence is a suit run. In 4-suit, pure runs are gold.
pub const P_SUIT_RUN_MULT_1: i64 = 65;
pub const P_SUIT_RUN_MULT_2: i64 = 120;
pub const P_SUIT_RUN_MULT_4: i64 = 200;
// The multiplier is applied to the consecutive-card run length.
pub fn suit_run_mult(suit_count: u32) -> i64 {
    match suit_count {
        1 => P_SUIT_RUN_MULT_1,
        2 => P_SUIT_RUN_MULT_2,
        _ => P_SUIT_RUN_MULT_4,
    }
}

// -- Empty-column bonus (by variant) --
// Empty columns are temporary storage. In 4-suit they are essential.
pub const P_EMPTY_COLUMN_BONUS_1: i64 = 150;
pub const P_EMPTY_COLUMN_BONUS_2: i64 = 220;
pub const P_EMPTY_COLUMN_BONUS_4: i64 = 350;
// The bonus is applied per empty column, encouraging strategic use.
pub fn empty_column_bonus(suit_count: u32) -> i64 {
    match suit_count {
        1 => P_EMPTY_COLUMN_BONUS_1,
        2 => P_EMPTY_COLUMN_BONUS_2,
        _ => P_EMPTY_COLUMN_BONUS_4,
    }
}

// -- Deadlock penalty (by variant) --
// Deadlocks are more damaging with more suits.
pub const P_DEADLOCK_PENALTY_1: i64 = 40;
pub const P_DEADLOCK_PENALTY_2: i64 = 60;
pub const P_DEADLOCK_PENALTY_4: i64 = 90;

pub fn deadlock_penalty(suit_count: u32) -> i64 {
    match suit_count {
        1 => P_DEADLOCK_PENALTY_1,
        2 => P_DEADLOCK_PENALTY_2,
        _ => P_DEADLOCK_PENALTY_4,
    }
}

// -- Burial depth multiplier (by variant) --
// Uncovering targets is more costly with more mixed suits.
pub const P_BURIAL_DEPTH_MULT_1: i64 = 25;
pub const P_BURIAL_DEPTH_MULT_2: i64 = 35;
pub const P_BURIAL_DEPTH_MULT_4: i64 = 55;

pub fn burial_depth_mult(suit_count: u32) -> i64 {
    match suit_count {
        1 => P_BURIAL_DEPTH_MULT_1,
        2 => P_BURIAL_DEPTH_MULT_2,
        _ => P_BURIAL_DEPTH_MULT_4,
    }
}

// -- Suit fragmentation penalty --
// Penalizes suit transitions in each column's face_up cards.
// In 1-suit this is always 0 (everything is the same suit).
pub const P_SUIT_FRAG_PENALTY_1: i64 = 0;
pub const P_SUIT_FRAG_PENALTY_2: i64 = 20;
pub const P_SUIT_FRAG_PENALTY_4: i64 = 45;
// The penalty is applied per suit transition, encouraging more homogeneous columns.
pub fn suit_frag_penalty(suit_count: u32) -> i64 {
    match suit_count {
        1 => P_SUIT_FRAG_PENALTY_1,
        2 => P_SUIT_FRAG_PENALTY_2,
        _ => P_SUIT_FRAG_PENALTY_4,
    }
}

// ── Ordering: thoughtful signals ────────────────────────────────────

/// Bonus when a move directly reveals a target.
pub const O_TARGET_REVEAL_BONUS: i64 = 500;
/// Bonus when a move reveals a card 1-2 values away from a target.
pub const O_NEAR_TARGET_BONUS: i64 = 200;
/// Bonus when a move reveals a King and an empty column is available.
pub const O_KING_EMPTY_COL_BONUS: i64 = 300;
/// Bonus when a move reduces cards above a buried target.
pub const O_CRITICAL_PATH_BONUS: i64 = 150;

// -- Same-suit affinity bonus (by variant) --
// In 1-suit it does not matter (everything is the same suit). In 4-suit it is critical.
pub const O_SAME_SUIT_AFFINITY_1: i64 = 80;
pub const O_SAME_SUIT_AFFINITY_2: i64 = 200;
pub const O_SAME_SUIT_AFFINITY_4: i64 = 350;

pub fn same_suit_affinity(suit_count: u32) -> i64 {
    match suit_count {
        1 => O_SAME_SUIT_AFFINITY_1,
        2 => O_SAME_SUIT_AFFINITY_2,
        _ => O_SAME_SUIT_AFFINITY_4,
    }
}

// -- Break run penalty (new, by variant) --
// Penalizes moves that break an existing suit run.
pub const O_BREAK_RUN_PENALTY_1: i64 = 0;
pub const O_BREAK_RUN_PENALTY_2: i64 = 100;
pub const O_BREAK_RUN_PENALTY_4: i64 = 200;
// The penalty is applied for each broken suit run, encouraging same-suit runs to be preserved.
pub fn break_run_penalty(suit_count: u32) -> i64 {
    match suit_count {
        1 => O_BREAK_RUN_PENALTY_1,
        2 => O_BREAK_RUN_PENALTY_2,
        _ => O_BREAK_RUN_PENALTY_4,
    }
}

// ── Transposition table ──────────────────────────────────────────

// -- Maximum TT entries (by variant) --
// With a TT persisted across chunks, 2-suit and 4-suit accumulate more entries.
// Variant-specific limits maximize retention without OOM risk.
pub const TT_MAX_ENTRIES_1: usize = 3_000_000; // ~60 MB
pub const TT_MAX_ENTRIES_2: usize = 8_000_000; // ~150 MB
pub const TT_MAX_ENTRIES_4: usize = 12_000_000; // ~230 MB

pub fn tt_max_entries(suit_count: u32) -> usize {
    match suit_count {
        1 => TT_MAX_ENTRIES_1,
        2 => TT_MAX_ENTRIES_2,
        _ => TT_MAX_ENTRIES_4,
    }
}

// ── Global safety timeout ────────────────────────────────────────────

/// Hardcoded global timeout as a safety net.
/// Only acts between attempts; DFS internals and chunks are deterministic
/// (controlled by node_limit). In practice, node budgets finish long before
/// this limit is reached.
pub const TIMEOUT_SECS_1: f64 = 60.0;
pub const TIMEOUT_SECS_2: f64 = 120.0;
pub const TIMEOUT_SECS_4: f64 = 180.0;

pub fn timeout_secs(suit_count: u32) -> f64 {
    match suit_count {
        1 => TIMEOUT_SECS_1,
        2 => TIMEOUT_SECS_2,
        _ => TIMEOUT_SECS_4,
    }
}

// ── Multi-attempt with perturbation ──────────────────────────────────

// -- Max attempts (by variant) --
pub const MAX_ATTEMPTS_1: u32 = 15;
pub const MAX_ATTEMPTS_2: u32 = 20;
pub const MAX_ATTEMPTS_4: u32 = 25;

pub fn max_attempts(suit_count: u32) -> u32 {
    match suit_count {
        1 => MAX_ATTEMPTS_1,
        2 => MAX_ATTEMPTS_2,
        _ => MAX_ATTEMPTS_4,
    }
}

// -- Perturbation range (by variant) --
pub const PERTURBATION_RANGE_1: u64 = 400;
pub const PERTURBATION_RANGE_2: u64 = 500;
pub const PERTURBATION_RANGE_4: u64 = 600;

pub fn perturbation_range(suit_count: u32) -> u64 {
    match suit_count {
        1 => PERTURBATION_RANGE_1,
        2 => PERTURBATION_RANGE_2,
        _ => PERTURBATION_RANGE_4,
    }
}

// -- Deal-eager start (by variant) --
pub const DEAL_EAGER_START_1: u32 = 10;
pub const DEAL_EAGER_START_2: u32 = 8;
pub const DEAL_EAGER_START_4: u32 = 6;

pub fn deal_eager_start(suit_count: u32) -> u32 {
    match suit_count {
        1 => DEAL_EAGER_START_1,
        2 => DEAL_EAGER_START_2,
        _ => DEAL_EAGER_START_4,
    }
}

// -- Deal-eager frequency (by variant) --
pub const DEAL_EAGER_FREQUENCY_1: u32 = 3;
pub const DEAL_EAGER_FREQUENCY_2: u32 = 3;
pub const DEAL_EAGER_FREQUENCY_4: u32 = 2;
// The frequency is applied from the turn defined by deal_eager_start,
// controlling how often a forced deal occurs.
pub fn deal_eager_frequency(suit_count: u32) -> u32 {
    match suit_count {
        1 => DEAL_EAGER_FREQUENCY_1,
        2 => DEAL_EAGER_FREQUENCY_2,
        _ => DEAL_EAGER_FREQUENCY_4,
    }
}

// ── EmptyColumnDiscipline (by variant) ────────────────────────────

// Minimum card count for automatic bypass when moving to an empty column.
pub const EMPTY_COL_MIN_CARDS_1: usize = 3;
pub const EMPTY_COL_MIN_CARDS_2: usize = 3;
pub const EMPTY_COL_MIN_CARDS_4: usize = 5;

pub fn empty_col_min_cards(suit_count: u32) -> usize {
    match suit_count {
        1 => EMPTY_COL_MIN_CARDS_1,
        2 => EMPTY_COL_MIN_CARDS_2,
        _ => EMPTY_COL_MIN_CARDS_4,
    }
}

// Completed-set threshold for considering endgame (relaxes pruning).
pub const EMPTY_COL_ENDGAME_THRESHOLD_1: usize = 2;
pub const EMPTY_COL_ENDGAME_THRESHOLD_2: usize = 2;
pub const EMPTY_COL_ENDGAME_THRESHOLD_4: usize = 3;

pub fn empty_col_endgame_threshold(suit_count: u32) -> usize {
    match suit_count {
        1 => EMPTY_COL_ENDGAME_THRESHOLD_1,
        2 => EMPTY_COL_ENDGAME_THRESHOLD_2,
        _ => EMPTY_COL_ENDGAME_THRESHOLD_4,
    }
}
