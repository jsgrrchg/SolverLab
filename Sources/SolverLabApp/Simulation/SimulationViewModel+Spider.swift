import Foundation

#if canImport(SolverCoreBridge)
    import SolverCoreBridge
#endif

extension SimulationViewModel {
    nonisolated func runSpider(gameID: Int, suitCount: Int, config: SimConfig) -> SimResult {
        var deck: [Card] = []
        let suits: [Suit] = {
            switch suitCount {
            case 1: return [.spade]
            case 2: return [.spade, .heart]
            default: return [.club, .diamond, .heart, .spade]
            }
        }()
        let repeats = suitCount == 1 ? 8 : (suitCount == 2 ? 4 : 2)
        for _ in 0..<repeats {
            for suit in suits {
                for v in 1...13 {
                    deck.append(Card(suit: suit, value: UInt8(v)))
                }
            }
        }
        deck.shuffle()

        let engine = SpiderEngine(suitCount: UInt32(suitCount))

        var startBoard: SpiderBoard?
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
        var score = 500  // Puntaje inicial Spider
        var appliedMoves = 0
        var timedOut = false

        let plannedMoves = engine.solve(
            board: finalBoard,
            allowPartial: true)
        let checkpoints = Int(engine.lastCheckpointsAdopted())

        if let plannedMoves = plannedMoves {
            for move in plannedMoves {
                let beforeSets = engine.completedSets(board: finalBoard)
                guard let nextBoard = engine.applyMove(board: finalBoard, moveDesc: move) else {
                    break
                }

                finalBoard = nextBoard
                appliedMoves += 1
                score -= 1  // Cada jugada: -1

                let newSets = engine.completedSets(board: finalBoard) - beforeSets
                if newSets > 0 {
                    score += Int(newSets) * 100
                }

                if engine.isWin(board: finalBoard) { break }
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
            won: won,
            stopReason: stopReason, duration: duration, score: score, movesDetail: "")
    }
}
