import Foundation

#if canImport(SolverCoreBridge)
    import SolverCoreBridge
#endif

extension SimulationViewModel {
    nonisolated func applyKlondikeMove(
        _ move: KlondikeMoveDesc,
        engine: KlondikeEngine,
        board: inout KlondikeBoard,
        score: inout Int,
        undoCount: inout Int,
        appliedMoves: inout Int
    ) -> Bool {
        let isFromFoundation = engine.isFromFoundation(m: move)

        let beforeBoard = board
        let isToFoundation = engine.isToFoundation(m: move)
        let isToColumn = engine.isToColumn(m: move)
        let sourceColumn = move.source

        var fdBefore: Int32 = 0
        if sourceColumn >= 0 && sourceColumn < 7 && move.moveType.starts(with: "columnTo") {
            fdBefore = engine.faceDownCount(board: beforeBoard, column: sourceColumn)
        }

        guard let nextBoard = engine.applyMove(board: beforeBoard, moveDesc: move) else {
            return false
        }
        board = nextBoard
        appliedMoves += 1

        if isToFoundation {
            score += 10
        } else if isToColumn {
            if isFromFoundation {
                score = max(0, score - 15)
            } else {
                score += 5
            }
        }

        if isFromFoundation {
            undoCount += 1
        }

        if sourceColumn >= 0 && sourceColumn < 7 && move.moveType.starts(with: "columnTo") {
            let fdAfter = engine.faceDownCount(board: nextBoard, column: sourceColumn)
            if fdBefore >= 1 && fdAfter == fdBefore - 1 {
                score += 5
            }
        }

        return true
    }

    nonisolated func runKlondike(gameID: Int, drawAdvance: Int, config: SimConfig) -> SimResult {
        var deck: [Card] = []
        let suits: [Suit] = [.club, .diamond, .heart, .spade]
        for suit in suits {
            for v in 1...13 {
                deck.append(Card(suit: suit, value: UInt8(v)))
            }
        }
        deck.shuffle()

        let engine = KlondikeEngine(drawAdvance: Int32(drawAdvance))

        var startBoard: KlondikeBoard?
        do {
            startBoard = try engine.deal(deck: deck)
        } catch {}

        guard let board = startBoard else {
            return SimResult(
                id: gameID, moveCount: 0, undoCount: 0, checkpointCount: 0, won: false,
                stopReason: .stalled, duration: 0, score: 0, movesDetail: "deal_failed")
        }

        let start = Date()

        var finalBoard = board
        var score = 0
        var undoCount = 0
        var appliedMoves = 0

        let plannedMoves = engine.solve(
            board: finalBoard,
            allowPartial: true)
        let checkpoints = Int(engine.lastCheckpointsAdopted())

        if let plannedMoves = plannedMoves {
            for move in plannedMoves {
                let applied = applyKlondikeMove(
                    move,
                    engine: engine,
                    board: &finalBoard,
                    score: &score,
                    undoCount: &undoCount,
                    appliedMoves: &appliedMoves)
                if !applied {
                    break
                }
            }
        }

        let duration = Date().timeIntervalSince(start)
        let won = engine.isWin(board: finalBoard)
        let timedOut = duration >= 100.0

        let hasUsefulProgress =
            engine.progressToken(board: finalBoard) != engine.progressToken(board: board)
        let stopReason: SimStopReason =
            won ? .win : ((timedOut && hasUsefulProgress) ? .timeout : .stalled)

        return SimResult(
            id: gameID, moveCount: appliedMoves, undoCount: undoCount, checkpointCount: checkpoints,
            won: won, stopReason: stopReason, duration: duration, score: score, movesDetail: "")
    }
}
