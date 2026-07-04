import AppKit
import Darwin
import Dispatch
import Foundation
import SwiftUI

#if canImport(SolverCore)
    import SolverCore
#endif

@MainActor
final class SimulationViewModel: ObservableObject {
    @Published var config = SimConfig() {
        didSet {
            persistConfig()
        }
    }
    @Published var isRunning = false
    @Published var completedGames = 0
    @Published var wins = 0
    @Published var totalMoves = 0
    @Published var totalUndos = 0
    @Published var totalDuration: TimeInterval = 0
    @Published var timeoutCount = 0
    @Published var stalledCount = 0
    @Published var winCount = 0
    @Published var results: [SimResult] = []
    @Published var statusText = "Ready"
    @Published var countdownRemaining: TimeInterval = 0
    @Published var estimatedRemainingTotal: TimeInterval = 0
    @Published var currentParallelUsed: Int = 0

    private static let configDefaultsKey = "solverlab.simconfig.v1"
    private var runTask: Task<Void, Never>?
    var countdownTask: Task<Void, Never>?
    var allResults: [SimResult] = []
    private var lastRunConfig: SimConfig?
    private var runActivity: NSObjectProtocol?
    let activeGamesClock = SimActiveGamesClock()
    var runStartedAt: Date?

    init() {
        loadPersistedConfig()
    }

    var winRate: Double {
        guard completedGames > 0 else { return 0 }
        return Double(wins) / Double(completedGames)
    }

    var averageMoves: Double {
        guard completedGames > 0 else { return 0 }
        return Double(totalMoves) / Double(completedGames)
    }

    var averageDuration: Double {
        guard completedGames > 0 else { return 0 }
        return totalDuration / Double(completedGames)
    }

    var averageUndos: Double {
        guard completedGames > 0 else { return 0 }
        return Double(totalUndos) / Double(completedGames)
    }

    var progressFraction: Double {
        let total = max(1, config.simulations)
        return min(1.0, Double(completedGames) / Double(total))
    }

    var countdownDisplay: String {
        formatDuration(countdownRemaining)
    }

    var estimatedRemainingDisplay: String {
        formatDuration(estimatedRemainingTotal)
    }

    private func formatDuration(_ value: TimeInterval) -> String {
        let totalSeconds = max(0, Int(value.rounded(.up)))
        let hours = totalSeconds / 3600
        let minutes = totalSeconds / 60
        let seconds = totalSeconds % 60
        if hours > 0 {
            return String(format: "%02d:%02d:%02d", hours, minutes % 60, seconds)
        }
        return String(format: "%02d:%02d", minutes, seconds)
    }

    func exportCSV() {
        guard !allResults.isEmpty else {
            statusText = "No results to export"
            return
        }

        let panel = NSSavePanel()
        panel.canCreateDirectories = true
        let csvGameType = lastRunConfig?.gameType ?? config.gameType
        panel.nameFieldStringValue = defaultCSVFileName(for: csvGameType)
        panel.allowedContentTypes = [.commaSeparatedText]
        panel.title = "Export CSV Results"

        guard panel.runModal() == .OK, let url = panel.url else { return }

        do {
            try makeCSV(
                from: allResults,
                timeoutActiveDisplay: countdownDisplay,
                runConfig: lastRunConfig
            ).write(to: url, atomically: true, encoding: .utf8)
            statusText = "CSV exported: \(url.lastPathComponent)"
        } catch {
            statusText = "Error exporting CSV"
        }
    }

    func start() {
        guard !isRunning else { return }
        resetMetrics()
        isRunning = true
        statusText = "Running..."
        beginRunActivity()

        let currentConfig = config
        lastRunConfig = currentConfig
        runStartedAt = Date()
        countdownRemaining = currentConfig.timeoutSeconds > 0 ? currentConfig.timeoutSeconds : 15.0
        estimatedRemainingTotal = 0
        countdownTask = Task.detached(priority: .utility) { [weak self] in
            guard let self else { return }
            await self.runCountdown(config: currentConfig)
        }
        let runPriority: TaskPriority = .userInitiated
        runTask = Task.detached(priority: runPriority) { [weak self] in
            guard let self else { return }
            await self.runSimulation(config: currentConfig)
        }
    }

    func stop() {
        runTask?.cancel()
        countdownTask?.cancel()
        runTask = nil
        countdownTask = nil
        endRunActivity()
        isRunning = false
        runStartedAt = nil
        countdownRemaining = 0
        estimatedRemainingTotal = 0
        statusText = "Stopped"
    }

    func clearData() {
        runTask?.cancel()
        countdownTask?.cancel()
        runTask = nil
        countdownTask = nil
        endRunActivity()
        isRunning = false
        runStartedAt = nil
        countdownRemaining = 0
        estimatedRemainingTotal = 0
        resetMetrics()
        statusText = "Data cleared"
    }

