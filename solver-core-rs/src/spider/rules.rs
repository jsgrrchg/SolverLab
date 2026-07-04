use super::board::SpiderBoard;
use super::moves::SpiderMove;
use super::weights;

/// A solver transition: the resulting board and the move that produced it.
/// Note: Spider transitions do NOT include `from_board` (it is passed separately).
pub struct SpiderTransition {
    pub to_board: SpiderBoard,
    pub the_move: SpiderMove,
    pub depth: usize,
}

/// Spider pruning rules as an `enum` (replaces the Swift protocol).
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
    /// Default rules adjusted to the variant. Uses DFS_MAX_DEPTH from weights.
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

    /// Early pruning: evaluates without the resulting board (before `apply`).
    /// Only `DepthLimit` and `NoImmediateUndo` support this path.
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

    /// Full pruning: evaluates with the source board and resulting transition.
    /// `DepthLimit` and `NoImmediateUndo` are evaluated in `can_prune_early` (before apply).
    pub fn should_prune(&self, transition: &SpiderTransition, from_board: &SpiderBoard) -> bool {
        match self {
            // Evaluated in can_prune_early; they never reach this point.
            SpiderRule::DepthLimit { .. } | SpiderRule::NoImmediateUndo => false,

            SpiderRule::NoopTransition => {
                from_board.signature == transition.to_board.signature
                    && *from_board == transition.to_board
            }

            SpiderRule::EmptyColumnDiscipline {
                min_cards,
                endgame_threshold,
            } => {
                // Only applies to column-to-column moves toward an empty column.
                let (source, destination, count) = match &transition.the_move {
                    SpiderMove::ColumnToColumn {
                        source,
                        destination,
                        card_count,
                    } => (*source, *destination, *card_count),
                    _ => return false,
                };

                let dest_col_before = &from_board.columns[destination];
                // Only applies when the destination was empty.
                if !dest_col_before.is_empty() {
                    return false;
                }

                // Large moves or endgame: always allowed.
                if count >= *min_cards || from_board.completed_sets >= *endgame_threshold {
                    return false;
                }

                let src_col_before = &from_board.columns[source];
                // Allow the move if it reveals face-down cards.
                if src_col_before.has_face_down() {
                    return false;
                }

                let src_col_after = &transition.to_board.columns[source];
                let dest_col_after = &transition.to_board.columns[destination];

                let before_suit_run = src_col_before.longest_run;
                let after_dest_run = dest_col_after.longest_run;
                let after_src_run = src_col_after.longest_run;

                // Allow the move if it improves the same-suit run on either side.
                if after_dest_run > before_suit_run || after_src_run > before_suit_run {
                    return false;
                }

                // Allow the move if a sequence was completed.
                if transition.to_board.completed_sets > from_board.completed_sets {
                    return false;
                }

                // Prune: moving to an empty column without improving anything.
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
