use super::board::SpiderBoard;
use super::moves::SpiderMove;
use super::rules::{SpiderRule, SpiderTransition};
use super::weights;
use crate::common::card::Card;

/// Motor de generación de jugadas de Spider: genera sucesores con poda por reglas y orden por prioridad.
pub struct SpiderEngine {
    pub suit_count: u32,
    pub rules: Vec<SpiderRule>,
}

impl SpiderEngine {
    /// Crea el motor con reglas por defecto ajustadas a la variante.
    pub fn new(suit_count: u32) -> SpiderEngine {
        SpiderEngine {
            suit_count,
            rules: SpiderRule::default_rules(suit_count),
        }
    }

    /// Condición de victoria: 8 secuencias completadas.
    pub fn is_win(board: &SpiderBoard) -> bool {
        board.completed_sets == 8
    }

    /// Genera transiciones sucesoras con poda y ordenamiento por prioridad.
    /// `attempt` controla la perturbación determinista del ordenamiento:
    /// attempt=0 usa el ordenamiento original, attempt>0 añade ruido
    /// para explorar caminos alternativos.
    pub fn successors(
        &self,
        board: &SpiderBoard,
        previous_move: Option<&SpiderMove>,
        depth: usize,
        attempt: u64,
    ) -> Vec<SpiderTransition> {
        let candidate_moves = find_candidate_moves(board);
        let mut transitions = Vec::with_capacity(candidate_moves.len());
        let next_depth = depth + 1;

        for the_move in candidate_moves {
            // Poda temprana: antes de `apply()` (optimización `can_prune_early`).
            let mut early_pruned = false;
            for rule in &self.rules {
                if rule.can_prune_early(&the_move, previous_move, next_depth) {
                    early_pruned = true;
                    break;
                }
            }
            if early_pruned {
                continue;
            }

            let next_board = match the_move.apply(board) {
                Some(b) => b,
                None => continue,
            };

            let transition = SpiderTransition {
                to_board: next_board,
                the_move,
                depth: next_depth,
            };

            let mut pruned = false;
            for rule in &self.rules {
                if rule.should_prune(&transition, board) {
                    pruned = true;
                    break;
                }
            }
            if !pruned {
                transitions.push(transition);
            }
        }

        // Precalcula métricas del tablero de origen una sola vez (optimización).
        let from_face_down = board.total_face_down();
        let from_suit_run_total: usize = board.columns.iter().map(|c| c.longest_run).sum();
        let targets = board.suit_targets();
        let sc = self.suit_count;

        // 3.3: Ordenamiento por bucketing de 2 niveles.
        // Bucket (ascendente) → local_priority (descendente) dentro de cada bucket.
        // Con attempt>0, se añade perturbación determinista para diversificar la exploración.
        transitions.sort_by(|lhs, rhs| {
            let lb = move_bucket(lhs, board, from_suit_run_total, &targets);
            let rb = move_bucket(rhs, board, from_suit_run_total, &targets);
            if lb != rb {
                return lb.cmp(&rb);
            }
            let lp = local_priority(
                lhs,
                board,
                from_face_down,
                from_suit_run_total,
                &targets,
                sc,
            ) + move_perturbation(attempt, board.signature, &lhs.the_move, sc);
            let rp = local_priority(
                rhs,
                board,
                from_face_down,
                from_suit_run_total,
                &targets,
                sc,
            ) + move_perturbation(attempt, board.signature, &rhs.the_move, sc);
            rp.cmp(&lp)
        });

        transitions
    }
}

/// Reúne todas las jugadas candidatas desde el tablero actual.
fn find_candidate_moves(board: &SpiderBoard) -> Vec<SpiderMove> {
    let mut moves = SpiderMove::find_column_to_column_moves(board);
    moves.extend(SpiderMove::find_deal_from_stock_moves(board));
    moves
}

