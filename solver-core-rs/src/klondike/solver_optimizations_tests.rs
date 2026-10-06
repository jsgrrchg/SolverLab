use super::*;
use crate::common::card::Card;

fn visible_endgame() -> KlondikeBoard {
    let mut board = KlondikeBoard::new();
    board.foundation = [10; 4];
    for (column, suits) in [
        [Suit::Club, Suit::Heart, Suit::Club],
        [Suit::Diamond, Suit::Spade, Suit::Heart],
        [Suit::Heart, Suit::Club, Suit::Diamond],
        [Suit::Spade, Suit::Diamond, Suit::Spade],
    ]
    .into_iter()
    .enumerate()
    {
        for (suit, rank) in suits.into_iter().zip([13, 12, 11]) {
            board.columns[column].push(Card::new(suit, rank), false);
        }
    }
    board.compute_signature();
    board
}

fn assert_full_deck(board: &KlondikeBoard) {
    let mut cards = board.stock[..board.stock_len as usize].to_vec();
    for column in &board.columns {
        cards.extend_from_slice(&column.cards[..column.len as usize]);
    }
    for suit in Suit::ALL {
        cards.extend((1..=board.foundation[suit as usize]).map(|rank| Card::new(suit, rank)));
    }
    cards.sort_by_key(|card| card.encode());
    assert_eq!(cards, Card::standard_deck());
}

fn replay(mut board: KlondikeBoard, moves: &[KlondikeMove], draw: u8) -> KlondikeBoard {
    assert_full_deck(&board);
    for &step in moves {
        assert!(KlondikeMove::find_candidate_moves(&board, draw).contains(&step));
        board = step.apply(board).expect("legal primitive replay");
        assert_full_deck(&board);
    }
    board
}

fn context(draw: u8) -> IdaContext {
    IdaContext::new(Instant::now(), None, i64::MIN, draw)
}

#[test]
fn visible_closure_replays_all_cards_and_keeps_the_existing_prefix_once() {
    let initial = visible_endgame();
    let first = KlondikeMove::ColumnToFoundation {
        source: 0,
        card: Card::new(Suit::Club, 11),
    };
    let after = first.apply(initial).unwrap();
    for draw in [1, 3] {
        for mode in [KlondikeSolveMode::Fast, KlondikeSolveMode::Strict] {
            let solver = KlondikeSolver::new_with_mode(draw, mode);
            let mut ctx = context(draw as u8);
            assert_eq!(
                solver.try_finish_visible_without_stock(&after, 1, 12, &[first], &mut ctx,),
                Some(IdaSearchResult::Found)
            );
            assert_eq!(ctx.checked_nodes, 11);
            assert_eq!(ctx.checkpoint_nodes, 11);
            assert_eq!(ctx.progress_sample_counter, 11);
            let moves = ctx.solution_path.unwrap();
            assert_eq!(moves.len(), 12);
            assert_eq!(moves[0], first);
            assert!(KlondikeSolver::is_win(&replay(initial, &moves, draw as u8)));
            // Exercise the public root path, including its initial zero heuristic.
            let stats = solver.solve_with_stats(&initial, false);
            let moves = stats.moves.expect("direct closure from root");
            assert_eq!(moves.len(), 12);
            assert_eq!(stats.checkpoints_adopted, 0);
            let won = replay(initial, &moves, draw as u8);
            assert!(KlondikeSolver::is_win(&won));
            assert_eq!(solver.solve(&won, false), Some(vec![]));
            let mut won_ctx = context(draw as u8);
            assert_eq!(
                solver.try_finish_visible_without_stock(&won, 0, 0, &[], &mut won_ctx),
                Some(IdaSearchResult::Found)
            );
            assert_eq!(won_ctx.checked_nodes, 0);
        }
    }
}

#[test]
fn visible_closure_returns_exact_bound_and_obeys_the_smallest_depth_limit() {
    let board = visible_endgame();
    let mut solver = KlondikeSolver::new(1);
    let mut ctx = context(1);
    assert_eq!(
        solver.try_finish_visible_without_stock(&board, 0, 11, &[], &mut ctx),
        Some(IdaSearchResult::NextBound(12))
    );
    assert_eq!(ctx.checked_nodes, 0);
    assert_eq!(ctx.progress_sample_counter, 0);
    solver
        .rules
        .push(KlondikeRule::DepthLimit { max_depth: 11 });
    assert_eq!(
        solver.try_finish_visible_without_stock(&board, 0, 100, &[], &mut ctx),
        Some(IdaSearchResult::NextBound(i64::MAX))
    );
    assert_eq!(solver.solve(&board, false), None);
    assert_eq!(ctx.checked_nodes, 0);
}

