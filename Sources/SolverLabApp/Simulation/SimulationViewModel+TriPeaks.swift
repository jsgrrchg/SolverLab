import Foundation

#if canImport(SolverCoreBridge)
    import SolverCoreBridge
#endif

extension SimulationViewModel {
    nonisolated func applyTriPeaksMove(
        _ move: TriPeaksMoveDesc,
        engine: TriPeaksEngine,
        board: inout TriPeaksBoard,
        score: inout Int,
        streak: inout Int,
        appliedMoves: inout Int
    ) -> Bool {
        guard let nextBoard = engine.applyMove(board: board, moveDesc: move) else { return false }

        if engine.isTableauMove(m: move) {
            streak += 1
            score += 5 * streak
        } else {
            streak = 0
            score = max(0, score - 5)
        }

        board = nextBoard
        appliedMoves += 1
        return true
    }

    nonisolated func runTriPeaks(gameID: Int, config: SimConfig) -> SimResult {
        var deck: [Card] = []
        let suits: [Suit] = [.club, .diamond, .heart, .spade]
        for suit in suits {
            for v in 1...13 {
                deck.append(Card(suit: suit, value: UInt8(v)))
            }
        }
        deck.shuffle()

        let engine = TriPeaksEngine()

        var startBoard: TriPeaksBoard?
        do {
            startBoard = try engine.deal(deck: deck)
        } catch {}

        guard let board = startBoard else {
            return SimResult(
                id: gameID, moveCount: 0, undoCount: 0, checkpointCount: nil, won: false,
                stopReason: .stalled, duration: 0, score: 0, movesDetail: "deal_failed")
        }

        let start = Date()
        var finalBoard = board
        var score = 0
        var streak = 0
        var appliedMoves = 0
        let plannedMoves = engine.solve(
            board: finalBoard,
            allowPartial: true)
        let checkpoints = Int(engine.lastCheckpointsAdopted())

        if let plannedMoves = plannedMoves {
            for move in plannedMoves {
                let applied = applyTriPeaksMove(
                    move,
                    engine: engine,
                    board: &finalBoard,
                    score: &score,
                    streak: &streak,
                    appliedMoves: &appliedMoves)
                if !applied {
                    break
                }
            }
        }

        let duration = Date().timeIntervalSince(start)
        let won = engine.isWin(board: finalBoard)
        let timedOut = duration >= 5.0
        let hasUsefulProgress =
            engine.progressToken(board: finalBoard) != engine.progressToken(board: board)
        let stopReason: SimStopReason =
            won ? .win : ((timedOut && hasUsefulProgress) ? .timeout : .stalled)

        return SimResult(
            id: gameID, moveCount: appliedMoves, undoCount: 0, checkpointCount: checkpoints,
            won: won,
            stopReason: stopReason, duration: duration, score: score, movesDetail: "")
    }
}
