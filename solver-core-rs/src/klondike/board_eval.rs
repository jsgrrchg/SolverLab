//! Evaluación unificada de tablero y política de checkpoints para Klondike.
//!
//! `BoardEval` computa todas las métricas del tablero una sola vez,
//! y expone tanto `heuristic_cost()` como `progress_score()` de forma coherente.
//!
//! `CheckpointPolicy` encapsula la lógica de cuándo hacer checkpoint y cuándo adoptar.

use super::board::KlondikeBoard;
use super::weights::*;
use crate::common::card::Card;

// ═══════════════════════════════════════════
// BoardEval
// ═══════════════════════════════════════════

/// Evaluación completa del estado de un tablero.
/// Se computa una vez y se consulta para heurística y progreso.
pub struct BoardEval {
    pub foundation_count: i64,
    pub face_up: i64,
    pub hidden: i64,
    pub empty_cols: i64,
    pub blocked_kings: i64,
    pub stock_remaining: i64,
    pub waste_count: i64,
    pub stock_inaccessible: i64,
    pub foundation_imbalance: i64, // max_rank - min_rank (raw, sin tolerancia)
    pub depth_penalty: i64,        // suma de fd + (fd-1)/DIVISOR por columna
    pub stock_advance_cost: i64,   // mínimo de advances necesarios para acceder al stock
    pub target_burial_penalty: i64, // penalización por cartas target boca abajo
    pub deadlock_count: i64,       // cantidad de deadlocks lógicos detectados
    pub reveal_bonus: i64,         // bonificación por exponer carta clave
    pub stranded_blockers: i64,    // bloqueadores sin destino encima de targets
    pub stock_target_penalty: i64, // penalización por targets atrapados en stock
}

impl BoardEval {
    /// Constructor rápido: solo métricas base necesarias para heuristic_cost.
    /// Usado en el hot path de IDA* (search_with_bound) donde el rendimiento es crítico.
    /// NO computa thoughtful fields (target_burial, deadlock, reveal_bonus).
    pub fn from_board_fast(board: &KlondikeBoard, draw_advance: u8) -> Self {
        let mut depth_penalty = 0i64;
        let mut hidden = 0i64;
        let mut face_up = 0i64;
        let mut empty_cols = 0i64;

        for c in &board.columns {
            let fd = c.num_face_down() as i64;
            hidden += fd;
            depth_penalty += fd + fd.saturating_sub(1) / HIDDEN_DEPTH_DIVISOR;
            face_up += c.num_face_up() as i64;
            if c.len == 0 {
                empty_cols += 1;
            }
        }

        // Blocked kings: solo cuando no hay columnas vacías
        let blocked_kings = if empty_cols == 0 {
            board
                .columns
                .iter()
                .filter(|c| {
                    c.has_face_down()
                        && c.has_face_up()
                        && c.face_up_cards()
                            .first()
                            .map(|card| card.value == 13)
                            .unwrap_or(false)
                })
                .count() as i64
        } else {
            0
        };

        // Foundation imbalance
        let mut min_rank: i64 = 13;
        let mut max_rank: i64 = 0;
        for &rank in &board.foundation {
            let r = rank as i64;
            if r < min_rank {
                min_rank = r;
            }
            if r > max_rank {
                max_rank = r;
            }
        }

        let foundation_count = board.total_foundation_count() as i64;
        let stock_remaining = board.stock_len.saturating_sub(board.stock_index) as i64;
        let waste_count = board.stock_index as i64;

        // Stock inaccesible: fórmula global restaurada.
        // Con Draw-1 todas las cartas son accesibles secuencialmente.
        // Con Draw-3 solo ceil(remaining/3) son directamente accesibles.
        let da = (draw_advance as i64).max(1);
        let stock_inaccessible = if stock_remaining > 0 && da > 1 {
            let accessible = (stock_remaining + da - 1) / da;
            stock_remaining.saturating_sub(accessible)
        } else {
            0
        };

        // Stock advance cost: número mínimo de moves de advance necesarios
        // para hacer accesible cada carta restante del stock.
        // Estos son moves NO-foundation, adicionales al base 52-fc.
        let stock_advance_cost = if stock_remaining > 0 {
            (stock_remaining + da - 1) / da
        } else {
            0
        };

        Self {
            foundation_count,
            face_up,
            hidden,
            empty_cols,
            blocked_kings,
            stock_remaining,
            waste_count,
            stock_inaccessible,
            foundation_imbalance: max_rank - min_rank,
            depth_penalty,
            stock_advance_cost,
            target_burial_penalty: 0,
            deadlock_count: 0,
            reveal_bonus: 0,
            stranded_blockers: 0,
            stock_target_penalty: 0,
        }
    }

