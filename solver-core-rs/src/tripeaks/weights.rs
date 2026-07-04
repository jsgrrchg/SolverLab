//! Pesos y umbrales del solver de TriPeaks.
//! Centraliza constantes de heurística, progreso, prioridad local y checkpoints.

// ── Solver ──────────────────────────────────────

/// Profundidad máxima de búsqueda
pub const MAX_DEPTH: usize = 100;

/// Timeout de búsqueda en segundos (corte de seguridad)
pub const TIMEOUT_SECS: f64 = 5.0;

// ── Prioridad Local ─────────────────────────────

/// Multiplicador para priorizar transiciones que remueven cartas de tableau.
pub const LOCAL_PRIORITY_TABLEAU_REMOVAL_WEIGHT: i64 = 10;

// ── Heuristic Cost ──────────────────────────────

/// Peso para cartas restantes en tableau (heurística A*).
pub const HEURISTIC_REMAINING_TABLEAU_WEIGHT: i64 = 1;

// ── Progress Score ──────────────────────────────

/// Peso para cartas removidas de tableau en el puntaje de progreso.
pub const PROGRESS_REMOVED_TABLEAU_WEIGHT: i64 = 1;

// ── Límites de búsqueda ─────────────────────────

/// Máximo de nodos expandidos antes de detener la búsqueda A*.
pub const MAX_NODES: usize = 300_000;
