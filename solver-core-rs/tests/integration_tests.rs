//! Integration tests for the 5 game solvers.

use solver_core::common::card::{Card, Suit};
use solver_core::*;
use std::sync::Arc;

// ═══════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════

fn standard_deck() -> Vec<Card> {
    Card::standard_deck()
}

fn spider_deck(suits: u32) -> Vec<Card> {
    Card::spider_deck(suits)
}

fn parse_klondike_progress_token(token: &str) -> Option<(i32, i32, i32, i32)> {
    let mut f = None;
    let mut u = None;
    let mut s = None;
    let mut w = None;
    for part in token.split('|') {
        let (k, v) = part.split_once(':')?;
        let parsed = v.parse::<i32>().ok()?;
        match k {
            "f" => f = Some(parsed),
            "u" => u = Some(parsed),
            "s" => s = Some(parsed),
            "w" => w = Some(parsed),
            _ => return None,
        }
    }
    Some((f?, u?, s?, w?))
}

fn replay_klondike_moves(
    engine: &KlondikeEngine,
    mut board: Arc<KlondikeBoard>,
    moves: &[KlondikeMoveDesc],
) -> Option<Arc<KlondikeBoard>> {
    for m in moves {
        board = engine.apply_move(&board, m.clone())?;
    }
    Some(board)
}

fn replay_freecell_moves(
    engine: &FreeCellEngine,
    mut board: Arc<FreeCellBoard>,
    moves: &[FreeCellMoveDesc],
) -> Option<Arc<FreeCellBoard>> {
    for m in moves {
        board = engine.apply_move(&board, m.clone())?;
    }
    Some(board)
}

fn replay_spider_moves(
    engine: &SpiderEngine,
    mut board: Arc<SpiderBoard>,
    moves: &[SpiderMoveDesc],
) -> Option<Arc<SpiderBoard>> {
    for m in moves {
        board = engine.apply_move(&board, m.clone())?;
    }
    Some(board)
}

fn replay_tripeaks_moves(
    engine: &TriPeaksEngine,
    mut board: Arc<TriPeaksBoard>,
    moves: &[TriPeaksMoveDesc],
) -> Option<Arc<TriPeaksBoard>> {
    for m in moves {
        board = engine.apply_move(&board, m.clone())?;
    }
    Some(board)
}

// ═══════════════════════════════════════════
// Spider integration tests
// ═══════════════════════════════════════════

// Verifies that Spider deals a valid board and exposes basic operations.
#[test]
fn spider_deal_and_basic_ops() {
    let engine = SpiderEngine::new(1);
    let deck = spider_deck(1);
    let board = engine.deal(deck).expect("deal should succeed");
    assert!(!engine.is_win(&board));
    let token = engine.progress_token(&board);
    assert!(!token.is_empty());
    assert_eq!(engine.completed_sets(&board), 0);
}

// Verifies that Spider rejects a deck with the wrong number of cards.
#[test]
fn spider_deal_wrong_deck_size() {
    let engine = SpiderEngine::new(1);
    let short_deck = vec![Card::new(Suit::Spade, 1); 52];
    assert!(engine.deal(short_deck).is_err());
}

// Verifies that progress_token has the 6 expected fields and correct initial values.
#[test]
fn spider_progress_token_has_expected_fields() {
    let engine = SpiderEngine::new(1);
    let deck = spider_deck(1);
    let board = engine.deal(deck).expect("deal");
    let token = engine.progress_token(&board);

    // Expected format: c:N|u:N|d:N|r:N|s:N|e:N
    for field in &["c:", "u:", "d:", "r:", "s:", "e:"] {
        assert!(
            token.contains(field),
            "token missing field '{}': {}",
            field,
            token
        );
    }

    // Initially: 0 completed sets, 50 cards in stock, 0 empty columns.
    assert!(
        token.contains("c:0"),
        "should start with 0 completed sets: {}",
        token
    );
    assert!(
        token.contains("s:50"),
        "should start with 50 cards in stock: {}",
        token
    );
    assert!(
        token.contains("e:0"),
        "should start with 0 empty columns: {}",
        token
    );
}