    /// Constructor completo: computa todas las métricas incluyendo señales thoughtful.
    /// Usado para progress_score en consider_progress y checkpoint adoption.
    pub fn from_board(board: &KlondikeBoard, draw_advance: u8) -> Self {
        let mut eval = Self::from_board_fast(board, draw_advance);

        // Computar métricas thoughtful solo para progress_score
        let mut target_cards = [255u8; 4];
        for suit in 0..4 {
            let required_val = board.foundation[suit] + 1;
            if required_val <= 13 {
                target_cards[suit] = required_val;
            }
        }

        for c in &board.columns {
            if c.face_down_len == 0 {
                continue;
            }

            // Para detectar deadlocks: el valor más bajo visto de cada pinta
            // iterando de superficie a fondo
            let mut lowest_ranks = [255i32; 4];

            for i in (0..c.face_down_len).rev() {
                let card = c.cards[i as usize];
                let card_suit = card.suit as usize;
                let card_val = card.value;

                // Target Depth Penalty: cartas target enterradas
                if card_val == target_cards[card_suit] {
                    let obstacle_count = (c.len - i) as i64;
                    eval.target_burial_penalty += obstacle_count * BURIED_TARGET_PENALTY;
                }

                // Deadlock Detection: inversión de rango en misma pinta
                if lowest_ranks[card_suit] < card_val as i32 {
                    let severity = (card_val as i32 - lowest_ranks[card_suit]).min(3) as i64;
                    eval.deadlock_count += severity;
                }
                if (card_val as i32) < lowest_ranks[card_suit] {
                    lowest_ranks[card_suit] = card_val as i32;
                }
            }

            // Reveal Value Reward (Thoughtful: conocemos la carta boca abajo)
            if c.face_down_len > 0 {
                let would_reveal = c.cards[(c.face_down_len - 1) as usize];
                if would_reveal.value == 13 && eval.empty_cols > 0 {
                    eval.reveal_bonus += REVEALED_KING_BONUS;
                } else if would_reveal.value == target_cards[would_reveal.suit as usize] {
                    eval.reveal_bonus += REVEALED_FOUNDATION_CARD_BONUS;
                }
            }
        }

        // ── Análisis de Viabilidad de Bloqueadores (Thoughtful Profundo) ────────────
        // Para cada target enterrado, verificar si los bloqueadores encima
        // tienen destino visible. Un bloqueador "varado" (sin destino en el
        // estado actual) indica que desenterrar ese target será costoso.
        let mut stranded_blockers = 0i64;
        for suit in 0..4usize {
            if target_cards[suit] == 255 {
                continue;
            }
            let target_val = target_cards[suit];

            for col_idx in 0..7usize {
                let col = &board.columns[col_idx];
                let mut found = false;

                for pos in 0..(col.face_down_len as usize) {
                    let card = col.cards[pos];
                    if card.suit as usize == suit && card.value == target_val {
                        // Target encontrado en posición face-down `pos`.
                        // Revisar bloqueadores face-down encima (pos+1..face_down_len).
                        for bp in (pos + 1)..(col.face_down_len as usize) {
                            let blocker = col.cards[bp];
                            if !Self::card_has_column_destination(board, blocker, col_idx) {
                                stranded_blockers += 1;
                            }
                        }
                        // Revisar si el stack face-up puede moverse.
                        // El bottom del face-up run determina si toda la pila puede irse.
                        if col.num_face_up() > 0 {
                            let bottom_fu = col.cards[col.face_down_len as usize];
                            if !Self::card_has_column_destination(board, bottom_fu, col_idx) {
                                stranded_blockers += 1;
                            }
                        }
                        found = true;
                        break;
                    }
                }
                if found {
                    break; // Target de esta pinta ya encontrado
                }
            }
        }
        eval.stranded_blockers = stranded_blockers;

        // ── Accesibilidad de Targets en Stock (Thoughtful Profundo) ────────────
        // Para cada target que está en el stock (no en columnas), evaluar
        // qué tan accesible es dada la posición actual y el draw_advance.
        let mut stock_target_penalty = 0i64;
        let da = draw_advance as usize;
        let si = board.stock_index as usize;

        'suit_loop: for suit in 0..4usize {
            if target_cards[suit] == 255 {
                continue;
            }
            let target_val = target_cards[suit];

            // Primero verificar si el target está en alguna columna
            for col in &board.columns {
                for pos in 0..(col.len as usize) {
                    if col.cards[pos].suit as usize == suit && col.cards[pos].value == target_val {
                        continue 'suit_loop; // En columna: blocker analysis se encarga
                    }
                }
            }

            // Target no está en columnas → buscar en stock
            for pos in 0..(board.stock_len as usize) {
                let card = board.stock[pos];
                if card.suit as usize == suit && card.value == target_val {
                    if si > 0 && pos == si - 1 {
                        // Es el pile card actual: accesible ahora, sin penalización
                    } else if pos < si {
                        // En waste (ya pasó): necesita recycle
                        stock_target_penalty += STOCK_WASTE_TARGET_PENALTY as i64;
                    } else if da > 1 {
                        // En stock restante: verificar alineación con draw_advance
                        let offset = pos - si + 1;
                        if offset % da != 0 {
                            // Misaligned: no accesible en este pass
                            stock_target_penalty += STOCK_MISALIGNED_TARGET_PENALTY as i64;
                        }
                    }
                    break;
                }
            }
        }
        eval.stock_target_penalty = stock_target_penalty;

