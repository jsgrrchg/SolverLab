use super::board::KlondikeBoard;
use super::moves::KlondikeMove;

// ═══════════════════════════════════════════
// Transición de tablero
// ═══════════════════════════════════════════

/// Representa el paso de un estado a otro durante la búsqueda IDA*.
/// Contiene el tablero destino, el movimiento aplicado,
/// el movimiento anterior (para detectar undos inmediatos) y la profundidad actual.
/// El tablero origen (from_board) lo mantiene el caller y se pasa por referencia
/// a las funciones que lo necesitan, evitando ~376 bytes de copia redundante por transición.
pub struct KlondikeTransition {
    pub to_board: KlondikeBoard,
    pub the_move: KlondikeMove,
    pub previous_move: Option<KlondikeMove>,
    pub depth: usize,
}

// ═══════════════════════════════════════════
// Reglas de poda
// ═══════════════════════════════════════════

/// Reglas de poda que el solver aplica a cada transición antes de explorarla.
/// Si alguna regla retorna `should_prune = true`, el movimiento se descarta
/// sin descender en la rama de búsqueda.
#[derive(Debug, Clone)]
pub enum KlondikeRule {
    /// Descarta movimientos que superen la profundidad máxima permitida.
    DepthLimit { max_depth: usize },

    /// Descarta movimientos que deshagan exactamente el movimiento anterior
    /// (undo inmediato). Evita ciclos cortos sin progreso.
    NoImmediateUndo,

    /// Descarta movimientos donde el tablero resultante es idéntico al anterior.
    /// Protege contra movimientos que no cambian el estado (e.g., stock advance
    /// con stock vacío que el motor no debería generar, pero por seguridad).
    NoopTransition,

    /// Solo permite mover un Rey a una columna vacía si el movimiento
    /// expone al menos una carta boca abajo en la columna origen.
    /// Evita mover Reyes a columnas vacías de forma improductiva.
    KingToEmptyMustExpose,

    /// Solo permite devolver una carta de foundation al tableau si el movimiento
    /// incrementa el número total de cartas boca arriba en el tableau.
    /// Evita undos de foundation puramente destructivos.
    FoundationRollback,
}

impl KlondikeRule {
    /// Evalúa si un movimiento puede podarse ANTES de llamar a `apply()`.
    /// Solo evalúa reglas que no necesitan el board resultante, evitando
    /// la copia de ~376 bytes + compute_signature cuando la poda es segura.
    pub fn can_prune_early(
        &self,
        board: &KlondikeBoard,
        the_move: KlondikeMove,
        previous_move: Option<KlondikeMove>,
        depth: usize,
    ) -> bool {
        match self {
            KlondikeRule::DepthLimit { max_depth } => depth > *max_depth,

            KlondikeRule::NoImmediateUndo => {
                let prev = match previous_move {
                    Some(p) => p,
                    None => return false,
                };
                match (prev, the_move) {
                    (
                        KlondikeMove::ColumnToColumn {
                            source: a,
                            destination: b,
                            count: c1,
                        },
                        KlondikeMove::ColumnToColumn {
                            source: c,
                            destination: d,
                            count: c2,
                        },
                    ) => a == d && b == c && c1 == c2,
                    (
                        KlondikeMove::ColumnToFoundation { source, card },
                        KlondikeMove::FoundationToColumn {
                            destination,
                            card: same_card,
                        },
                    ) => source == destination && card == same_card,
                    (
                        KlondikeMove::FoundationToColumn { destination, card },
                        KlondikeMove::ColumnToFoundation {
                            source,
                            card: same_card,
                        },
                    ) => source == destination && card == same_card,
                    _ => false,
                }
            }

            KlondikeRule::KingToEmptyMustExpose => {
                let (source, destination, count) = match the_move {
                    KlondikeMove::ColumnToColumn {
                        source,
                        destination,
                        count,
                    } => (source, destination, count),
                    _ => return false,
                };
                let col = &board.columns[source as usize];
                if count == 0 || count > col.num_face_up() as u8 || count > col.len {
                    return false;
                }
                if col.len == 0 {
                    return false;
                }
                let run_start = col.len - count;
                if col.cards[run_start as usize].value != 13 {
                    return false;
                }
                let to_col_before = &board.columns[destination as usize];
                if to_col_before.has_face_up() {
                    return false;
                }
                let source_len_after = col.len - count;
                let exposes_hidden = source_len_after > 0 && source_len_after == col.face_down_len;
                !exposes_hidden
            }

            _ => false,
        }
    }

