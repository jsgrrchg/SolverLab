//! Pesos y umbrales del solver de Pyramid.
//! Centraliza constantes de heurística, progreso, desempate y límites de búsqueda.

// ── Límites de búsqueda ──────────────────────────

/// Máximo de nodos expandidos antes de detener la búsqueda A*.
pub const MAX_NODES: usize = 300_000;

// ── Heuristic Cost ──────────────────────────────

/// Penalización aplicada cuando no hay flujo de stock ni opciones de remoción.
pub const DEAD_END_PENALTY: i64 = 160;

/// Cantidad objetivo de opciones de pareja antes de que la penalización por escasez sea cero.
pub const HEURISTIC_SCARCE_PAIR_TARGET: i64 = 2;
/// Peso de la penalización por escasez de opciones de pareja.
pub const HEURISTIC_SCARCE_PAIR_WEIGHT: i64 = 14;
/// Cantidad objetivo de reyes expuestos antes de que la penalización de presión de reyes sea cero.
pub const HEURISTIC_KING_PRESSURE_TARGET: i64 = 2;
/// Peso de la penalización de presión de reyes.
pub const HEURISTIC_KING_PRESSURE_WEIGHT: i64 = 10;
/// Peso para cartas restantes en la pirámide.
pub const HEURISTIC_REMAINING_PYRAMID_WEIGHT: i64 = 13;
/// Peso para cartas bloqueadas.
pub const HEURISTIC_BLOCKED_WEIGHT: i64 = 14;
/// Peso para cartas bloqueadas en profundidad.
pub const HEURISTIC_DEEP_BLOCKED_WEIGHT: i64 = 18;
/// Peso para tamaño del stock.
pub const HEURISTIC_STOCK_WEIGHT: i64 = 2;
/// Peso para tamaño del waste.
pub const HEURISTIC_WASTE_WEIGHT: i64 = 1;
/// Bonificación restada por oportunidades de pareja waste-pirámide (capeada a max 2).
pub const HEURISTIC_WASTE_PAIR_BONUS: i64 = 5;
/// Máximo de waste pairs que aplican bonus en la heurística.
pub const HEURISTIC_WASTE_PAIR_BONUS_CAP: i64 = 2;
/// Peso para cartas expuestas cuyo complemento está enterrado (no en expuestas/waste/stock).
pub const HEURISTIC_UNREACHABLE_WEIGHT: i64 = 12;

// ── Progress Score ──────────────────────────────

/// Total de cartas en un mazo estándar.
pub const PROGRESS_DECK_SIZE: i64 = 52;
/// Peso para cartas removidas.
pub const PROGRESS_REMOVED_WEIGHT: i64 = 120;
/// Peso para cartas expuestas de la pirámide.
pub const PROGRESS_EXPOSED_WEIGHT: i64 = 18;
/// Peso para reyes expuestos.
pub const PROGRESS_EXPOSED_KING_WEIGHT: i64 = 24;
/// Peso para opciones totales de pareja.
pub const PROGRESS_PAIR_OPTIONS_WEIGHT: i64 = 30;
/// Peso para opciones de pareja waste-pirámide.
pub const PROGRESS_WASTE_PAIR_OPTIONS_WEIGHT: i64 = 12;
/// Penalización para cartas bloqueadas.
pub const PROGRESS_BLOCKED_PENALTY: i64 = 14;
/// Penalización para cartas bloqueadas en profundidad.
pub const PROGRESS_DEEP_BLOCKED_PENALTY: i64 = 24;
/// Multiplicador de la penalización de callejón sin salida en el puntaje de progreso.
pub const PROGRESS_DEAD_END_MULTIPLIER: i64 = 1;

// ── Puntaje de Desbloqueo (desempate) ───────────

/// Peso para cartas expuestas en el puntaje de desempate.
pub const UNLOCK_EXPOSED_WEIGHT: i64 = 8;
/// Peso para reyes expuestos en el puntaje de desempate.
pub const UNLOCK_EXPOSED_KING_WEIGHT: i64 = 14;
/// Peso para opciones de pareja en el puntaje de desempate.
pub const UNLOCK_PAIR_OPTIONS_WEIGHT: i64 = 18;
/// Peso para opciones de pareja waste-pirámide en el puntaje de desempate.
pub const UNLOCK_WASTE_PAIR_OPTIONS_WEIGHT: i64 = 10;
/// Penalización para cartas bloqueadas en el puntaje de desempate.
pub const UNLOCK_BLOCKED_PENALTY: i64 = 5;
/// Penalización para cartas bloqueadas en profundidad en el puntaje de desempate.
pub const UNLOCK_DEEP_BLOCKED_PENALTY: i64 = 10;
