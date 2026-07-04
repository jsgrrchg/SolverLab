//! Pesos y umbrales del solver de Spider.
//! Centraliza todas las constantes numéricas usadas en la heurística,
//! el puntaje de progreso y el control de checkpoints.

// ── Chunked DFS con Checkpoints ─────────────────────────────────────

/// Profundidad fija del DFS por chun, en la práctica el max depth global no es necesario.
pub const DFS_MAX_DEPTH: usize = 15;

// -- Presupuesto de nodos por chunk --
pub const CHUNK_NODE_BUDGET_1: u64 = 150_000; // 300_000 original, permite 98-99 winrate, sin embargo se compromete calidad por velocidad.
pub const CHUNK_NODE_BUDGET_2: u64 = 300_000; // original 500_000, 300_000 dando buenos resultados.  .
pub const CHUNK_NODE_BUDGET_4: u64 = 300_000; // original 1_000_000

pub fn chunk_node_budget(suit_count: u32) -> u64 {
    match suit_count {
        1 => CHUNK_NODE_BUDGET_1,
        2 => CHUNK_NODE_BUDGET_2,
        _ => CHUNK_NODE_BUDGET_4,
    }
}

/// Budget adaptativo: escala el presupuesto según el número de intento.
/// Intentos tempranos usan budget reducido (descartar caminos muertos rápido),
/// intentos tardíos usan budget ampliado (más exploración para juegos difíciles).
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

// -- Máximo de checkpoints --
pub const MAX_CHECKPOINTS_1: u32 = 60; // no ocupa más de 10-15
pub const MAX_CHECKPOINTS_2: u32 = 60; // no ocupa más de 20-30
pub const MAX_CHECKPOINTS_4: u32 = 100;
// El máximo de checkpoints controla la memoria usada para almacenar estados intermedios.
pub fn max_checkpoints(suit_count: u32) -> u32 {
    match suit_count {
        1 => MAX_CHECKPOINTS_1,
        2 => MAX_CHECKPOINTS_2,
        _ => MAX_CHECKPOINTS_4,
    }
}

// -- Ganancia mínima para adopción de checkpoint --
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

// ── Puntaje de progreso (progress_score) ────────────────────────────

/// Bono por set completado (K→A) — señal dominante. Igual para todos.
pub const P_COMPLETED_SET_BONUS: i64 = 5000;
/// Multiplicador por carta boca arriba. Igual para todos.
pub const P_FACE_UP_MULT: i64 = 25;
/// Penalización por carta boca abajo. Igual para todos.
pub const P_HIDDEN_PENALTY_MULT: i64 = 40;
/// Multiplicador por carta de stock ya repartida. Igual para todos.
pub const P_STOCK_BONUS_MULT: i64 = 8;
/// Penalización por rey enterrado. Igual para todos.
pub const P_KING_BURIAL_PENALTY: i64 = 60;
/// Bono por columna sin cartas boca abajo. Igual para todos.
pub const P_NEAR_EMPTY_BONUS: i64 = 70;

// -- Suit run multiplier (por variante) --
// En 1-suit toda secuencia es suit run. En 4-suit, rachas puras son oro.
pub const P_SUIT_RUN_MULT_1: i64 = 65;
pub const P_SUIT_RUN_MULT_2: i64 = 120;
pub const P_SUIT_RUN_MULT_4: i64 = 200;
// El multiplicador se aplica a la longitud de la racha de cartas consecutivas
pub fn suit_run_mult(suit_count: u32) -> i64 {
    match suit_count {
        1 => P_SUIT_RUN_MULT_1,
        2 => P_SUIT_RUN_MULT_2,
        _ => P_SUIT_RUN_MULT_4,
    }
}

// -- Bonus por columna vacía (por variante) --
// Columnas vacías son almacenamiento temporal. En 4-suit son esenciales.
pub const P_EMPTY_COLUMN_BONUS_1: i64 = 150;
pub const P_EMPTY_COLUMN_BONUS_2: i64 = 220;
pub const P_EMPTY_COLUMN_BONUS_4: i64 = 350;
// El bono se aplica por cada columna vacía, incentivando su uso estratégico.
pub fn empty_column_bonus(suit_count: u32) -> i64 {
    match suit_count {
        1 => P_EMPTY_COLUMN_BONUS_1,
        2 => P_EMPTY_COLUMN_BONUS_2,
        _ => P_EMPTY_COLUMN_BONUS_4,
    }
}

