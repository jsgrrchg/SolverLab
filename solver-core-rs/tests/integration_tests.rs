//! Pruebas de integración para los 5 solvers de juegos.

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
// Pruebas de integración de Spider
// ═══════════════════════════════════════════

// Verifica que Spider reparte un tablero válido y expone operaciones básicas.
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

// Verifica que Spider rechaza un mazo con cantidad de cartas incorrecta.
#[test]
fn spider_deal_wrong_deck_size() {
    let engine = SpiderEngine::new(1);
    let short_deck = vec![Card::new(Suit::Spade, 1); 52];
    assert!(engine.deal(short_deck).is_err());
}

// Verifica que el progress_token tiene los 6 campos esperados y valores correctos al inicio.
#[test]
fn spider_progress_token_has_expected_fields() {
    let engine = SpiderEngine::new(1);
    let deck = spider_deck(1);
    let board = engine.deal(deck).expect("deal");
    let token = engine.progress_token(&board);

    // Formato esperado: c:N|u:N|d:N|r:N|s:N|e:N
    for field in &["c:", "u:", "d:", "r:", "s:", "e:"] {
        assert!(
            token.contains(field),
            "token missing field '{}': {}",
            field,
            token
        );
    }

    // Al inicio: 0 sets completados, 50 cartas en stock, 0 columnas vacías.
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

// Verifica que apply_move acepta movimientos válidos y rechaza inválidos.
#[test]
fn spider_apply_move_valid_and_invalid() {
    let engine = SpiderEngine::new(1);
    let deck = spider_deck(1);
    let board = engine.deal(deck).expect("deal");

    // dealFromStock es válido en el tablero inicial (todas las columnas tienen cartas).
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

    // El tablero debe haber cambiado (stock se redujo en 10).
    let after = after_deal.unwrap();
    assert_ne!(
        engine.progress_token(&after),
        engine.progress_token(&board),
        "board should change after dealFromStock"
    );

    // Movimiento inválido: tipo desconocido → None.
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

    // Movimiento inválido: columna de origen fuera de rango → None.
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

    // Movimiento inválido: card_count = 0 → None.
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

// Verifica que last_checkpoints_adopted es > 0 después de un solve real.
#[test]
fn spider_last_checkpoints_adopted_nonzero_after_solve() {
    let engine = SpiderEngine::new(1);
    let deck = spider_deck(1);
    let board = engine.deal(deck).expect("deal");

    let _ = engine.solve(&board, true);

    // Un tablero real (no ganado inmediatamente) debe adoptar al menos 1 checkpoint.
    assert!(
        engine.last_checkpoints_adopted() > 0,
        "solver should adopt at least one checkpoint on a real board"
    );
}

// Verifica que Spider puede ejecutar `solve` con timeout corto sin crashear (1 palo).
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

// Verifica que Spider funciona con 2 palos: deal, solve y replay correctos.
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

// Verifica que Spider funciona con 4 palos: deal, solve y replay correctos.
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
// Pruebas de integración de Klondike
// ═══════════════════════════════════════════

// Verifica que Klondike reparte un tablero válido y genera token de progreso.
#[test]
fn klondike_deal_and_basic_ops() {
    let engine = KlondikeEngine::new(3);
    let deck = standard_deck();
    let board = engine.deal(deck).expect("deal should succeed");
    assert!(!engine.is_win(&board));
    let token = engine.progress_token(&board);
    assert!(!token.is_empty());
}

// Verifica que Klondike rechaza un mazo con tamaño inválido.
#[test]
fn klondike_deal_wrong_deck_size() {
    let engine = KlondikeEngine::new(3);
    let short = vec![Card::new(Suit::Spade, 1); 10];
    assert!(engine.deal(short).is_err());
}

// Verifica que los movimientos a foundation detectados al inicio sean legales y aplicables.
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
// Pruebas de integración de FreeCell
// ═══════════════════════════════════════════

// Verifica que FreeCell reparte correctamente y el tablero inicial no es victoria.
#[test]
fn freecell_deal_and_basic_ops() {
    let engine = FreeCellEngine::new();
    let deck = standard_deck();
    let board = engine.deal(deck).expect("deal should succeed");
    assert!(!engine.is_win(&board));
}

// Verifica que FreeCell rechaza un mazo de tamaño incorrecto.
#[test]
fn freecell_deal_wrong_deck_size() {
    let engine = FreeCellEngine::new();
    let short = vec![Card::new(Suit::Spade, 1); 20];
    assert!(engine.deal(short).is_err());
}

// ═══════════════════════════════════════════
// Pruebas de integración de Pyramid
// ═══════════════════════════════════════════

// Verifica que Pyramid reparte bien y devuelve un token de progreso válido.
#[test]
fn pyramid_deal_and_basic_ops() {
    let engine = PyramidEngine::new();
    let deck = standard_deck();
    let board = engine.deal(deck).expect("deal should succeed");
    assert!(!engine.is_win(&board));
    let token = engine.progress_token(&board);
    assert!(!token.is_empty());
}

// Verifica que Pyramid rechaza un mazo con cantidad inválida de cartas.
#[test]
fn pyramid_deal_wrong_deck_size() {
    let engine = PyramidEngine::new();
    let short = vec![Card::new(Suit::Spade, 1); 20];
    assert!(engine.deal(short).is_err());
}

// Verifica que los movimientos greedy de Pyramid sean de rey o pareja válidos.
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
// Pruebas de integración de TriPeaks
// ═══════════════════════════════════════════

// Verifica que TriPeaks reparte correctamente y reporta cartas restantes en tableau.
#[test]
fn tripeaks_deal_and_basic_ops() {
    let engine = TriPeaksEngine::new();
    let deck = standard_deck();
    let board = engine.deal(deck).expect("deal should succeed");
    assert!(!engine.is_win(&board));
    let remaining = engine.remaining_tableau(&board);
    assert!(remaining > 0);
}

// Verifica que TriPeaks rechaza un mazo con tamaño inválido.
#[test]
fn tripeaks_deal_wrong_deck_size() {
    let engine = TriPeaksEngine::new();
    let short = vec![Card::new(Suit::Spade, 1); 20];
    assert!(engine.deal(short).is_err());
}

// ═══════════════════════════════════════════
// Pruebas de solve (timeouts cortos para verificar que no crashee)
// ═══════════════════════════════════════════

// Verifica que Klondike devuelve una secuencia reaplicable y con progreso observable si es parcial.
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

// Verifica en modos Fast y Strict que cualquier secuencia devuelta por Klondike sea reaplicable.
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

// Verifica que FreeCell puede ejecutar `solve` con timeout corto sin errores.
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

// Verifica que Pyramid puede ejecutar `solve` con timeout corto y que el resultado es coherente.
#[test]
fn pyramid_solve_short_timeout() {
    let engine = PyramidEngine::new();
    let deck = standard_deck();
    let board = engine.deal(deck).expect("deal");
    let result = engine.solve(&board, 0.1, true);
    if let Some(moves) = result {
        // Una solución vacía solo es válida si el tablero ya está ganado.
        if moves.is_empty() {
            assert!(
                engine.is_win(&board),
                "empty solution only valid if board is already won"
            );
        }
        // El token de progreso siempre debe tener el campo 'r:' (restantes en pirámide).
        let token = engine.progress_token(&board);
        assert!(
            token.contains("r:"),
            "progress token should have 'r:' field: {}",
            token
        );
    }
}

// Verifica que TriPeaks puede ejecutar `solve` con timeout corto y que el resultado es coherente.
#[test]
fn tripeaks_solve_short_timeout() {
    let engine = TriPeaksEngine::new();
    let deck = standard_deck();
    let board = engine.deal(deck).expect("deal");
    let start_token = engine.progress_token(&board);
    let result = engine.solve(&board, true);
    if let Some(moves) = result {
        // Una solución vacía solo es válida si el tablero ya está ganado.
        if moves.is_empty() {
            assert!(engine.is_win(&board));
        } else {
            // Los movimientos deben poder reproducirse y el tablero debe cambiar.
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
// Pruebas de descriptores de movimiento
// ═══════════════════════════════════════════

// Verifica la clasificación de movimientos de Klondike (a/from foundation y a columna).
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

// Verifica la detección de movimientos a foundation en FreeCell.
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

// Verifica la clasificación de movimientos de Pyramid (rey y pareja).
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

// Verifica la clasificación de movimientos de TriPeaks entre tableau y robo de stock.
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
