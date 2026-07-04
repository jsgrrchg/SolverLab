//! TriPeaks solver weights and thresholds.
//! Centralizes heuristic, progress, local-priority, and checkpoint constants.

// ── Solver ──────────────────────────────────────

/// Maximum search depth.
pub const MAX_DEPTH: usize = 100;

/// Search timeout in seconds (safety cutoff).
pub const TIMEOUT_SECS: f64 = 5.0;

// ── Local Priority ─────────────────────────────

/// Multiplier for prioritizing transitions that remove tableau cards.
pub const LOCAL_PRIORITY_TABLEAU_REMOVAL_WEIGHT: i64 = 10;

// ── Heuristic Cost ──────────────────────────────

/// Weight for remaining tableau cards (A* heuristic).
pub const HEURISTIC_REMAINING_TABLEAU_WEIGHT: i64 = 1;

// ── Progress Score ──────────────────────────────

/// Weight for removed tableau cards in progress score.
pub const PROGRESS_REMOVED_TABLEAU_WEIGHT: i64 = 1;

// ── Search limits ─────────────────────────

/// Maximum expanded nodes before stopping A* search.
pub const MAX_NODES: usize = 300_000;
