import Foundation

#if canImport(SolverCoreBridge)
    import SolverCoreBridge
#endif

extension SimulationViewModel {
    nonisolated func applyFreeCellMove(
        _ move: FreeCellMoveDesc,
        engine: FreeCellEngine,
        board: inout FreeCellBoard,
        score: inout Int,
        appliedMoves: inout Int
    ) -> Bool {
        // Aplica la jugada sobre el tablero actual; si falla, se corta la secuencia.
        guard let nextBoard = engine.applyMove(board: board, moveDesc: move) else { return false }
        board = nextBoard
        appliedMoves += 1
        // Sistema de puntaje estilo FreeCell: +10 por jugadas hacia foundation.
        if engine.isFoundationMove(m: move) {
            score += 10
        }
        return true
    }

    nonisolated func runFreeCell(gameID: Int, config: SimConfig) -> SimResult {
        // Construcción de mazo estándar de 52 cartas y barajado por partida.
        var deck: [Card] = []
        let suits: [Suit] = [.club, .diamond, .heart, .spade]
        for suit in suits {
            for v in 1...13 {
                deck.append(Card(suit: suit, value: UInt8(v)))
            }
        }
        deck.shuffle()

        let engine = FreeCellEngine()

        // Reparte el tablero inicial; si falla, devuelve resultado estancado.
        var startBoard: FreeCellBoard?
        do {
            startBoard = try engine.deal(deck: deck)
        } catch {}

        guard let board = startBoard else {
            return SimResult(
                id: gameID, moveCount: 0, undoCount: 0, checkpointCount: nil, won: false,
                stopReason: .stalled, duration: 0, score: 0, movesDetail: "deal_failed")
        }

        let start = Date()
        let timeoutBudget = config.timeoutSeconds > 0 ? config.timeoutSeconds : nil

        var finalBoard = board
        var score = 0
        var appliedMoves = 0
        var timedOut = false

        // El solver en Rust gestiona internamente checkpoints y retorno parcial.
        let solveTimeout = timeoutBudget ?? 15.0
        let plannedMoves = engine.solve(
            board: finalBoard,
            timeoutSecs: solveTimeout,
            allowPartial: true)
        let checkpoints = Int(engine.lastCheckpointsAdopted())

        // Reproduce la secuencia entregada por Rust para obtener tablero final y score UI.
        if let plannedMoves = plannedMoves {
            for move in plannedMoves {
                let applied = applyFreeCellMove(
                    move,
                    engine: engine,
                    board: &finalBoard,
                    score: &score,
                    appliedMoves: &appliedMoves)
                if !applied {
                    break
                }
            }
        }

        let duration = Date().timeIntervalSince(start)
        let won = engine.isWin(board: finalBoard)
        // Verifica timeout global al finalizar la ejecución/replay de jugadas.
        if let timeoutBudget, duration >= timeoutBudget {
            timedOut = true
        }
        // Timeout solo se reporta si hubo progreso útil; si no, se considera stalled.
        let hasUsefulProgress =
            engine.progressToken(board: finalBoard) != engine.progressToken(board: board)
        let stopReason: SimStopReason =
            won ? .win : ((timedOut && hasUsefulProgress) ? .timeout : .stalled)

        // Resultado final consolidado de la partida simulada.
        return SimResult(
            id: gameID, moveCount: appliedMoves, undoCount: 0, checkpointCount: checkpoints,
            won: won,
            stopReason: stopReason, duration: duration, score: score, movesDetail: "")
    }
}