// Verifies that apply_move accepts valid moves and rejects invalid ones.
#[test]
fn spider_apply_move_valid_and_invalid() {
    let engine = SpiderEngine::new(1);
    let deck = spider_deck(1);
    let board = engine.deal(deck).expect("deal");

    // dealFromStock is valid on the initial board (all columns have cards).
    let deal_move = SpiderMoveDesc {
        move_type: "dealFromStock".into(),
        source: -1,
        destination: -1,
        card_count: 0,
    };
    let after_deal = engine.apply_move(&board, deal_move);
    assert!(
        after_deal.is_some(),
        "dealFromStock should succeed on initial board"
    );

    // The board should have changed (stock was reduced by 10).
    let after = after_deal.unwrap();
    assert_ne!(
        engine.progress_token(&after),
        engine.progress_token(&board),
        "board should change after dealFromStock"
    );

    // Invalid move: unknown type -> None.
    let bad_type = SpiderMoveDesc {
        move_type: "teleport".into(),
        source: 0,
        destination: 1,
        card_count: 1,
    };
    assert!(
        engine.apply_move(&board, bad_type).is_none(),
        "unknown move type should return None"
    );

    // Invalid move: source column out of range -> None.
    let bad_source = SpiderMoveDesc {
        move_type: "columnToColumn".into(),
        source: 99,
        destination: 1,
        card_count: 1,
    };
    assert!(
        engine.apply_move(&board, bad_source).is_none(),
        "out-of-bounds source column should return None"
    );

    // Invalid move: card_count = 0 -> None.
    let zero_cards = SpiderMoveDesc {
        move_type: "columnToColumn".into(),
        source: 0,
        destination: 1,
        card_count: 0,
    };
    assert!(
        engine.apply_move(&board, zero_cards).is_none(),
        "zero card_count should return None"
    );
}

// Verifies that last_checkpoints_adopted is > 0 after a real solve.
#[test]
fn spider_last_checkpoints_adopted_nonzero_after_solve() {
    let engine = SpiderEngine::new(1);
    let deck = spider_deck(1);
    let board = engine.deal(deck).expect("deal");

    let _ = engine.solve(&board, true);

    // A real board (not immediately won) should adopt at least 1 checkpoint.
    assert!(
        engine.last_checkpoints_adopted() > 0,
        "solver should adopt at least one checkpoint on a real board"
    );
}

// Verifies that Spider can run `solve` with a short timeout without crashing (1 suit).
#[test]
fn spider_solve_short_timeout() {
    let engine = SpiderEngine::new(1);
    let deck = spider_deck(1);
    let board = engine.deal(deck).expect("deal");
    let start_token = engine.progress_token(&board);
    let result = engine.solve(&board, true);
    if let Some(moves) = result {
        let final_board = replay_spider_moves(&engine, Arc::clone(&board), &moves)
            .expect("solver returned a replayable sequence");
        if moves.is_empty() {
            assert!(engine.is_win(&final_board));
        } else if !engine.is_win(&final_board) {
            assert_ne!(
                engine.progress_token(&final_board),
                start_token,
                "partial solution should produce observable progress"
            );
        }
    }
}

// Verifies that Spider works with 2 suits: correct deal, solve, and replay.
#[test]
fn spider_solve_2suit_short_timeout() {
    let engine = SpiderEngine::new(2);
    let deck = spider_deck(2);
    let board = engine.deal(deck).expect("deal 2-suit");
    let start_token = engine.progress_token(&board);
    let result = engine.solve(&board, true);
    if let Some(moves) = result {
        let final_board = replay_spider_moves(&engine, Arc::clone(&board), &moves)
            .expect("2-suit solver returned a replayable sequence");
        if moves.is_empty() {
            assert!(engine.is_win(&final_board));
        } else if !engine.is_win(&final_board) {
            assert_ne!(
                engine.progress_token(&final_board),
                start_token,
                "2-suit partial solution should produce observable progress"
            );
        }
    }
}

// Verifies that Spider works with 4 suits: correct deal, solve, and replay.
#[test]
fn spider_solve_4suit_short_timeout() {
    let engine = SpiderEngine::new(4);
    let deck = spider_deck(4);
    let board = engine.deal(deck).expect("deal 4-suit");
    let start_token = engine.progress_token(&board);
    let result = engine.solve(&board, true);
    if let Some(moves) = result {
        let final_board = replay_spider_moves(&engine, Arc::clone(&board), &moves)
            .expect("4-suit solver returned a replayable sequence");
        if moves.is_empty() {
            assert!(engine.is_win(&final_board));
        } else if !engine.is_win(&final_board) {
            assert_ne!(
                engine.progress_token(&final_board),
                start_token,
                "4-suit partial solution should produce observable progress"
            );
        }
    }
}

// ═══════════════════════════════════════════
// Klondike integration tests
// ═══════════════════════════════════════════

// Verifies that Klondike deals a valid board and generates a progress token.
#[test]
fn klondike_deal_and_basic_ops() {
    let engine = KlondikeEngine::new(3);
    let deck = standard_deck();
    let board = engine.deal(deck).expect("deal should succeed");
    assert!(!engine.is_win(&board));
    let token = engine.progress_token(&board);
    assert!(!token.is_empty());
}

// Verifies that Klondike rejects a deck with an invalid size.
#[test]
fn klondike_deal_wrong_deck_size() {
    let engine = KlondikeEngine::new(3);
    let short = vec![Card::new(Suit::Spade, 1); 10];
    assert!(engine.deal(short).is_err());
}

