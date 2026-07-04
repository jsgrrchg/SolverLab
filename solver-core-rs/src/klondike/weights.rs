//! Klondike solver weights and thresholds.
//! Centralized to simplify tuning and avoid scattered literals.

// ── Heuristic Cost ──────────────────────────────

/// Divisor for burial depth bonus, formula: cost += fd + (fd - 1) * MULTIPLIER.
pub const HIDDEN_DEPTH_DIVISOR: i64 = 2;

/// Empty-column bonus with diminishing returns (index = min(empty_cols, 4)).
pub const EMPTY_COL_BONUS: [i64; 5] = [0, 3, 5, 6, 6];

/// Penalty for a blocked King (no empty column + hidden cards below).
pub const BLOCKED_KING_PENALTY: i64 = 3;

/// Remaining-stock threshold for enabling waste penalty.
pub const WASTE_THRESHOLD_STOCK: i64 = 6;

/// Minimum waste threshold for enabling penalty.
pub const WASTE_THRESHOLD_COUNT: i64 = 10;

/// Waste scaling divisor once the penalty is enabled.
pub const WASTE_SCALE_DIVISOR: i64 = 3;

/// Penalty divisor when the stock is completely empty.
pub const WASTE_EMPTY_STOCK_DIVISOR: i64 = 4;

/// Foundation imbalance tolerance before applying a penalty.
pub const IMBALANCE_TOLERANCE: i64 = 2;

/// Multiplier weight for foundation imbalance.
pub const IMBALANCE_WEIGHT: i64 = 4;

/// Divisor for stock advance cost in heuristic_cost (endgame path: hidden==0).
/// Conservative (/2) to avoid overestimation (real cost = stock_remaining).
pub const STOCK_ADVANCE_DIVISOR: i64 = 2;

// ── Thoughtful Heuristics ──────────────────────
// These signals are NOT used in heuristic_cost (admissibility),
// only in progress_score and successor ordering.

/// Multiplier penalty if a "target" card is face-down.
/// Only used in progress_score via PROGRESS_BURIAL_WEIGHT.
pub const BURIED_TARGET_PENALTY: i64 = 2;

/// Penalty for a logical face-down deadlock, weighted by severity.
/// Only used in progress_score via PROGRESS_DEADLOCK_WEIGHT.
pub const DEADLOCK_PENALTY: i64 = 1;

/// Ordering bonus when a move reveals the exact target card.
/// Only affects exploration order, not IDA* admissibility.
pub const THOUGHTFUL_REVEAL_TARGET: i64 = 400;

/// Ordering bonus when a move reveals a King with an empty column available.
pub const THOUGHTFUL_REVEAL_KING: i64 = 200;

/// Ordering bonus when a move reveals a card near the target (+1 or +2).
pub const THOUGHTFUL_REVEAL_NEAR_TARGET: i64 = 150;

// ── Thoughtful Deep Analysis ────────────────────
// Signals that use knowledge of ALL hidden cards.
// Only used in progress_score and successor ordering.

/// Penalty for a blocker with no visible destination above a target.
/// A target with "stranded" blockers is much harder to uncover than one
/// with blockers that have clear destinations.
pub const PROGRESS_STRANDED_BLOCKER_PENALTY: i64 = 200;

/// Penalty for a target in waste (already passed, requires recycle to access).
pub const STOCK_WASTE_TARGET_PENALTY: i64 = 150;

/// Penalty for a target in stock that is not aligned with draw_advance.
/// With draw-3, only 1/3 of cards are accessible per pass.
/// A misaligned target requires a full recycle.
pub const STOCK_MISALIGNED_TARGET_PENALTY: i64 = 200;

/// Ordering bonus for moves that remove cards from a column containing a
/// buried target (a "critical path" move).
pub const CRITICAL_PATH_MOVE_BONUS: i64 = 250;

// ── Progress Score ──────────────────────────────

/// Weight of each foundation card in progress_score.
pub const PROGRESS_FOUNDATION_WEIGHT: i64 = 1300;

/// Weight of each face-up card in columns.
pub const PROGRESS_FACE_UP_WEIGHT: i64 = 110;

/// Penalty for each hidden card in columns.
pub const PROGRESS_HIDDEN_PENALTY: i64 = 300;

/// Empty-column bonus in progress_score.
pub const PROGRESS_EMPTY_COL_BONUS: i64 = 200;

/// Blocked-King penalty in progress_score.
pub const PROGRESS_BLOCKED_KINGS_PENALTY: i64 = 250;

/// waste_penalty multiplier in progress_score.
pub const PROGRESS_WASTE_MULTIPLIER: i64 = 5;