// -- Deadlock penalty (por variante) --
// Deadlocks son más fatales con más palos.
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

// -- Burial depth multiplier (por variante) --
// Desenterrar targets es más costoso con más palos mezclados.
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
// Penaliza transiciones de palo en face_up de cada columna.
// En 1-suit siempre es 0 (todo es mismo palo).
pub const P_SUIT_FRAG_PENALTY_1: i64 = 0;
pub const P_SUIT_FRAG_PENALTY_2: i64 = 20;
pub const P_SUIT_FRAG_PENALTY_4: i64 = 45;
// La penalización se aplica por cada transición de palo, incentivando columnas más homogéneas.
pub fn suit_frag_penalty(suit_count: u32) -> i64 {
    match suit_count {
        1 => P_SUIT_FRAG_PENALTY_1,
        2 => P_SUIT_FRAG_PENALTY_2,
        _ => P_SUIT_FRAG_PENALTY_4,
    }
}

// ── Ordering: señales thoughtful ────────────────────────────────────

/// Bono cuando un movimiento destapa directamente un target.
pub const O_TARGET_REVEAL_BONUS: i64 = 500;
/// Bono cuando un movimiento destapa una carta a 1-2 valores de un target.
pub const O_NEAR_TARGET_BONUS: i64 = 200;
/// Bono cuando un movimiento destapa un K y hay columna vacía disponible.
pub const O_KING_EMPTY_COL_BONUS: i64 = 300;
/// Bono cuando un movimiento reduce cartas encima de un target enterrado.
pub const O_CRITICAL_PATH_BONUS: i64 = 150;

// -- Same-suit affinity bonus (por variante) --
// En 1-suit no importa (todo es mismo palo). En 4-suit es crítico.
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

// -- Break run penalty (NUEVA, por variante) --
// Penaliza movimientos que rompen un suit run existente.
pub const O_BREAK_RUN_PENALTY_1: i64 = 0;
pub const O_BREAK_RUN_PENALTY_2: i64 = 100;
pub const O_BREAK_RUN_PENALTY_4: i64 = 200;
// La penalización se aplica por cada suit run que se rompe, incentivando mantener rachas de mismo palo.
pub fn break_run_penalty(suit_count: u32) -> i64 {
    match suit_count {
        1 => O_BREAK_RUN_PENALTY_1,
        2 => O_BREAK_RUN_PENALTY_2,
        _ => O_BREAK_RUN_PENALTY_4,
    }
}

// ── Tabla de transposición ──────────────────────────────────────────

// -- Máximo de entradas en la TT (por variante) --
// Con TT persistente entre chunks, suit 2 y 4 acumulan más entradas.
// Límites diferenciados para maximizar retención sin riesgo de OOM.
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

// ── Safety timeout global ────────────────────────────────────────────

/// Timeout global hardcodeado como red de seguridad.
/// Solo actúa entre attempts — el interior del DFS y los chunks son
/// deterministas (controlados por node_limit). En la práctica los
/// node budgets terminan mucho antes de que se alcance este límite.
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

// ── Multi-attempt con perturbación ──────────────────────────────────

// -- Max attempts (por variante) --
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

// -- Perturbation range (por variante) --
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

// -- Deal-eager start (por variante) --
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

// -- Deal-eager frequency (por variante) --
pub const DEAL_EAGER_FREQUENCY_1: u32 = 3;
pub const DEAL_EAGER_FREQUENCY_2: u32 = 3;
pub const DEAL_EAGER_FREQUENCY_4: u32 = 2;
// La frecuencia se aplica a partir del turno definido por deal_eager_start,
// controlando cada cuántos turnos se fuerza un deal.
pub fn deal_eager_frequency(suit_count: u32) -> u32 {
    match suit_count {
        1 => DEAL_EAGER_FREQUENCY_1,
        2 => DEAL_EAGER_FREQUENCY_2,
        _ => DEAL_EAGER_FREQUENCY_4,
    }
}

// ── EmptyColumnDiscipline (por variante) ────────────────────────────

// Mínimo de cartas para bypass automático al mover a columna vacía.
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

// Umbral de sets completados para considerar endgame (relaja la poda).
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
