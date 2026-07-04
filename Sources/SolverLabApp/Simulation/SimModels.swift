import Foundation
import SwiftUI

enum SimGameType: String, CaseIterable, Identifiable, Codable {
    case klondikeDraw1 = "Klondike Draw 1"
    case klondikeDraw3 = "Klondike Draw 3"
    case freeCell = "FreeCell"
    case pyramid = "Pyramid"
    case triPeaks = "TriPeaks"
    case spider1Suit = "Spider 1 Suit"
    case spider2Suit = "Spider 2 Suits"
    case spider4Suit = "Spider 4 Suits"

    var id: String { rawValue }

    var uiLabel: String {
        switch self {
        case .klondikeDraw1:
            return "Klondike Draw 1"
        case .klondikeDraw3:
            return "Klondike Draw 3"
        case .freeCell:
            return "FreeCell"
        case .pyramid:
            return "Pyramid"
        case .triPeaks:
            return "TriPeaks"
        case .spider1Suit:
            return "Spider 1 Suit"
        case .spider2Suit:
            return "Spider 2 Suits"
        case .spider4Suit:
            return "Spider 4 Suits"
        }
    }

    var strategyLabel: String {
        switch self {
        case .klondikeDraw1, .klondikeDraw3:
            return "IDA*"
        case .freeCell:
            return "A*"
        case .pyramid:
            return "DFS"
        case .triPeaks:
            return "A*"
        case .spider1Suit, .spider2Suit, .spider4Suit:
            return "A*"
        }
    }
}

enum SimStopReason: String, CaseIterable {
    case win
    case stalled
    case timeout
}

struct SimResult: Identifiable {
    let id: Int
    let moveCount: Int
    let undoCount: Int
    let checkpointCount: Int?
    let won: Bool
    let stopReason: SimStopReason
    let duration: TimeInterval
    let score: Int
    let movesDetail: String

    var resultLabel: String { won ? "won" : "lost" }
    var stopReasonLabel: String {
        switch stopReason {
        case .win:
            return "won"
        case .stalled:
            return "stalled"
        case .timeout:
            return "timeout"
        }
    }
    var checkpointCountLabel: String { checkpointCount.map(String.init) ?? "-" }
    var checkpointCountSortValue: Int { checkpointCount ?? -1 }

    var stopReasonIcon: String {
        switch stopReason {
        case .win: return "checkmark.circle.fill"
        case .stalled: return "pause.circle.fill"
        case .timeout: return "clock.badge.exclamationmark.fill"
        }
    }
    var stopReasonColor: Color {
        switch stopReason {
        case .win: return .green
        case .stalled: return .orange
        case .timeout: return .red
        }
    }
}

struct SimConfig: Codable {
    var gameType: SimGameType = .klondikeDraw1
    var simulations: Int = 0
    var parallelGames: Int = 0
    var autoParallel: Bool = false
    var maxUndos: Int = -1
    var timeoutSeconds: Double = 0
    var maxDepth: Int = 150
}

actor SimCoordinator {
    private var nextGameID: Int = 1
    private let total: Int

    init(total: Int) {
        self.total = total
    }

    func takeNextGameID() -> Int? {
        guard nextGameID <= total else { return nil }
        let id = nextGameID
        nextGameID += 1
        return id
    }
}

struct SimBatchStats {
    var completed = 0
    var wins = 0
    var timeouts = 0
}

actor SimActiveGamesClock {
    private var startedAtByGameID: [Int: Date] = [:]

    func begin(gameID: Int) {
        startedAtByGameID[gameID] = Date()
    }

    func end(gameID: Int) {
        startedAtByGameID[gameID] = nil
    }

    func clear() {
        startedAtByGameID.removeAll()
    }

    func oldestStart() -> Date? {
        startedAtByGameID.values.min()
    }
}