#[test]
fn visible_closure_checks_budgets_before_accepting_the_last_move_as_a_win() {
    let board = visible_endgame();
    let solver = KlondikeSolver::new(3);
    for limit in [1, 4, 12] {
        let mut ctx = context(3);
        ctx.checkpoint_node_limit = limit;
        assert_eq!(
            solver.try_finish_visible_without_stock(&board, 0, 12, &[], &mut ctx),
            Some(IdaSearchResult::CheckpointTriggered)
        );
        assert_eq!(ctx.checked_nodes, limit);
        assert_eq!(ctx.checkpoint_nodes, limit);
        assert_eq!(ctx.progress_sample_counter, limit);
        assert!(ctx.solution_path.is_none());
        assert_eq!(ctx.best_progress_path.len(), (limit / 4 * 4) as usize);
        replay(board, &ctx.best_progress_path, 3);
    }
    let mut expired = IdaContext::new(
        Instant::now() - Duration::from_secs(1),
        Some(Duration::ZERO),
        0,
        3,
    );
    assert_eq!(
        solver.try_finish_visible_without_stock(&board, 0, 12, &[], &mut expired),
        Some(IdaSearchResult::Timeout)
    );
    assert_eq!(expired.checked_nodes, 0);
    assert!(expired.solution_path.is_none());
}

#[test]
fn visible_closure_declines_stock_waste_and_hidden_cards_and_rejects_dead_ends() {
    let solver = KlondikeSolver::new(1);
    for stock_index in [0, 1] {
        let mut board = visible_endgame();
        board.stock[0] = board.columns[0].pop().unwrap();
        board.stock_len = 1;
        board.stock_index = stock_index;
        board.compute_signature();
        assert_full_deck(&board);
        assert_eq!(
            solver.try_finish_visible_without_stock(&board, 0, 100, &[], &mut context(1)),
            None
        );
    }
    let mut hidden = visible_endgame();
    hidden.columns[0].face_down_len = 1;
    assert_eq!(
        solver.try_finish_visible_without_stock(&hidden, 0, 100, &[], &mut context(1)),
        None
    );
    let mut ctx = context(1);
    assert_eq!(
        solver.try_finish_visible_without_stock(&KlondikeBoard::new(), 0, 52, &[], &mut ctx),
        Some(IdaSearchResult::NextBound(i64::MAX))
    );
    assert!(ctx.solution_path.is_none());
}

// A deliberately primitive DFS oracle: one recursive call per move, with no
// terminal closure or chain compression. Used only on bounded fixtures that
// retain hidden cards or stock, to isolate P4c's accounting and backtracking.
impl KlondikeSolver {
    fn search_primitive_reference(
        &self,
        board: &KlondikeBoard,
        depth: usize,
        bound: i64,
        path: &mut Vec<KlondikeMove>,
        undo_count: usize,
        path_signatures: &mut AHashSet<u64>,
        tt: &mut AHashMap<KlondikeTtKey, i64>,
        context: &mut IdaContext,
    ) -> IdaSearchResult {
        if context.should_timeout() {
            return IdaSearchResult::Timeout;
        }
        if context.should_checkpoint() {
            return IdaSearchResult::CheckpointTriggered;
        }

        // Limit memory: clear the TT when it exceeds the configured threshold.
        if tt.len() >= weights::TT_MAX_ENTRIES {
            tt.clear();
        }

        let h_cost = BoardEval::from_board_fast(board, self.draw_advance).heuristic_cost();
        let f_score = depth as i64 + h_cost;
        // Typical IDA* pruning: the node exceeds the f = g + h bound.
        if f_score > bound {
            return IdaSearchResult::NextBound(f_score);
        }

        if Self::is_win(board) {
            context.solution_path = Some(path.clone());
            return IdaSearchResult::Found;
        }

        let prev_move = path.last().copied();
        let transitions = self.successors(board, prev_move, depth);
        let mut min_next_bound = i64::MAX;

        for transition in transitions {
            let next_board = transition.to_board;
            let next_signature = next_board.signature;
            let next_depth = depth + 1;
            let next_remaining_budget = bound - next_depth as i64;
            let next_undo_count = if transition.the_move.is_from_foundation() {
                undo_count + 1
            } else {
                undo_count
            };

            if let Some(max) = self.max_undos {
                if next_undo_count > max {
                    continue;
                }
            }
            // Avoid cycles in the current path (depth-first search).
            if path_signatures.contains(&next_signature) {
                continue;
            }

            let next_tt_key = self.tt_key(next_signature, next_undo_count);
            if Self::should_prune_by_tt(tt, next_tt_key, next_remaining_budget) {
                continue;
            }

            Self::upsert_remaining_budget(tt, next_tt_key, next_remaining_budget);
            path_signatures.insert(next_signature);
            path.push(transition.the_move);
            context.consider_progress(&next_board, path);

            match self.search_primitive_reference(
                &next_board,
                next_depth,
                bound,
                path,
                next_undo_count,
                path_signatures,
                tt,
                context,
            ) {
                IdaSearchResult::Found => return IdaSearchResult::Found,
                IdaSearchResult::Timeout => return IdaSearchResult::Timeout,
                IdaSearchResult::CheckpointTriggered => {
                    return IdaSearchResult::CheckpointTriggered
                }
                IdaSearchResult::NextBound(nb) => {
                    if nb < min_next_bound {
                        min_next_bound = nb;
                    }
                }
            }

            path.pop();
            path_signatures.remove(&next_signature);
        }
        IdaSearchResult::NextBound(min_next_bound)
    }
}