    private func resetMetrics() {
        completedGames = 0
        wins = 0
        totalMoves = 0
        totalUndos = 0
        totalDuration = 0
        timeoutCount = 0
        stalledCount = 0
        winCount = 0
        results = []
        allResults = []
        lastRunConfig = nil
        runStartedAt = nil
        countdownRemaining = 0
        estimatedRemainingTotal = 0
        currentParallelUsed = 0
        statusText = "Ready"
    }

    private func loadPersistedConfig() {
        guard let data = UserDefaults.standard.data(forKey: Self.configDefaultsKey) else { return }
        if let saved = try? JSONDecoder().decode(SimConfig.self, from: data) {
            config = saved
            return
        }

        if let legacy = try? JSONDecoder().decode(LegacySimConfig.self, from: data) {
            config = SimConfig(
                gameType: legacy.gameType,
                simulations: legacy.simulations,
                parallelGames: legacy.parallelGames,
                autoParallel: legacy.autoParallel,
                maxUndos: legacy.maxUndos,
                timeoutSeconds: legacy.timeoutSeconds,
                maxDepth: legacy.maxDepth)
        }
    }

    private func persistConfig() {
        guard let data = try? JSONEncoder().encode(config) else { return }
        UserDefaults.standard.set(data, forKey: Self.configDefaultsKey)
    }

    private struct LegacySimConfig: Codable {
        var gameType: SimGameType
        var simulations: Int
        var parallelGames: Int
        var autoParallel: Bool
        var maxUndos: Int
        var timeoutSeconds: Double
        var maxDepth: Int
    }

    func beginRunActivity() {
        guard runActivity == nil else { return }
        runActivity = ProcessInfo.processInfo.beginActivity(
            options: [
                .userInitiated, .latencyCritical, .suddenTerminationDisabled,
                .automaticTerminationDisabled,
            ],
            reason: "Solver simulations running"
        )
    }

    func endRunActivity() {
        guard let runActivity else { return }
        ProcessInfo.processInfo.endActivity(runActivity)
        self.runActivity = nil
    }

    /// Re-registers system activity if macOS silently cancelled it.
    /// Call between batches to ensure continuous App Nap protection.
    func ensureRunActivity() {
        guard isRunning else { return }
        if runActivity == nil {
            beginRunActivity()
        }
    }

    nonisolated func autoParallelInitial(totalGames: Int) -> Int {
        let cpu = ProcessInfo.processInfo.activeProcessorCount
        let recommended = max(1, cpu / 2)
        return min(totalGames, recommended)
    }

    nonisolated func autoParallelNext(
        current: Int,
        completed: Int,
        totalGames: Int,
        batchStats: SimBatchStats
    ) -> Int {
        let cpuCap = max(1, ProcessInfo.processInfo.activeProcessorCount)
        let hardCap = min(totalGames, cpuCap)
        guard batchStats.completed > 0 else { return max(1, min(current, hardCap)) }

        let timeoutRate = Double(batchStats.timeouts) / Double(batchStats.completed)
        let winRate = Double(batchStats.wins) / Double(batchStats.completed)

        var next = current
        if timeoutRate > 0.55 {
            next = max(1, current - 1)
        } else if timeoutRate < 0.25, winRate >= 0.25 {
            next = min(hardCap, current + 1)
        }

        let remaining = max(1, totalGames - completed)
        return max(1, min(next, remaining, hardCap))
    }

    nonisolated private func runCountdown(config: SimConfig) async {
        let timeoutSeconds = config.timeoutSeconds > 0 ? config.timeoutSeconds : 15.0
        let totalGames = max(1, config.simulations)
        let configuredWorkers = max(1, min(config.parallelGames, totalGames))

        while !Task.isCancelled {
            let now = Date()
            let oldest = await activeGamesClock.oldestStart()
            let remaining =
                oldest.map { max(0, timeoutSeconds - now.timeIntervalSince($0)) } ?? timeoutSeconds

            await MainActor.run {
                guard !Task.isCancelled else { return }
                self.countdownRemaining = remaining

                let remainingGames = max(0, totalGames - self.completedGames)
                guard remainingGames > 0 else {
                    self.estimatedRemainingTotal = 0
                    return
                }

                if let startedAt = self.runStartedAt, self.completedGames > 0 {
                    let elapsed = max(0, now.timeIntervalSince(startedAt))
                    let averageWallTimePerGame = elapsed / Double(self.completedGames)
                    self.estimatedRemainingTotal = max(
                        0, Double(remainingGames) * averageWallTimePerGame)
                } else {
                    let initialAutoWorkers = self.autoParallelInitial(totalGames: totalGames)
                    let fallbackWorkers =
                        config.autoParallel ? initialAutoWorkers : configuredWorkers
                    let dynamicWorkers =
                        self.currentParallelUsed > 0 ? self.currentParallelUsed : fallbackWorkers
                    let workersForEstimate = max(1, min(dynamicWorkers, remainingGames))
                    let conservative =
                        (Double(remainingGames) * timeoutSeconds) / Double(workersForEstimate)
                    self.estimatedRemainingTotal = max(0, conservative)
                }
            }

            try? await Task.sleep(nanoseconds: 200_000_000)
        }
    }

