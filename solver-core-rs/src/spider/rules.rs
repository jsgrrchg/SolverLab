use super::board::SpiderBoard;
use super::moves::SpiderMove;
use super::weights;

/// Una transición del solver: el tablero resultante y la jugada que lo produjo.
/// Nota: las transiciones de Spider NO incluyen `from_board` (se pasa por separado).
pub struct SpiderTransition {
    pub to_board: SpiderBoard,
    pub the_move: SpiderMove,
    pub depth: usize,
}

/// Reglas de poda de Spider como `enum` (reemplaza el protocolo de Swift).
#[derive(Debug, Clone)]
pub enum SpiderRule {
    DepthLimit {
        max_depth: usize,
    },
    NoImmediateUndo,
    NoopTransition,
    EmptyColumnDiscipline {
        min_cards: usize,
        endgame_threshold: usize,
    },
}

impl SpiderRule {
    /// Reglas por defecto ajustadas a la variante. Usa DFS_MAX_DEPTH de weights.
    pub fn default_rules(suit_count: u32) -> Vec<SpiderRule> {
        vec![
            SpiderRule::DepthLimit {
                max_depth: weights::DFS_MAX_DEPTH,
            },
            SpiderRule::NoImmediateUndo,
            SpiderRule::NoopTransition,
            SpiderRule::EmptyColumnDiscipline {
                min_cards: weights::empty_col_min_cards(suit_count),
                endgame_threshold: weights::empty_col_endgame_threshold(suit_count),
            },
        ]
    }

    /// Poda temprana: evalúa sin tablero resultante (antes de `apply`).
    /// Solo `DepthLimit` y `NoImmediateUndo` soportan esta vía.
    pub fn can_prune_early(
        &self,
        the_move: &SpiderMove,
        previous_move: Option<&SpiderMove>,
        depth: usize,
    ) -> bool {
        match self {
            SpiderRule::DepthLimit { max_depth } => depth > *max_depth,

            SpiderRule::NoImmediateUndo => {
                if let Some(prev) = previous_move {
                    match (prev, the_move) {
                        (
                            SpiderMove::ColumnToColumn {
                                source: a,
                                destination: b,
                                card_count: prev_count,
                            },
                            SpiderMove::ColumnToColumn {
                                source: c,
                                destination: d,
                                card_count: count,
                            },
                        ) => *a == *d && *b == *c && prev_count == count,
                        _ => false,
                    }
                } else {
                    false
                }
            }

            _ => false,
        }
    }

    /// Poda completa: evalúa con el tablero de origen y la transición resultante.
    /// `DepthLimit` y `NoImmediateUndo` se evalúan en `can_prune_early` (antes de apply).
    pub fn should_prune(&self, transition: &SpiderTransition, from_board: &SpiderBoard) -> bool {
        match self {
            // Evaluados en can_prune_early, nunca llegan aquí.
            SpiderRule::DepthLimit { .. } | SpiderRule::NoImmediateUndo => false,

            SpiderRule::NoopTransition => {
                from_board.signature == transition.to_board.signature
                    && *from_board == transition.to_board
            }

            SpiderRule::EmptyColumnDiscipline {
                min_cards,
                endgame_threshold,
            } => {
                // Solo aplica a movimientos entre columnas hacia una columna vacía.
                let (source, destination, count) = match &transition.the_move {
                    SpiderMove::ColumnToColumn {
                        source,
                        destination,
                        card_count,
                    } => (*source, *destination, *card_count),
                    _ => return false,
                };

                let dest_col_before = &from_board.columns[destination];
                // Solo aplica cuando el destino estaba vacío.
                if !dest_col_before.is_empty() {
                    return false;
                }

                // Movimientos grandes o endgame: siempre permitidos.
                if count >= *min_cards || from_board.completed_sets >= *endgame_threshold {
                    return false;
                }

                let src_col_before = &from_board.columns[source];
                // Permite el movimiento si destapa cartas boca abajo.
                if src_col_before.has_face_down() {
                    return false;
                }

                let src_col_after = &transition.to_board.columns[source];
                let dest_col_after = &transition.to_board.columns[destination];

                let before_suit_run = src_col_before.longest_run;
                let after_dest_run = dest_col_after.longest_run;
                let after_src_run = src_col_after.longest_run;

                // Permite el movimiento si mejora la racha del mismo palo en cualquiera de los lados.
                if after_dest_run > before_suit_run || after_src_run > before_suit_run {
                    return false;
                }

                // Permite el movimiento si se completó una secuencia.
                if transition.to_board.completed_sets > from_board.completed_sets {
                    return false;
                }

                // Poda: mover a columna vacía sin mejorar nada.
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_depth_limit() {
        let rule = SpiderRule::DepthLimit { max_depth: 100 };
        assert!(rule.can_prune_early(&SpiderMove::DealFromStock, None, 101));
        assert!(!rule.can_prune_early(&SpiderMove::DealFromStock, None, 100));
    }

    #[test]
    fn test_no_immediate_undo() {
        let rule = SpiderRule::NoImmediateUndo;
        let prev = SpiderMove::ColumnToColumn {
            source: 0,
            destination: 1,
            card_count: 1,
        };
        let undo = SpiderMove::ColumnToColumn {
            source: 1,
            destination: 0,
            card_count: 1,
        };
        let other = SpiderMove::ColumnToColumn {
            source: 2,
            destination: 3,
            card_count: 1,
        };

        assert!(rule.can_prune_early(&undo, Some(&prev), 5));
        assert!(!rule.can_prune_early(&other, Some(&prev), 5));
        assert!(!rule.can_prune_early(&undo, None, 5));
    }
}