// Verifies that initial foundation moves are legal and applicable.
#[test]
fn klondike_find_foundation_moves_initial() {
    let engine = KlondikeEngine::new(3);
    let deck = standard_deck();
    let board = engine.deal(deck).expect("deal");
    let moves = engine.find_foundation_moves(&board);
    for m in &moves {
        assert!(engine.is_to_foundation(m.clone()));
        assert!(engine.apply_move(&board, m.clone()).is_some());
    }
}

// ═══════════════════════════════════════════
// FreeCell integration tests
// ═══════════════════════════════════════════

// Verifies that FreeCell deals correctly and the initial board is not a win.
#[test]
fn freecell_deal_and_basic_ops() {
    let engine = FreeCellEngine::new();
    let deck = standard_deck();
    let board = engine.deal(deck).expect("deal should succeed");
    assert!(!engine.is_win(&board));
}

// Verifies that FreeCell rejects a deck with the wrong size.
#[test]
fn freecell_deal_wrong_deck_size() {
    let engine = FreeCellEngine::new();
    let short = vec![Card::new(Suit::Spade, 1); 20];
    assert!(engine.deal(short).is_err());
}

// ═══════════════════════════════════════════
// Pyramid integration tests
// ═══════════════════════════════════════════

// Verifies that Pyramid deals correctly and returns a valid progress token.
#[test]
fn pyramid_deal_and_basic_ops() {
    let engine = PyramidEngine::new();
    let deck = standard_deck();
    let board = engine.deal(deck).expect("deal should succeed");
    assert!(!engine.is_win(&board));
    let token = engine.progress_token(&board);
    assert!(!token.is_empty());
}

// Verifies that Pyramid rejects a deck with an invalid number of cards.
#[test]
fn pyramid_deal_wrong_deck_size() {
    let engine = PyramidEngine::new();
    let short = vec![Card::new(Suit::Spade, 1); 20];
    assert!(engine.deal(short).is_err());
}

// Verifies that Pyramid greedy moves are valid king or pair moves.
#[test]
fn pyramid_greedy_moves() {
    let engine = PyramidEngine::new();
    let deck = standard_deck();
    let board = engine.deal(deck).expect("deal");
    let greedy = engine.find_greedy_moves(&board);
    for m in &greedy {
        assert!(engine.is_king_move(m.clone()) || engine.is_pair_move(m.clone()));
    }
}

// ═══════════════════════════════════════════
// TriPeaks integration tests
// ═══════════════════════════════════════════

// Verifies that TriPeaks deals correctly and reports remaining tableau cards.
#[test]
fn tripeaks_deal_and_basic_ops() {
    let engine = TriPeaksEngine::new();
    let deck = standard_deck();
    let board = engine.deal(deck).expect("deal should succeed");
    assert!(!engine.is_win(&board));
    let remaining = engine.remaining_tableau(&board);
    assert!(remaining > 0);
}

// Verifies that TriPeaks rejects a deck with an invalid size.
#[test]
fn tripeaks_deal_wrong_deck_size() {
    let engine = TriPeaksEngine::new();
    let short = vec![Card::new(Suit::Spade, 1); 20];
    assert!(engine.deal(short).is_err());
}

// ═══════════════════════════════════════════
// Solve tests (short timeouts to verify that nothing crashes)
// ═══════════════════════════════════════════

// Verifies that Klondike returns a replayable sequence with observable progress if partial.
#[test]
fn klondike_solve_short_timeout() {
    let engine = KlondikeEngine::new(1);
    let deck = standard_deck();
    let board = engine.deal(deck).expect("deal");
    let start_token = engine.progress_token(&board);
    let (start_f, start_u, _, _) =
        parse_klondike_progress_token(&start_token).expect("valid progress token");
    let result = engine.solve(&board, true);
    if let Some(moves) = result {
        let final_board = replay_klondike_moves(&engine, Arc::clone(&board), &moves)
            .expect("solver returned a replayable sequence");
        if moves.is_empty() {
            assert!(engine.is_win(&final_board));
        } else if !engine.is_win(&final_board) {
            let end_token = engine.progress_token(&final_board);
            let (end_f, end_u, _, _) =
                parse_klondike_progress_token(&end_token).expect("valid progress token");
            assert!(
                end_f > start_f || end_u > start_u || end_token != start_token,
                "partial solution should produce observable progress"
            );
        }
    }
}

