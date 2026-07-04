//! FreeCell solver weights and thresholds.
//! Centralizes heuristic, progress, local-priority, and search-limit constants.

// ── Search limits ────────────────────────

/// Maximum expanded nodes before stopping A* search.
pub const MAX_NODES: usize = 1_000_000; // Above 3_000_000 reaches the theoretical 98-99% high, but trades speed for quality.

/// Maximum search depth (hardcoded).
pub const MAX_DEPTH: usize = 200;

// ── One-step Lookahead ───────────────────────────

/// Weight for immediate foundation moves.
pub const LOOKAHEAD_FOUNDATION_OPTIONS_WEIGHT: i64 = 90;
/// Weight for empty columns.
pub const LOOKAHEAD_EMPTY_COLUMNS_WEIGHT: i64 = 25;
/// Penalty for occupied free cells.
pub const LOOKAHEAD_FREE_USED_PENALTY: i64 = 12;
/// Penalty for blocked low cards.
pub const LOOKAHEAD_LOW_BLOCKED_PENALTY: i64 = 8;

// ── Local Priority ──────────────────────────────

/// Base priority for moves to foundation.
pub const PRIORITY_BASE_TO_FOUNDATION: i64 = 900;
/// Base priority for moving from free cell to tableau.
pub const PRIORITY_BASE_FREE_TO_TABLEAU: i64 = 260;
/// Base priority for moves between tableau columns.
pub const PRIORITY_BASE_TABLEAU_TO_TABLEAU: i64 = 220;
/// Base priority for moving from tableau to free cell.
pub const PRIORITY_BASE_TABLEAU_TO_FREE: i64 = -60;
/// Base priority for rollback moves from foundation.
pub const PRIORITY_BASE_FROM_FOUNDATION: i64 = -250;
/// Base priority for deal moves (not used in normal play).
pub const PRIORITY_BASE_DEAL: i64 = -1000;

/// Weight for net foundation card change.
pub const PRIORITY_FOUNDATION_GAIN_WEIGHT: i64 = 1200;
/// Weight for change in immediate foundation options.
pub const PRIORITY_OPTIONS_GAIN_WEIGHT: i64 = 180;
/// Weight for gaining empty columns.
pub const PRIORITY_EMPTY_COL_GAIN_WEIGHT: i64 = 220;
/// Weight for freeing occupied free cells.
pub const PRIORITY_FREE_FREED_WEIGHT: i64 = 100;
/// Weight for improving maximum run length.
pub const PRIORITY_RUN_GAIN_WEIGHT: i64 = 20;
/// Weight for unblocking low cards (A,2,3).
pub const PRIORITY_LOW_UNBLOCK_WEIGHT: i64 = 160;

// ── Heuristic Cost ───────────────────────────────

/// Weight for cards missing from foundation (52 - foundation_count).
pub const HEURISTIC_FOUNDATION_REMAINING_WEIGHT: i64 = 120;
/// Penalty for occupied free cells.
pub const HEURISTIC_FREE_USED_PENALTY: i64 = 14;
/// Penalty for blocked low cards.
pub const HEURISTIC_LOW_BLOCKED_PENALTY: i64 = 18;
/// Penalty for imbalance between suit foundations.
pub const HEURISTIC_FOUNDATION_IMBALANCE_PENALTY: i64 = 12;
/// Bonus for empty tableau columns.
pub const HEURISTIC_EMPTY_TABLEAU_BONUS: i64 = 10;
/// Bonus for an already formed long run in tableau.
pub const HEURISTIC_LONGEST_RUN_BONUS: i64 = 3;
/// Bonus for immediately available moves to foundation.
pub const HEURISTIC_IMMEDIATE_FOUNDATION_BONUS: i64 = 30;

// ── Progress Score ───────────────────────────────

/// Main reward for irreversible foundation progress.
pub const PROGRESS_FOUNDATION_WEIGHT: i64 = 2500;
/// Reward for immediate foundation options.
pub const PROGRESS_IMMEDIATE_FOUNDATION_WEIGHT: i64 = 120;
/// Mobility bonus from empty columns.
pub const PROGRESS_EMPTY_TABLEAU_BONUS: i64 = 90;
/// Bonus for long-run structure in tableau.
pub const PROGRESS_LONGEST_RUN_BONUS: i64 = 25;
/// Penalty for free-cell saturation.
pub const PROGRESS_FREE_USED_PENALTY: i64 = 35;
/// Penalty for blocking key low cards.
pub const PROGRESS_LOW_BLOCKED_PENALTY: i64 = 22;

// ── Endgame / expansion ──────────────────────────

/// From this foundation count onward, unsafe foundation moves are allowed.
pub const ENDGAME_UNSAFE_FOUNDATION_THRESHOLD: usize = 40;
