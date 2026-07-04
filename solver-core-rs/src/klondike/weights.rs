//! Sistema de Pesos y Umbrales del solver Klondike.
//! Centralizado para facilitar tuning y evitar literales dispersos.

// ── Heuristic Cost ──────────────────────────────

/// Divisor para el bonus de profundidad de enterramiento, fórmula: cost += fd + (fd - 1) * MULTIPLIER
pub const HIDDEN_DEPTH_DIVISOR: i64 = 2;

/// Bonus por columnas vacías con rendimientos decrecientes (index = min(empty_cols, 4))
pub const EMPTY_COL_BONUS: [i64; 5] = [0, 3, 5, 6, 6];

/// Penalización por king bloqueado (sin columna vacía + ocultas debajo)
pub const BLOCKED_KING_PENALTY: i64 = 3;

/// Umbral de stock restante para activar penalización de waste
pub const WASTE_THRESHOLD_STOCK: i64 = 6;

/// Umbral mínimo de waste para activar penalización
pub const WASTE_THRESHOLD_COUNT: i64 = 10;

/// Divisor del escalado de waste cuando se activa la penalización
pub const WASTE_SCALE_DIVISOR: i64 = 3;

/// Divisor para penalización con stock completamente vacío
pub const WASTE_EMPTY_STOCK_DIVISOR: i64 = 4;

/// Tolerancia de desequilibrio entre foundations antes de penalizar
pub const IMBALANCE_TOLERANCE: i64 = 2;

/// Peso multiplicador del desequilibrio de foundation
pub const IMBALANCE_WEIGHT: i64 = 4;

/// Divisor para el costo de stock advances en heuristic_cost (endgame path: hidden==0).
/// Conservador (/2) para no sobreestimar (costo real = stock_remaining).
pub const STOCK_ADVANCE_DIVISOR: i64 = 2;

// ── Heurísticas Thoughtful ──────────────────────
// Estas señales NO se usan en heuristic_cost (admisibilidad),
// solo en progress_score y successor ordering.

/// Penalización multiplicadora si una carta "Target" se encuentra boca abajo.
/// Solo se usa en progress_score via PROGRESS_BURIAL_WEIGHT.
pub const BURIED_TARGET_PENALTY: i64 = 2;

/// Penalidad por deadlock lógico boca abajo, ponderada por severidad.
/// Solo se usa en progress_score via PROGRESS_DEADLOCK_WEIGHT.
pub const DEADLOCK_PENALTY: i64 = 1;

/// Bonus de ordenamiento cuando un movimiento revela la carta target exacta.
/// Solo afecta al orden de exploración, no a la admisibilidad de IDA*.
pub const THOUGHTFUL_REVEAL_TARGET: i64 = 400;

/// Bonus de ordenamiento cuando un movimiento revela un Rey con columna vacía disponible.
pub const THOUGHTFUL_REVEAL_KING: i64 = 200;

/// Bonus de ordenamiento cuando un movimiento revela una carta cercana al target (+1 o +2).
pub const THOUGHTFUL_REVEAL_NEAR_TARGET: i64 = 150;

// ── Thoughtful Deep Analysis ────────────────────
// Señales que explotan el conocimiento de TODAS las cartas ocultas.
// Solo se usan en progress_score y successor ordering.

/// Penalización por bloqueador sin destino visible encima de un target.
/// Un target con blockers "varados" es mucho más difícil de desenterrar
/// que uno con blockers que tienen destinos claros.
pub const PROGRESS_STRANDED_BLOCKER_PENALTY: i64 = 200;

/// Penalización por target en waste (ya pasó, necesita recycle para acceder).
pub const STOCK_WASTE_TARGET_PENALTY: i64 = 150;

/// Penalización por target en stock no alineado con draw_advance.
/// Con draw-3, solo 1/3 de las cartas son accesibles por pass.
/// Un target misaligned necesita recycle completo.
pub const STOCK_MISALIGNED_TARGET_PENALTY: i64 = 200;

/// Bonus de ordenamiento para movimientos que sacan cartas de una columna
/// que tiene un target enterrado (movimiento en el "critical path").
pub const CRITICAL_PATH_MOVE_BONUS: i64 = 250;

// ── Progress Score ──────────────────────────────

/// Peso de cada carta en foundation para progress_score
pub const PROGRESS_FOUNDATION_WEIGHT: i64 = 1300;

/// Peso de cada carta boca arriba en columnas
pub const PROGRESS_FACE_UP_WEIGHT: i64 = 110;

/// Penalización de cada carta oculta en columnas
pub const PROGRESS_HIDDEN_PENALTY: i64 = 300;

/// Bonus por columna vacía en progress_score
pub const PROGRESS_EMPTY_COL_BONUS: i64 = 200;

/// Penalización por king bloqueado en progress_score
pub const PROGRESS_BLOCKED_KINGS_PENALTY: i64 = 250;

/// Multiplicador de waste_penalty en progress_score
pub const PROGRESS_WASTE_MULTIPLIER: i64 = 5;