        eval
    }

    /// Costo heurístico para IDA* (escala ~0–80+, menor = mejor).
    /// Mantiene admisibilidad: nunca sobreestima el costo real.
    /// Las señales thoughtful NO se incluyen aquí para preservar admisibilidad.
    pub fn heuristic_cost(&self) -> i64 {
        // Auto-play detection: si no hay cartas ocultas ni stock, el juego está ganado
        if self.hidden == 0 && self.stock_remaining == 0 && self.waste_count == 0 {
            return 0;
        }

        // Endgame mejorado: sin cartas ocultas, el juego es casi auto-ganado.
        // Solo necesitamos mover cartas restantes a foundations + advances del stock.
        if self.hidden == 0 {
            return 52 - self.foundation_count + self.stock_advance_cost / STOCK_ADVANCE_DIVISOR;
        }

        // Base: cartas que faltan en foundations
        let mut cost = 52 - self.foundation_count;

        // Profundidad de enterramiento
        cost += self.depth_penalty;

        // Movilidad: bonus por columnas vacías con rendimientos decrecientes
        let idx = (self.empty_cols as usize).min(EMPTY_COL_BONUS.len() - 1);
        cost -= EMPTY_COL_BONUS[idx];

        // Kings bloqueados
        cost += self.blocked_kings * BLOCKED_KING_PENALTY;

        // Agotamiento del waste
        cost += self.waste_penalty();

        // Desequilibrio de foundation
        let excess = self.foundation_imbalance - IMBALANCE_TOLERANCE;
        if excess > 0 {
            cost += excess * IMBALANCE_WEIGHT;
        }

        // Costo de avances de stock
        cost += self.stock_advance_cost / STOCK_ADVANCE_DIVISOR;

        // NO sumar target_burial_penalty ni deadlock_count aquí.
        // Esas señales van SOLO a progress_score y successor_ordering.

        cost
    }

    /// Score de progreso para checkpoints (mayor = mejor).
    /// Incorpora señales thoughtful: el solver penaliza checkpoints que
    /// mantienen targets enterrados o deadlocks activos.
    pub fn progress_score(&self) -> i64 {
        let mut score = self.foundation_count * PROGRESS_FOUNDATION_WEIGHT
            + self.face_up * PROGRESS_FACE_UP_WEIGHT
            - self.hidden * PROGRESS_HIDDEN_PENALTY
            + self.empty_cols * PROGRESS_EMPTY_COL_BONUS
            - self.blocked_kings * PROGRESS_BLOCKED_KINGS_PENALTY
            - self.waste_penalty() * PROGRESS_WASTE_MULTIPLIER
            + self.reveal_bonus
            - self.target_burial_penalty * PROGRESS_BURIAL_WEIGHT
            - self.deadlock_count * PROGRESS_DEADLOCK_WEIGHT
            - self.stranded_blockers * PROGRESS_STRANDED_BLOCKER_PENALTY
            - self.stock_target_penalty;

        // Near-autoplay bonus: estados con pocas cartas ocultas están cerca
        // de auto-play (hidden==0 → juego esencialmente ganado).
        // Bonus cuadrático que crece exponencialmente al acercarse a 0.
        if self.hidden > 0 && self.hidden <= AUTOPLAY_PROXIMITY_THRESHOLD {
            let proximity = (AUTOPLAY_PROXIMITY_THRESHOLD + 1 - self.hidden) as i64;
            score += proximity * proximity * PROGRESS_AUTOPLAY_PROXIMITY;
        }

        score
    }

    /// Verifica si una carta tiene destino válido en alguna columna del tableau.
    /// Usado por blocker viability analysis para determinar si un bloqueador
    /// puede moverse fuera del camino de un target enterrado.
    fn card_has_column_destination(board: &KlondikeBoard, card: Card, exclude_col: usize) -> bool {
        if card.value == 13 {
            // Rey: necesita columna vacía
            return board
                .columns
                .iter()
                .enumerate()
                .any(|(i, c)| i != exclude_col && c.len == 0);
        }
        for (i, col) in board.columns.iter().enumerate() {
            if i == exclude_col {
                continue;
            }
            if let Some(top) = col.top_face_up() {
                if top.value == card.value + 1 && top.color() != card.color() {
                    return true;
                }
            }
        }
        false
    }

    /// Penalización de waste compartida entre heuristic_cost y progress_score.
    fn waste_penalty(&self) -> i64 {
        let mut p = 0;
        if self.stock_remaining <= WASTE_THRESHOLD_STOCK
            && self.waste_count >= WASTE_THRESHOLD_COUNT
        {
            p += 1 + (self.waste_count - WASTE_THRESHOLD_COUNT) / WASTE_SCALE_DIVISOR;
        }
        if self.stock_remaining == 0 && self.waste_count > 0 {
            p += self.waste_count / WASTE_EMPTY_STOCK_DIVISOR;
        }
        p
    }
}