fn forced_chain() -> KlondikeBoard {
    let mut board = KlondikeBoard::new();
    board.foundation = [0, 2, 2, 2];
    for rank in (1..=5).rev() {
        board.columns[0].push(Card::new(Suit::Club, rank), rank != 1);
    }
    board.stock[0] = Card::new(Suit::Spade, 9);
    board.stock_len = 1;
    board.compute_signature();
    board
}

#[derive(Debug, PartialEq, Eq)]
struct SearchSnapshot {
    result: IdaSearchResult,
    path: Vec<KlondikeMove>,
    signatures: Vec<u64>,
    tt: Vec<(u64, u16, i64)>,
    checked: u64,
    checkpoint: u64,
    samples: u64,
    progress: i64,
    progress_path: Vec<KlondikeMove>,
    solution: Option<Vec<KlondikeMove>>,
}

#[test]
fn safe_chains_match_primitive_search_at_every_intermediate_cut_and_branch() {
    for draw in [1, 3] {
        for mode in [KlondikeSolveMode::Fast, KlondikeSolveMode::Strict] {
            for scenario in 0..10 {
                let mut board = forced_chain();
                if scenario == 8 {
                    // Two safe candidates: preserve both branches instead of choosing one.
                    board.foundation[Suit::Spade as usize] = 0;
                    board.columns[1].push(Card::new(Suit::Spade, 1), false);
                    board.compute_signature();
                }
                let mut solver = KlondikeSolver::new_with_mode(draw, mode);
                solver.rules.push(KlondikeRule::DepthLimit {
                    max_depth: if scenario == 5 { 2 } else { 5 },
                });
                solver.max_undos = Some(0);
                let first = solver.successors(&board, None, 0).remove(0);
                let after = first.to_board;
                let initial_path = vec![first.the_move];
                let bound = if scenario == 9 { 1 } else { 100 };
                let run = |compressed: bool| {
                    let mut ctx = context(draw as u8);
                    ctx.checkpoint_node_limit = match scenario {
                        1..=4 => scenario,
                        _ => 1000,
                    };
                    if scenario == 7 {
                        ctx.start = Instant::now() - Duration::from_secs(1);
                        ctx.timeout = Some(Duration::ZERO);
                        ctx.checked_nodes = 2046;
                    }
                    let mut signatures = AHashSet::from_iter([board.signature, after.signature]);
                    let mut tt = AHashMap::new();
                    if scenario == 0 || scenario == 6 {
                        let child = solver
                            .successors(&after, initial_path.last().copied(), 1)
                            .remove(0);
                        if scenario == 0 {
                            signatures.insert(child.to_board.signature);
                        } else {
                            KlondikeSolver::upsert_remaining_budget(
                                &mut tt,
                                solver.tt_key(child.to_board.signature, 0),
                                98,
                            );
                        }
                    }
                    let mut path = initial_path.clone();
                    let result = if compressed {
                        solver.search_with_bound(
                            &after,
                            1,
                            bound,
                            &mut path,
                            0,
                            &mut signatures,
                            &mut tt,
                            &mut ctx,
                        )
                    } else {
                        solver.search_primitive_reference(
                            &after,
                            1,
                            bound,
                            &mut path,
                            0,
                            &mut signatures,
                            &mut tt,
                            &mut ctx,
                        )
                    };
                    let mut signatures: Vec<_> = signatures.into_iter().collect();
                    signatures.sort_unstable();
                    let mut tt: Vec<_> = tt
                        .into_iter()
                        .map(|(key, value)| (key.signature, key.undo_bucket, value))
                        .collect();
                    tt.sort_unstable();
                    SearchSnapshot {
                        result,
                        path,
                        signatures,
                        tt,
                        checked: ctx.checked_nodes,
                        checkpoint: ctx.checkpoint_nodes,
                        samples: ctx.progress_sample_counter,
                        progress: ctx.best_progress,
                        progress_path: ctx.best_progress_path,
                        solution: ctx.solution_path,
                    }
                };
                let actual = run(true);
                let expected = run(false);
                assert_eq!(
                    actual, expected,
                    "draw {draw}, mode {mode:?}, scenario {scenario}"
                );
                if matches!(actual.result, IdaSearchResult::NextBound(_)) {
                    assert_eq!(actual.path, initial_path, "restore the caller's prefix");
                }
                if scenario == 7 {
                    assert_eq!(actual.result, IdaSearchResult::Timeout);
                    assert_eq!(actual.checked, 2048);
                }
            }
        }
    }
}