// Verifies in Fast and Strict modes that any sequence returned by Klondike is replayable.
#[test]
fn klondike_solve_fast_and_strict_modes_replay_if_any() {
    let deck = standard_deck();
    for mode in [KlondikeSearchMode::Fast, KlondikeSearchMode::Strict] {
        let engine = KlondikeEngine::new_with_mode(1, mode);
        let board = engine.deal(deck.clone()).expect("deal");
        let start_token = engine.progress_token(&board);
        let result = engine.solve(&board, true);
        if let Some(moves) = result {
            let final_board = replay_klondike_moves(&engine, Arc::clone(&board), &moves)
                .expect("solver returned a replayable sequence");
            if !moves.is_empty() && !engine.is_win(&final_board) {
                assert_ne!(engine.progress_token(&final_board), start_token);
            }
        }
    }
}

// Verifies that FreeCell can run `solve` with a short timeout without errors.
#[test]
fn freecell_solve_short_timeout() {
    let engine = FreeCellEngine::new();
    let deck = standard_deck();
    let board = engine.deal(deck).expect("deal");
    let result = engine.solve(&board, 0.1, true);
    if let Some(moves) = result {
        let final_board = replay_freecell_moves(&engine, Arc::clone(&board), &moves)
            .expect("solver returned a replayable sequence");
        if moves.is_empty() {
            assert!(engine.is_win(&final_board));
        }
    }
}

// Verifies that Pyramid can run `solve` with a short timeout and a coherent result.
#[test]
fn pyramid_solve_short_timeout() {
    let engine = PyramidEngine::new();
    let deck = standard_deck();
    let board = engine.deal(deck).expect("deal");
    let result = engine.solve(&board, 0.1, true);
    if let Some(moves) = result {
        // An empty solution is only valid if the board is already won.
        if moves.is_empty() {
            assert!(
                engine.is_win(&board),
                "empty solution only valid if board is already won"
            );
        }
        // The progress token should always have the 'r:' field (remaining in pyramid).
        let token = engine.progress_token(&board);
        assert!(
            token.contains("r:"),
            "progress token should have 'r:' field: {}",
            token
        );
    }
}

// Verifies that TriPeaks can run `solve` with a short timeout and a coherent result.
#[test]
fn tripeaks_solve_short_timeout() {
    let engine = TriPeaksEngine::new();
    let deck = standard_deck();
    let board = engine.deal(deck).expect("deal");
    let start_token = engine.progress_token(&board);
    let result = engine.solve(&board, true);
    if let Some(moves) = result {
        // An empty solution is only valid if the board is already won.
        if moves.is_empty() {
            assert!(engine.is_win(&board));
        } else {
            // Moves should be replayable and the board should change.
            let final_board = replay_tripeaks_moves(&engine, Arc::clone(&board), &moves)
                .expect("tripeaks solver returned a replayable sequence");
            if !engine.is_win(&final_board) {
                assert_ne!(
                    engine.progress_token(&final_board),
                    start_token,
                    "partial tripeaks solution should produce observable progress"
                );
            }
        }
    }
}

// ═══════════════════════════════════════════
// Move descriptor tests
// ═══════════════════════════════════════════

// Verifies Klondike move classification (to/from foundation and to column).
#[test]
fn klondike_move_classification() {
    let engine = KlondikeEngine::new(3);
    let desc = KlondikeMoveDesc {
        move_type: "columnToFoundation".into(),
        source: 0,
        destination: -1,
        card_count: 1,
        card_suit: 0,
        card_value: 1,
    };
    assert!(engine.is_to_foundation(desc.clone()));
    assert!(!engine.is_from_foundation(desc.clone()));
    assert!(!engine.is_to_column(desc));
}

// Verifies foundation move detection in FreeCell.
#[test]
fn freecell_move_classification() {
    let engine = FreeCellEngine::new();
    let desc = FreeCellMoveDesc {
        move_type: "tableauToFoundation".into(),
        source: 0,
        destination: -1,
        card_count: 1,
        card_suit: 0,
        card_value: 1,
    };
    assert!(engine.is_foundation_move(desc));
}

// Verifies Pyramid move classification (king and pair).
#[test]
fn pyramid_move_classification() {
    let engine = PyramidEngine::new();
    let king = PyramidMoveDesc {
        move_type: "kingPyramid".into(),
        index_a: 0,
        index_b: -1,
    };
    assert!(engine.is_king_move(king));
    let pair = PyramidMoveDesc {
        move_type: "pairPP".into(),
        index_a: 3,
        index_b: 7,
    };
    assert!(engine.is_pair_move(pair));
}

// Verifies TriPeaks move classification between tableau and stock draw.
#[test]
fn tripeaks_move_classification() {
    let engine = TriPeaksEngine::new();
    let tab = TriPeaksMoveDesc {
        move_type: "tableauToWaste".into(),
        tableau_index: 5,
    };
    assert!(engine.is_tableau_move(tab));
    let draw = TriPeaksMoveDesc {
        move_type: "drawFromStock".into(),
        tableau_index: -1,
    };
    assert!(!engine.is_tableau_move(draw));
}