/// 3.3: Asigna cada transición a un bucket de prioridad (0 = máxima).
///
/// Bucket 0: Completa secuencia K→A
/// Bucket 1: Destapa face-down con target thoughtful
/// Bucket 2: Destapa face-down cualquiera
/// Bucket 3: Mejora suit run (consolidación)
/// Bucket 4: Movimiento a columna del mismo palo
/// Bucket 5: Movimiento genérico entre columnas
/// Bucket 6: Deal from stock
fn move_bucket(
    transition: &SpiderTransition,
    from: &SpiderBoard,
    from_suit_run_total: usize,
    targets: &[Card],
) -> u8 {
    let the_move = &transition.the_move;
    let to = &transition.to_board;

    // Bucket 0: Completa K→A.
    if to.completed_sets > from.completed_sets {
        return 0;
    }

    // Bucket 6: Deal from stock.
    if the_move.is_deal_from_stock() {
        return 6;
    }

    if let SpiderMove::ColumnToColumn {
        source,
        destination,
        card_count,
    } = the_move
    {
        let src_col = &from.columns[*source];

        // ¿Destapa face-down?
        let exposes_fd = *card_count == src_col.num_face_up() && src_col.has_face_down();

        if exposes_fd {
            let exposed = src_col.face_down.last().unwrap();
            if is_target(exposed, targets) {
                return 1; // Bucket 1: Destapa target.
            }
            return 2; // Bucket 2: Destapa face-down cualquiera.
        }

        // ¿Mejora suit run total?
        let to_suit_run: usize = to.columns.iter().map(|c| c.longest_run).sum();
        if to_suit_run > from_suit_run_total {
            return 3; // Bucket 3: Consolidación.
        }

        // ¿Movimiento al mismo palo?
        if let Some(top_of_dest) = from.columns[*destination].top_card() {
            let bottom_idx = src_col.face_up.len() - card_count;
            let first_card = &src_col.face_up[bottom_idx];
            if top_of_dest.suit == first_card.suit {
                return 4; // Bucket 4: Afinidad de palo.
            }
        }

        return 5; // Bucket 5: Genérico.
    }

    6
}

/// Puntaje de prioridad con señales thoughtful (targets, critical path).
/// `suit_count` permite escalar señales por variante.
fn local_priority(
    transition: &SpiderTransition,
    from: &SpiderBoard,
    from_face_down: usize,
    from_suit_run_total: usize,
    targets: &[Card],
    suit_count: u32,
) -> i64 {
    let the_move = &transition.the_move;
    let to = &transition.to_board;

    // Completar una secuencia K→A: prioridad máxima.
    let completed_diff = to.completed_sets as i64 - from.completed_sets as i64;
    let completed_bonus = completed_diff * 5000;

    if completed_bonus > 0 {
        return 10000 + completed_bonus;
    }

    // Repartir desde stock: prioridad baja (último recurso).
    if the_move.is_deal_from_stock() {
        return 10;
    }

    let mut priority: i64 = 0;

    // Bono por destapar cartas boca abajo.
    let after_face_down = to.total_face_down() as i64;
    let exposed_bonus = (from_face_down as i64 - after_face_down).max(0) * 200;
    priority += exposed_bonus;

    // Bono por consolidar rachas del mismo palo.
    let after_suit_run_total: i64 = to.columns.iter().map(|c| c.longest_run as i64).sum();
    let consolidation_bonus = (after_suit_run_total - from_suit_run_total as i64).max(0) * 100;
    priority += consolidation_bonus;

    if let SpiderMove::ColumnToColumn {
        source,
        destination,
        card_count,
    } = the_move
    {
        let src_col = &from.columns[*source];

        // Bono por mover sobre el mismo palo (afinidad de palo) — escalado por variante.
        if let Some(top_of_dest) = from.columns[*destination].top_card() {
            let bottom_idx = src_col.face_up.len() - card_count;
            let first_card = &src_col.face_up[bottom_idx];
            if top_of_dest.suit == first_card.suit {
                priority += weights::same_suit_affinity(suit_count);
            }
        }

        // Penalización por romper suit run existente — escalada por variante.
        let src_col_after = &to.columns[*source];
        if src_col.longest_run > src_col_after.longest_run + card_count {
            priority -= weights::break_run_penalty(suit_count);
        }

        // 2.1: Bono por destapar un target (señal thoughtful).
        // Si este movimiento retira todas las face_up de la columna origen
        // y hay face_down, la carta revelada es la última de face_down.
        if *card_count == src_col.num_face_up() && src_col.has_face_down() {
            let exposed = src_col.face_down.last().unwrap();
            if is_target(exposed, targets) {
                priority += weights::O_TARGET_REVEAL_BONUS;
            } else {
                // Bono menor si la carta revelada está a 1-2 valores de un target.
                for target in targets {
                    if exposed.suit == target.suit {
                        let diff = (exposed.value as i64 - target.value as i64).unsigned_abs();
                        if diff >= 1 && diff <= 2 {
                            priority += weights::O_NEAR_TARGET_BONUS;
                            break;
                        }
                    }
                }
            }
            // Rey destapado con columna vacía disponible: muy valioso.
            if exposed.value == 13 && from.has_empty_column() {
                priority += weights::O_KING_EMPTY_COL_BONUS;
            }
        }

        // 2.3: Critical path — mover cartas de una columna con target enterrado
        // reduce la pila sobre el target, acercándolo a ser revelado.
        if src_col.face_down.iter().any(|c| is_target(c, targets)) {
            priority += weights::O_CRITICAL_PATH_BONUS;
        }
    }

    // Base mínima para movimientos entre columnas.
    priority += 50;

    priority
}