#[test]
fn safe_chain_checks_the_intermediate_f_score_and_restores_signatures() {
    let mut board = KlondikeBoard::new();
    board.foundation = [2, 2, 2, 0];
    board.columns[0].push(Card::new(Suit::Club, 4), true);
    board.columns[0].push(Card::new(Suit::Club, 3), false);
    board.columns[1].push(Card::new(Suit::Spade, 10), true);
    board.columns[1].push(Card::new(Suit::Diamond, 11), false);
    board.stock[0] = Card::new(Suit::Spade, 9);
    board.stock_len = 1;
    board.compute_signature();
    for draw in [1, 3] {
        let solver = KlondikeSolver::new(draw);
        let transitions = solver.successors(&board, None, 0);
        assert_eq!(transitions.len(), 1);
        assert!(is_safe_foundation_move(&transitions[0], &board));
        let bound = BoardEval::from_board_fast(&board, draw as u8).heuristic_cost();
        let next_f =
            1 + BoardEval::from_board_fast(&transitions[0].to_board, draw as u8).heuristic_cost();
        assert!(next_f > bound);
        let mut ctx = context(draw as u8);
        let mut path = vec![];
        let mut signatures = AHashSet::from_iter([board.signature]);
        let result = solver.search_with_bound(
            &board,
            0,
            bound,
            &mut path,
            0,
            &mut signatures,
            &mut AHashMap::new(),
            &mut ctx,
        );
        assert_eq!(result, IdaSearchResult::NextBound(next_f));
        assert_eq!(ctx.checked_nodes, 2);
        assert_eq!(ctx.progress_sample_counter, 1);
        assert!(path.is_empty());
        assert_eq!(signatures, AHashSet::from_iter([board.signature]));
    }
}

#[test]
fn safe_foundation_fast_path_respects_depth_even_at_the_root() {
    let board = forced_chain();
    for draw in [1, 3] {
        for mode in [KlondikeSolveMode::Fast, KlondikeSolveMode::Strict] {
            let mut solver = KlondikeSolver::new_with_mode(draw, mode);
            solver.rules.push(KlondikeRule::DepthLimit { max_depth: 0 });
            assert!(solver.successors(&board, None, 0).is_empty());
            assert!(solver.solve(&board, false).is_none());
        }
    }
}

#[test]
fn safe_stock_chain_replays_primitive_moves_and_checks_the_budget_before_victory() {
    let mut board = KlondikeBoard::new();
    board.foundation = [10, 13, 13, 13];
    for (i, rank) in [13, 12, 11].into_iter().enumerate() {
        board.stock[i] = Card::new(Suit::Club, rank);
    }
    board.stock_len = 3;
    board.stock_index = 3;
    board.compute_signature();
    for draw in [1, 3] {
        for mode in [KlondikeSolveMode::Fast, KlondikeSolveMode::Strict] {
            let mut solver = KlondikeSolver::new_with_mode(draw, mode);
            solver.max_undos = Some(0);
            for limit in [4, 5] {
                let mut ctx = context(draw as u8);
                ctx.checkpoint_node_limit = limit;
                let mut path = vec![];
                let mut signatures = AHashSet::from_iter([board.signature]);
                let result = solver.search_with_bound(
                    &board,
                    0,
                    3,
                    &mut path,
                    0,
                    &mut signatures,
                    &mut AHashMap::new(),
                    &mut ctx,
                );
                assert_eq!(ctx.checked_nodes, 4);
                assert_eq!(ctx.checkpoint_nodes, 4);
                assert_eq!(ctx.progress_sample_counter, 3);
                assert_eq!(path.len(), 3);
                assert!(KlondikeSolver::is_win(&replay(board, &path, draw as u8)));
                if limit == 4 {
                    assert_eq!(result, IdaSearchResult::CheckpointTriggered);
                    assert!(ctx.solution_path.is_none());
                } else {
                    assert_eq!(result, IdaSearchResult::Found);
                    assert_eq!(ctx.solution_path, Some(path));
                }
            }
        }
    }
}