/// Bonus en progress_score por revelar una carta clave Rey con columna vacía.
pub const REVEALED_KING_BONUS: i64 = 400;

/// Bonus en progress_score por revelar la carta target siguiente de una pinta.
pub const REVEALED_FOUNDATION_CARD_BONUS: i64 = 600;

// ── Successor Ordering ──────────────────────────

/// Multiplicador del bonus por exponer cartas ocultas
pub const EXPOSED_BONUS_MULTIPLIER: i64 = 300;

/// Peso thoughtful de targets enterrados en progress_score.
/// Sin restricciones de admisibilidad: penaliza checkpoints que dejan targets bloqueados.
pub const PROGRESS_BURIAL_WEIGHT: i64 = 110;

/// Peso thoughtful de deadlocks en progress_score.
/// Sin restricciones de admisibilidad: penaliza checkpoints que mantienen
/// inversiones de pinta activas.
pub const PROGRESS_DEADLOCK_WEIGHT: i64 = 160;

/// Umbral de cartas ocultas para activar bonus near-autoplay en progress_score.
/// Cuando hidden <= este valor, se aplica bonus cuadrático creciente.
pub const AUTOPLAY_PROXIMITY_THRESHOLD: i64 = 7;

/// Peso base del bonus near-autoplay: proximity^2 * este valor.
/// Estados con pocas cartas ocultas son casi auto-ganados → fuerte incentivo.
pub const PROGRESS_AUTOPLAY_PROXIMITY: i64 = 100;

/// Prioridad secundaria para movimientos de foundation segura
pub const PRIORITY_SAFE_FOUNDATION: i64 = 600; // Restaurado

/// Prioridad secundaria para movimientos a foundation (no seguros)
pub const PRIORITY_TO_FOUNDATION: i64 = 500;

/// Prioridad secundaria para movimientos desde foundation
pub const PRIORITY_FROM_FOUNDATION: i64 = 100;

/// Prioridad secundaria base para stock advance
pub const PRIORITY_STOCK_ADVANCE: i64 = 120;

/// Prioridad secundaria para stock recycle
pub const PRIORITY_STOCK_RECYCLE: i64 = 10;

/// Prioridad secundaria base para movimientos de columna
pub const PRIORITY_COLUMN_BASE: i64 = 100;

/// Bonus secundario por cada carta adicional en stack movido
pub const PRIORITY_STACK_BONUS: i64 = 20;

/// Bonus secundario por crear columna vacía en C2C
pub const PRIORITY_EMPTY_COL_BONUS: i64 = 300;

/// ── Checkpoint Adoption ─────────────────────────
/// Define el progreso necesario para adoptar un checkpoint

/// Fase temprana (fc < 10): clamps para umbral de adopción
pub const ADOPT_EARLY_CLAMP: (i64, i64) = (150, 1500);

/// Fase media (10 <= fc < 30): clamps para umbral de adopción
pub const ADOPT_MID_CLAMP: (i64, i64) = (100, 1000);

/// Fase tardía (fc >= 30): clamps para umbral de adopción
pub const ADOPT_LATE_CLAMP: (i64, i64) = (20, 500);

/// Umbral de fc para considerar earlygame
pub const ADOPT_EARLY_THRESHOLD_FC: i64 = 10;

/// Umbral de fc para considerar midgame
pub const ADOPT_MID_THRESHOLD_FC: i64 = 30;

/// Porcentaje relativo al score inicial para calcular umbral base
pub const ADOPT_RELATIVE_PCT: i64 = 3;

/// ── Checkpoint Policy / Limits ──────────────────
/// Define el límite de nodos para cada dificultad de tablero.

/// Máximo de checkpoints adoptados antes de abandonar
pub const MAX_CHECKPOINTS: u32 = 40;

/// Profundidad máxima de búsqueda
pub const MAX_DEPTH: usize = 200;

/// Máximo de undos permitidos
pub const MAX_UNDOS: usize = 20;

/// Timeout de búsqueda en segundos
pub const TIMEOUT_SECS: f64 = 100.0;

/// Tamaño máximo de la transposition table antes de limpiar
pub const TT_MAX_ENTRIES: usize = 5_000_000;

/// Límite de nodos de checkpoint como fallback inicial
pub const CHECKPOINT_FALLBACK_LIMIT: u64 = 500_000;

/// Tabla de límites base de nodos por dificultad (hidden_bucket)
/// Índice 0..5: basado en rango de hidden cards.  Índice 6: endgame (fc > 30).
pub const CHECKPOINT_BASE_LIMITS: [u64; 7] = [
    800_000,   // hidden 0–6 (Básico)
    1_200_000, // hidden 7–9 (Medio-bajo)
    1_800_000, // hidden 10–12 (Normal)
    2_200_000, // hidden 13–15 (Medio-alto)
    3_500_000, // hidden 16–18 (Difícil)
    8_000_000, // hidden 19+ (Muy difícil)
    800_000, // endgame: fc > 30, hidden <= 6 — igualado al base para no perder juegos casi ganados
];