/// progress_score bonus for revealing a key King with an empty column.
pub const REVEALED_KING_BONUS: i64 = 400;

/// progress_score bonus for revealing the next target card of a suit.
pub const REVEALED_FOUNDATION_CARD_BONUS: i64 = 600;

// ── Successor Ordering ──────────────────────────

/// Multiplier for the bonus from exposing hidden cards.
pub const EXPOSED_BONUS_MULTIPLIER: i64 = 300;

/// Thoughtful weight for buried targets in progress_score.
/// No admissibility constraints: penalizes checkpoints that leave targets blocked.
pub const PROGRESS_BURIAL_WEIGHT: i64 = 110;

/// Thoughtful weight for deadlocks in progress_score.
/// No admissibility constraints: penalizes checkpoints that keep active suit inversions.
pub const PROGRESS_DEADLOCK_WEIGHT: i64 = 160;

/// Hidden-card threshold for enabling the near-autoplay bonus in progress_score.
/// When hidden <= this value, an increasing quadratic bonus is applied.
pub const AUTOPLAY_PROXIMITY_THRESHOLD: i64 = 7;

/// Base weight of the near-autoplay bonus: proximity^2 * this value.
/// States with few hidden cards are almost auto-won, so this is a strong incentive.
pub const PROGRESS_AUTOPLAY_PROXIMITY: i64 = 100;

/// Secondary priority for safe foundation moves.
pub const PRIORITY_SAFE_FOUNDATION: i64 = 600; // Restored.

/// Secondary priority for moves to foundation (not safe).
pub const PRIORITY_TO_FOUNDATION: i64 = 500;

/// Secondary priority for moves from foundation.
pub const PRIORITY_FROM_FOUNDATION: i64 = 100;

/// Base secondary priority for stock advance.
pub const PRIORITY_STOCK_ADVANCE: i64 = 120;

/// Secondary priority for stock recycle.
pub const PRIORITY_STOCK_RECYCLE: i64 = 10;

/// Base secondary priority for column moves.
pub const PRIORITY_COLUMN_BASE: i64 = 100;

/// Secondary bonus for each additional card in the moved stack.
pub const PRIORITY_STACK_BONUS: i64 = 20;

/// Secondary bonus for creating an empty column in C2C.
pub const PRIORITY_EMPTY_COL_BONUS: i64 = 300;

/// ── Checkpoint Adoption ─────────────────────────
/// Defines the progress required to adopt a checkpoint.

/// Early phase (fc < 10): adoption-threshold clamps.
pub const ADOPT_EARLY_CLAMP: (i64, i64) = (150, 1500);

/// Mid phase (10 <= fc < 30): adoption-threshold clamps.
pub const ADOPT_MID_CLAMP: (i64, i64) = (100, 1000);

/// Late phase (fc >= 30): adoption-threshold clamps.
pub const ADOPT_LATE_CLAMP: (i64, i64) = (20, 500);

/// fc threshold for considering the position earlygame.
pub const ADOPT_EARLY_THRESHOLD_FC: i64 = 10;

/// fc threshold for considering the position midgame.
pub const ADOPT_MID_THRESHOLD_FC: i64 = 30;

/// Percentage relative to the initial score for calculating the base threshold.
pub const ADOPT_RELATIVE_PCT: i64 = 3;

/// ── Checkpoint Policy / Limits ──────────────────
/// Defines the node limit for each board difficulty.

/// Maximum adopted checkpoints before giving up.
pub const MAX_CHECKPOINTS: u32 = 40;

/// Maximum search depth.
pub const MAX_DEPTH: usize = 200;

/// Maximum allowed undos.
pub const MAX_UNDOS: usize = 20;

/// Search timeout in seconds.
pub const TIMEOUT_SECS: f64 = 100.0;

/// Maximum transposition table size before clearing.
pub const TT_MAX_ENTRIES: usize = 5_000_000;

/// Checkpoint node limit used as the initial fallback.
pub const CHECKPOINT_FALLBACK_LIMIT: u64 = 500_000;

/// Table of base node limits by difficulty (hidden_bucket).
/// Index 0..5: based on hidden-card range. Index 6: endgame (fc > 30).
pub const CHECKPOINT_BASE_LIMITS: [u64; 7] = [
    800_000,   // hidden 0-6 (Basic)
    1_200_000, // hidden 7-9 (Lower-mid)
    1_800_000, // hidden 10–12 (Normal)
    2_200_000, // hidden 13-15 (Upper-mid)
    3_500_000, // hidden 16-18 (Hard)
    8_000_000, // hidden 19+ (Very hard)
    800_000, // endgame: fc > 30, hidden <= 6; matched to base to avoid losing nearly won games
];
