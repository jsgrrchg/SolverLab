import Foundation

#if canImport(SolverCoreBridge)
    import SolverCoreBridge
#endif

extension SimulationViewModel {
    nonisolated func runPyramid(gameID: Int, config: SimConfig) -> SimResult {
        var deck: [Card] = []
        let suits: [Suit] = [.club, .diamond, .heart, .spade]
        for suit in suits {
            for v in 1...13 {
                deck.append(Card(suit: suit, value: UInt8(v)))
            }
        }
        deck.shuffle()

        let engine = PyramidEngine()

        var startBoard: PyramidBoard?
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

        func applyPyramidMove(_ move: PyramidMoveDesc) -> Bool {
            guard let nextBoard = engine.applyMove(board: finalBoard, moveDesc: move) else {
                return false
            }
            finalBoard = nextBoard

            let isPair = move.moveType == "pairWP" || move.moveType == "pairPP"
            if move.moveType == "kingPyramid" || move.moveType == "kingWaste" {
                score += 10
            } else if isPair {
                score += 20
            }

            appliedMoves += 1
            return true
        }

        let solveTimeout = timeoutBudget ?? 0.0
        let plannedMoves = engine.solve(
            board: finalBoard,
            timeoutSecs: solveTimeout,
            allowPartial: true)
        let checkpoints = Int(engine.lastCheckpointsAdopted())

        if let plannedMoves = plannedMoves {
            for move in plannedMoves {
                if !applyPyramidMove(move) {
                    break
                }
            }
        }

        let duration = Date().timeIntervalSince(start)
        let won = engine.isWin(board: finalBoard)
        if let timeoutBudget, duration >= timeoutBudget {
            timedOut = true
        }
        let hasUsefulProgress =
            engine.progressToken(board: finalBoard) != engine.progressToken(board: board)
        let stopReason: SimStopReason =
            won ? .win : ((timedOut && hasUsefulProgress) ? .timeout : .stalled)

        return SimResult(
            id: gameID, moveCount: appliedMoves, undoCount: 0, checkpointCount: checkpoints,
            won: won, stopReason: stopReason, duration: duration, score: score, movesDetail: "")
    }
}