/// Perturbación determinista del local_priority basada en el intento y la firma del tablero.
/// attempt=0 → sin perturbación (ordenamiento original).
/// attempt>0 → añade ruido en [0, range) para reordenar movimientos
/// dentro del mismo bucket sin alterar la estructura entre buckets.
fn move_perturbation(attempt: u64, board_sig: u64, the_move: &SpiderMove, suit_count: u32) -> i64 {
    if attempt == 0 {
        return 0;
    }
    let move_hash: u64 = match the_move {
        SpiderMove::ColumnToColumn {
            source,
            destination,
            card_count,
        } => (*source as u64)
            .wrapping_mul(17)
            .wrapping_add(*destination as u64)
            .wrapping_mul(31)
            .wrapping_add(*card_count as u64),
        SpiderMove::DealFromStock => 0xDEAD,
        SpiderMove::Deal { .. } => 0,
    };
    // Escalar perturbación con el número de intento:
    // intentos tempranos (1-9): perturbación suave (1x)
    // intentos medios (10-19): perturbación moderada (2x)
    // intentos tardíos (20+): perturbación agresiva (3x)
    let scale: u64 = if attempt <= 9 {
        1
    } else if attempt <= 19 {
        2
    } else {
        3
    };
    let range = weights::perturbation_range(suit_count) * scale;
    let h = board_sig
        .wrapping_mul(2654435761) // Knuth multiplicative hash
        .wrapping_add(move_hash)
        .wrapping_mul(attempt.wrapping_mul(6364136223846793005).wrapping_add(1));
    (h % range) as i64
}

/// Verifica si una carta coincide con algún target (mismo palo y valor).
#[inline]
fn is_target(card: &Card, targets: &[Card]) -> bool {
    targets
        .iter()
        .any(|t| t.suit == card.suit && t.value == card.value)
}

/// Reparte un mazo sobre un tablero Spider.
pub fn deal(deck: &[crate::common::card::Card], num_columns: usize) -> Option<SpiderBoard> {
    let deal_move = SpiderMove::Deal {
        deck: deck.to_vec(),
        num_columns,
    };
    deal_move.apply(&SpiderBoard::new(vec![], vec![], 0))
}