    private func defaultCSVFileName(for gameType: SimGameType) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyyMMdd_HHmmss"
        let tokens = gameType.rawValue.unicodeScalars.split { scalar in
            !CharacterSet.alphanumerics.contains(scalar)
        }.map(String.init)
        let gameName = tokens.isEmpty ? "Game" : tokens.joined()
        return "\(gameName)_results_\(formatter.string(from: Date())).csv"
    }

    private func csvEscape(_ raw: String) -> String {
        let escaped = raw.replacingOccurrences(of: "\"", with: "\"\"")
        return "\"\(escaped)\""
    }

    private func makeCSV(
        from rows: [SimResult],
        timeoutActiveDisplay: String,
        runConfig: SimConfig?
    ) -> String {
        var lines: [String] = []
        lines.reserveCapacity(rows.count + 12)

        lines.append("configuration")
        lines.append(
            "game,search,simulations,parallel_games,auto_parallel,undos,timeout_s,max_depth")
        if let runConfig {
            let game = csvEscape(runConfig.gameType.rawValue)
            let strategyLabel = runConfig.gameType.strategyLabel
            let strategy = csvEscape(strategyLabel)
            let autoValue = runConfig.autoParallel ? "true" : "false"
            let timeoutValue = String(format: "%.2f", runConfig.timeoutSeconds)
            lines.append(
                "\(game),\(strategy),\(runConfig.simulations),\(runConfig.parallelGames),\(autoValue),\(runConfig.maxUndos),\(timeoutValue),\(runConfig.maxDepth)"
            )
        } else {
            lines.append("\"(no_config)\",\"(no_config)\",0,0,false,0,0.00,0")
        }
        lines.append("")

        let sortedRows = rows.sorted(by: { $0.id < $1.id })
        let totalGames = sortedRows.count
        let winCount = sortedRows.filter { $0.stopReason == .win }.count
        let stalledCount = sortedRows.filter { $0.stopReason == .stalled }.count
        let timeoutCount = sortedRows.filter { $0.stopReason == .timeout }.count
        let totalMoves = sortedRows.reduce(0) { $0 + $1.moveCount }
        let totalDuration = sortedRows.reduce(0.0) { $0 + $1.duration }
        let averageMoves = totalGames > 0 ? Double(totalMoves) / Double(totalGames) : 0
        let averageDuration = totalGames > 0 ? totalDuration / Double(totalGames) : 0
        let winRate = totalGames > 0 ? (Double(winCount) / Double(totalGames)) * 100 : 0
        let winRateValue = String(format: "%.2f%%", winRate)
        let averageMovesValue = String(format: "%.2f", averageMoves)
        let averageDurationValue = String(format: "%.2fs", averageDuration)

        lines.append("summary")
        let totalUndos = sortedRows.reduce(0) { $0 + $1.undoCount }
        let averageUndos = totalGames > 0 ? Double(totalUndos) / Double(totalGames) : 0
        let averageUndosValue = String(format: "%.2f", averageUndos)
        let totalCheckpoints = sortedRows.reduce(0) { $0 + ($1.checkpointCount ?? 0) }
        let averageCheckpoints = totalGames > 0 ? Double(totalCheckpoints) / Double(totalGames) : 0
        let averageCheckpointsValue = String(format: "%.2f", averageCheckpoints)

        lines.append(
            "games,active_timeout,win_rate,avg_moves,avg_undos,avg_checkpoints,avg_duration,wins,stalled,timeouts"
        )
        lines.append(
            "\(totalGames),\(timeoutActiveDisplay),\(winRateValue),\(averageMovesValue),\(averageUndosValue),\(averageCheckpointsValue),\(averageDurationValue),\(winCount),\(stalledCount),\(timeoutCount)"
        )
        lines.append("")

        lines.append("details")
        lines.append("game_id,moves,undos,checkpoints,result,stop_reason,duration_sec,score")

        for row in sortedRows {
            let resultValue = row.won ? "won" : "lost"
            let durationValue = String(format: "%.3f", row.duration)
            let checkpointValue = row.checkpointCount.map(String.init) ?? ""
            lines.append(
                "\(row.id),\(row.moveCount),\(row.undoCount),\(checkpointValue),\(resultValue),\(row.stopReason.rawValue),\(durationValue),\(row.score)"
            )
        }
        return lines.joined(separator: "\n") + "\n"
    }

}
