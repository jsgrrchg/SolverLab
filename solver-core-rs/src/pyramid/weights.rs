//! Pyramid solver weights and thresholds.
//! Centralizes heuristic, progress, tiebreaker, and search-limit constants.

// ── Search limits ──────────────────────────

/// Maximum expanded nodes before stopping A* search.
pub const MAX_NODES: usize = 300_000;

// ── Heuristic Cost ──────────────────────────────

/// Penalty applied when there is no stock flow and no removal option.
pub const DEAD_END_PENALTY: i64 = 160;

/// Target pair-option count before the scarcity penalty reaches zero.
pub const HEURISTIC_SCARCE_PAIR_TARGET: i64 = 2;
/// Weight for the pair-option scarcity penalty.
pub const HEURISTIC_SCARCE_PAIR_WEIGHT: i64 = 14;
/// Target exposed-King count before the King-pressure penalty reaches zero.
pub const HEURISTIC_KING_PRESSURE_TARGET: i64 = 2;
/// Weight for the King-pressure penalty.
pub const HEURISTIC_KING_PRESSURE_WEIGHT: i64 = 10;
/// Weight for remaining pyramid cards.
pub const HEURISTIC_REMAINING_PYRAMID_WEIGHT: i64 = 13;
/// Weight for blocked cards.
pub const HEURISTIC_BLOCKED_WEIGHT: i64 = 14;
/// Weight for deeply blocked cards.
pub const HEURISTIC_DEEP_BLOCKED_WEIGHT: i64 = 18;
/// Weight for stock size.
pub const HEURISTIC_STOCK_WEIGHT: i64 = 2;
/// Weight for waste size.
pub const HEURISTIC_WASTE_WEIGHT: i64 = 1;
/// Subtracted bonus for waste-pyramid pair opportunities (capped at max 2).
pub const HEURISTIC_WASTE_PAIR_BONUS: i64 = 5;
/// Maximum waste pairs that apply the heuristic bonus.
pub const HEURISTIC_WASTE_PAIR_BONUS_CAP: i64 = 2;
/// Weight for exposed cards whose complement is buried (not exposed, in waste, or in stock).
pub const HEURISTIC_UNREACHABLE_WEIGHT: i64 = 12;

// ── Progress Score ──────────────────────────────

/// Total cards in a standard deck.
pub const PROGRESS_DECK_SIZE: i64 = 52;
/// Weight for removed cards.
pub const PROGRESS_REMOVED_WEIGHT: i64 = 120;
/// Weight for exposed pyramid cards.
pub const PROGRESS_EXPOSED_WEIGHT: i64 = 18;
/// Weight for exposed Kings.
pub const PROGRESS_EXPOSED_KING_WEIGHT: i64 = 24;
/// Weight for total pair options.
pub const PROGRESS_PAIR_OPTIONS_WEIGHT: i64 = 30;
/// Weight for waste-pyramid pair options.
pub const PROGRESS_WASTE_PAIR_OPTIONS_WEIGHT: i64 = 12;
/// Penalty for blocked cards.
pub const PROGRESS_BLOCKED_PENALTY: i64 = 14;
/// Penalty for deeply blocked cards.
pub const PROGRESS_DEEP_BLOCKED_PENALTY: i64 = 24;
/// Multiplier for dead-end penalty in progress score.
pub const PROGRESS_DEAD_END_MULTIPLIER: i64 = 1;

// ── Unlock Score (tiebreaker) ───────────

/// Weight for exposed cards in the tiebreaker score.
pub const UNLOCK_EXPOSED_WEIGHT: i64 = 8;
/// Weight for exposed Kings in the tiebreaker score.
pub const UNLOCK_EXPOSED_KING_WEIGHT: i64 = 14;
/// Weight for pair options in the tiebreaker score.
pub const UNLOCK_PAIR_OPTIONS_WEIGHT: i64 = 18;
/// Weight for waste-pyramid pair options in the tiebreaker score.
pub const UNLOCK_WASTE_PAIR_OPTIONS_WEIGHT: i64 = 10;
/// Penalty for blocked cards in the tiebreaker score.
pub const UNLOCK_BLOCKED_PENALTY: i64 = 5;
/// Penalty for deeply blocked cards in the tiebreaker score.
pub const UNLOCK_DEEP_BLOCKED_PENALTY: i64 = 10;
