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
        // Apply the move to the current board; stop the sequence if it fails.
        guard let nextBoard = engine.applyMove(board: board, moveDesc: move) else { return false }
        board = nextBoard
        appliedMoves += 1
        // FreeCell-style scoring system: +10 for moves to foundation.
        if engine.isFoundationMove(m: move) {
            score += 10
        }
        return true
    }

    nonisolated func runFreeCell(gameID: Int, config: SimConfig) -> SimResult {
        // Build a standard 52-card deck and shuffle it per game.
        var deck: [Card] = []
        let suits: [Suit] = [.club, .diamond, .heart, .spade]
        for suit in suits {
            for v in 1...13 {
                deck.append(Card(suit: suit, value: UInt8(v)))
            }
        }
        deck.shuffle()

        let engine = FreeCellEngine()

        // Deal the initial board; return a stalled result if it fails.
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

        // The Rust solver manages checkpoints and partial returns internally.
        let solveTimeout = timeoutBudget ?? 15.0
        let plannedMoves = engine.solve(
            board: finalBoard,
            timeoutSecs: solveTimeout,
            allowPartial: true)
        let checkpoints = Int(engine.lastCheckpointsAdopted())

        // Replay the sequence returned by Rust to get the final board and UI score.
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
        // Check the global timeout after execution/replay finishes.
        if let timeoutBudget, duration >= timeoutBudget {
            timedOut = true
        }
        // Timeout is reported only if useful progress was made; otherwise this is stalled.
        let hasUsefulProgress =
            engine.progressToken(board: finalBoard) != engine.progressToken(board: board)
        let stopReason: SimStopReason =
            won ? .win : ((timedOut && hasUsefulProgress) ? .timeout : .stalled)

        // Consolidated final result for the simulated game.
        return SimResult(
            id: gameID, moveCount: appliedMoves, undoCount: 0, checkpointCount: checkpoints,
            won: won,
            stopReason: stopReason, duration: duration, score: score, movesDetail: "")
    }
}
