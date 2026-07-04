//! Pesos y umbrales del solver de FreeCell.
//! Centraliza constantes de heurística, progreso, prioridad local y límites de búsqueda.

// ── Límites de búsqueda ────────────────────────

/// Máximo de nodos expandidos antes de detener la búsqueda A*.
pub const MAX_NODES: usize = 1_000_000; // sobre 3_000_000 se obtiene el alto teórico de 98-99%, sin embargo, se compromete velocidad por calidad.

/// Profundidad máxima de búsqueda (hardcodeado).
pub const MAX_DEPTH: usize = 200;

// ── One-step Lookahead ───────────────────────────

/// Peso por jugadas inmediatas a fundación.
pub const LOOKAHEAD_FOUNDATION_OPTIONS_WEIGHT: i64 = 90;
/// Peso por columnas vacías.
pub const LOOKAHEAD_EMPTY_COLUMNS_WEIGHT: i64 = 25;
/// Penalización por free cells ocupadas.
pub const LOOKAHEAD_FREE_USED_PENALTY: i64 = 12;
/// Penalización por low cards bloqueadas.
pub const LOOKAHEAD_LOW_BLOCKED_PENALTY: i64 = 8;

// ── Prioridad Local ──────────────────────────────

/// Prioridad base para movimientos hacia foundation.
pub const PRIORITY_BASE_TO_FOUNDATION: i64 = 900;
/// Prioridad base para mover desde free cell hacia tableau.
pub const PRIORITY_BASE_FREE_TO_TABLEAU: i64 = 260;
/// Prioridad base para movimientos entre columnas de tableau.
pub const PRIORITY_BASE_TABLEAU_TO_TABLEAU: i64 = 220;
/// Prioridad base para mover de tableau a free cell.
pub const PRIORITY_BASE_TABLEAU_TO_FREE: i64 = -60;
/// Prioridad base para movimientos de retroceso desde foundation.
pub const PRIORITY_BASE_FROM_FOUNDATION: i64 = -250;
/// Prioridad base para movimiento de reparto (no se usa en juego normal).
pub const PRIORITY_BASE_DEAL: i64 = -1000;

/// Peso del cambio neto de cartas en foundation.
pub const PRIORITY_FOUNDATION_GAIN_WEIGHT: i64 = 1200;
/// Peso del cambio en opciones inmediatas de foundation.
pub const PRIORITY_OPTIONS_GAIN_WEIGHT: i64 = 180;
/// Peso de la ganancia de columnas vacías.
pub const PRIORITY_EMPTY_COL_GAIN_WEIGHT: i64 = 220;
/// Peso por liberar free cells ocupadas.
pub const PRIORITY_FREE_FREED_WEIGHT: i64 = 100;
/// Peso de la mejora en longitud de la corrida máxima.
pub const PRIORITY_RUN_GAIN_WEIGHT: i64 = 20;
/// Peso por desbloquear cartas bajas (A,2,3).
pub const PRIORITY_LOW_UNBLOCK_WEIGHT: i64 = 160;

// ── Heuristic Cost ───────────────────────────────

/// Peso por cartas faltantes en foundation (52 - foundation_count).
pub const HEURISTIC_FOUNDATION_REMAINING_WEIGHT: i64 = 120;
/// Penalización por free cells ocupadas.
pub const HEURISTIC_FREE_USED_PENALTY: i64 = 14;
/// Penalización por cartas bajas bloqueadas.
pub const HEURISTIC_LOW_BLOCKED_PENALTY: i64 = 18;
/// Penalización por desbalance entre foundations de palos.
pub const HEURISTIC_FOUNDATION_IMBALANCE_PENALTY: i64 = 12;
/// Bonificación por columnas vacías en tableau.
pub const HEURISTIC_EMPTY_TABLEAU_BONUS: i64 = 10;
/// Bonificación por corrida larga ya formada en tableau.
pub const HEURISTIC_LONGEST_RUN_BONUS: i64 = 3;
/// Bonificación por jugadas inmediatas disponibles hacia foundation.
pub const HEURISTIC_IMMEDIATE_FOUNDATION_BONUS: i64 = 30;

// ── Progress Score ───────────────────────────────

/// Recompensa principal por avance irreversible en foundation.
pub const PROGRESS_FOUNDATION_WEIGHT: i64 = 2500;
/// Recompensa por opciones inmediatas de foundation.
pub const PROGRESS_IMMEDIATE_FOUNDATION_WEIGHT: i64 = 120;
/// Bonificación por movilidad aportada por columnas vacías.
pub const PROGRESS_EMPTY_TABLEAU_BONUS: i64 = 90;
/// Bonificación por estructura de corridas largas en tableau.
pub const PROGRESS_LONGEST_RUN_BONUS: i64 = 25;
/// Penalización por saturación de free cells.
pub const PROGRESS_FREE_USED_PENALTY: i64 = 35;
/// Penalización por bloquear cartas bajas clave.
pub const PROGRESS_LOW_BLOCKED_PENALTY: i64 = 22;

// ── Endgame / expansión ──────────────────────────

/// Desde este conteo de fundación, se permiten jugadas no seguras a fundación.
pub const ENDGAME_UNSAFE_FOUNDATION_THRESHOLD: usize = 40;