    /// Conjunto de reglas para el modo Fast: incluye todas las podas heurísticas
    /// (`KingToEmptyMustExpose`, `FoundationRollback`) que reducen el espacio de
    /// búsqueda agresivamente, a costa de no explorar algunos caminos poco comunes.
    pub fn default_rules_fast(max_depth: usize) -> Vec<KlondikeRule> {
        vec![
            KlondikeRule::DepthLimit { max_depth },
            KlondikeRule::NoImmediateUndo,
            KlondikeRule::NoopTransition,
            KlondikeRule::KingToEmptyMustExpose,
            KlondikeRule::FoundationRollback,
        ]
    }

    /// Conjunto de reglas para el modo Strict: solo incluye podas seguras
    /// (límite de profundidad, undo inmediato, noop). Más lento pero completo:
    /// no descarta ramas potencialmente válidas.
    pub fn default_rules_strict(max_depth: usize) -> Vec<KlondikeRule> {
        vec![
            KlondikeRule::DepthLimit { max_depth },
            KlondikeRule::NoImmediateUndo,
            KlondikeRule::NoopTransition,
        ]
    }

    /// Evalúa si una transición debe ser podada según esta regla.
    /// Retorna `true` si el movimiento debe descartarse.
    ///
    /// Nota: `DepthLimit`, `NoImmediateUndo` y `KingToEmptyMustExpose` se evalúan
    /// en `can_prune_early()` antes de `apply()`. Aquí retornan `false` directamente.
    pub fn should_prune(
        &self,
        transition: &KlondikeTransition,
        from_board: &KlondikeBoard,
    ) -> bool {
        match self {
            // Evaluados en can_prune_early — no llegan aquí
            KlondikeRule::DepthLimit { .. }
            | KlondikeRule::NoImmediateUndo
            | KlondikeRule::KingToEmptyMustExpose => false,

            // Poda por noop: el tablero no cambió tras el movimiento
            KlondikeRule::NoopTransition => *from_board == transition.to_board,

            // Poda de rollback de foundation: solo permite devolver una carta de foundation
            // al tableau si el resultado tiene más cartas boca arriba que el estado anterior.
            KlondikeRule::FoundationRollback => {
                if !transition.the_move.is_from_foundation() {
                    return false;
                }
                let before_exposure: usize =
                    from_board.columns.iter().map(|c| c.num_face_up()).sum();
                let after_exposure: usize = transition
                    .to_board
                    .columns
                    .iter()
                    .map(|c| c.num_face_up())
                    .sum();
                after_exposure <= before_exposure
            }
        }
    }
}

// ═══════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::card::{Card, Suit};

    fn has_foundation_rollback(rules: &[KlondikeRule]) -> bool {
        rules
            .iter()
            .any(|r| matches!(r, KlondikeRule::FoundationRollback))
    }

    fn has_king_to_empty_must_expose(rules: &[KlondikeRule]) -> bool {
        rules
            .iter()
            .any(|r| matches!(r, KlondikeRule::KingToEmptyMustExpose))
    }

    /// Verifica que Fast incluya las reglas heurísticas y Strict no.
    #[test]
    fn fast_vs_strict_rule_sets_differ_on_heuristic_pruning() {
        let fast = KlondikeRule::default_rules_fast(100);
        let strict = KlondikeRule::default_rules_strict(100);

        assert!(has_foundation_rollback(&fast));
        assert!(has_king_to_empty_must_expose(&fast));

        assert!(!has_foundation_rollback(&strict));
        assert!(!has_king_to_empty_must_expose(&strict));
    }

    /// Verifica que KingToEmptyMustExpose permita el movimiento cuando expone
    /// una carta boca abajo, y lo pode cuando no expone ninguna.
    /// La poda se evalúa en can_prune_early (pre-apply), no en should_prune.
    #[test]
    fn king_to_empty_must_expose_only_allows_hidden_flip() {
        let rule = KlondikeRule::KingToEmptyMustExpose;
        let the_move = KlondikeMove::ColumnToColumn {
            source: 0,
            destination: 1,
            count: 1,
        };

        // Caso permitido: columna 0 tiene una carta oculta debajo del Rey
        let mut from = KlondikeBoard::new();
        from.columns[0].push(Card::new(Suit::Club, 9), true);
        from.columns[0].push(Card::new(Suit::Heart, 13), false);

        assert!(
            !rule.can_prune_early(&from, the_move, None, 1),
            "moving king should be allowed if it flips a hidden card"
        );

        // Caso podado: columna 0 solo tiene el Rey, sin cartas ocultas debajo
        let mut no_expose = KlondikeBoard::new();
        no_expose.columns[0].push(Card::new(Suit::Spade, 13), false);

        assert!(
            rule.can_prune_early(&no_expose, the_move, None, 1),
            "moving king should be pruned if it does not expose hidden cards"
        );
    }
}