// ═══════════════════════════════════════════
// CheckpointPolicy
// ═══════════════════════════════════════════

/// Política de checkpoints: encapsula todas las decisiones sobre
/// cuándo hacer checkpoint, cuándo adoptar, y cuándo abandonar.
pub struct CheckpointPolicy {
    pub max_adoptions: u32,
    pub base_limits: [u64; 7],
}

impl CheckpointPolicy {
    /// Política por defecto: los valores que actualmente usa el solver.
    pub fn default_policy() -> Self {
        Self {
            max_adoptions: MAX_CHECKPOINTS,
            base_limits: CHECKPOINT_BASE_LIMITS,
        }
    }

    /// Calcula el node_limit base según la dificultad del tablero.
    pub fn base_limit_for(&self, board: &KlondikeBoard) -> u64 {
        let fc = board.total_foundation_count();
        let hidden: usize = board.columns.iter().map(|c| c.num_face_down()).sum();

        if hidden > 18 {
            self.base_limits[5]
        } else if hidden > 15 {
            self.base_limits[4]
        } else if hidden > 12 {
            self.base_limits[3]
        } else if hidden > 9 {
            self.base_limits[2]
        } else if hidden > 6 {
            self.base_limits[1]
        } else if fc > 30 {
            self.base_limits[6]
        } else {
            self.base_limits[0]
        }
    }
    /// Decide si un delta de progreso es adoptable dada la fase actual.
    /// `foundation_count`: número real de cartas en foundations (no derivado del score).
    pub fn is_adoptable(&self, start_score: i64, best_score: i64, foundation_count: usize) -> bool {
        let delta = best_score - start_score;

        let fc = foundation_count as i64;
        let (clamp_min, clamp_max) = if fc < ADOPT_EARLY_THRESHOLD_FC {
            ADOPT_EARLY_CLAMP
        } else if fc < ADOPT_MID_THRESHOLD_FC {
            ADOPT_MID_CLAMP
        } else {
            ADOPT_LATE_CLAMP
        };
        let relative_threshold = (start_score.saturating_abs() * ADOPT_RELATIVE_PCT) / 100;
        let adopt_threshold = relative_threshold.clamp(clamp_min, clamp_max);
        delta >= adopt_threshold
    }
}
