import Darwin
import Foundation

extension SimulationViewModel {
    nonisolated func runSimulation(config: SimConfig) async {
        // Elevar QoS del hilo que ejecuta runSimulation al máximo
        // para evitar que macOS throttlee cuando la app está en segundo plano.
        _ = pthread_set_qos_class_self_np(QOS_CLASS_USER_INITIATED, 0)
        let cpuCap = max(1, ProcessInfo.processInfo.activeProcessorCount)
        var workers =
            config.autoParallel
            ? autoParallelInitial(totalGames: config.simulations)
            : max(1, min(config.parallelGames, config.simulations, cpuCap))
        let coordinator = SimCoordinator(total: config.simulations)
        let initialWorkers = workers
        await MainActor.run { self.currentParallelUsed = initialWorkers }

        while !Task.isCancelled {
            var batchResults: [SimResult] = []
            batchResults.reserveCapacity(workers)
            let workersForBatch = workers

            await withTaskGroup(of: SimResult?.self) { group in
                for _ in 0..<workersForBatch {
                    group.addTask(priority: .userInitiated) { [weak self] in
                        guard let self else { return nil }
                        guard let gameID = await coordinator.takeNextGameID() else { return nil }
                        await self.activeGamesClock.begin(gameID: gameID)
                        let result = await self.runSingleGameOffMain(gameID: gameID, config: config)
                        await self.activeGamesClock.end(gameID: gameID)
                        return result
                    }
                }

                for await maybeResult in group {
                    if let result = maybeResult {
                        batchResults.append(result)
                        let totalGames = config.simulations
                        await MainActor.run {
                            guard !Task.isCancelled else { return }
                            self.record(result: result, total: totalGames)
                        }
                    }
                }
            }

            if batchResults.isEmpty { break }

            // Re-verificar que la actividad de sistema siga activa entre batches.
            // Si macOS la anuló silenciosamente (ej. en background prolongado),
            // la re-registramos para mantener la protección contra App Nap.
            await MainActor.run {
                guard !Task.isCancelled else { return }
                self.ensureRunActivity()
            }

            var batchStats = SimBatchStats()
            batchStats.completed = batchResults.count
            batchStats.wins = batchResults.reduce(0) { $0 + ($1.won ? 1 : 0) }
            batchStats.timeouts = batchResults.reduce(0) {
                $0 + ($1.stopReason == .timeout ? 1 : 0)
            }

            if config.autoParallel {
                let completedNow = await MainActor.run { self.completedGames }
                workers = autoParallelNext(
                    current: workers,
                    completed: completedNow,
                    totalGames: config.simulations,
                    batchStats: batchStats)
                let updatedWorkers = workers
                await MainActor.run { self.currentParallelUsed = updatedWorkers }
            }
        }

        await activeGamesClock.clear()
        await MainActor.run {
            self.countdownTask?.cancel()
            self.countdownTask = nil
            self.endRunActivity()
            self.runStartedAt = nil
            self.countdownRemaining = 0
            self.estimatedRemainingTotal = 0
            self.currentParallelUsed = 0
            if !Task.isCancelled {
                self.isRunning = false
                self.statusText = "Completed"
            }
        }
    }

    nonisolated func runSingleGame(gameID: Int, config: SimConfig) -> SimResult {
        switch config.gameType {
        case .klondikeDraw1:
            return runKlondike(gameID: gameID, drawAdvance: 1, config: config)
        case .klondikeDraw3:
            return runKlondike(gameID: gameID, drawAdvance: 3, config: config)
        case .freeCell:
            return runFreeCell(gameID: gameID, config: config)
        case .pyramid:
            return runPyramid(gameID: gameID, config: config)
        case .triPeaks:
            return runTriPeaks(gameID: gameID, config: config)
        case .spider1Suit:
            return runSpider(gameID: gameID, suitCount: 1, config: config)
        case .spider2Suit:
            return runSpider(gameID: gameID, suitCount: 2, config: config)
        case .spider4Suit:
            return runSpider(gameID: gameID, suitCount: 4, config: config)
        }
    }

    nonisolated func runSingleGameOffMain(gameID: Int, config: SimConfig) async -> SimResult {
        _ = pthread_set_qos_class_self_np(QOS_CLASS_USER_INITIATED, 0)
        return runSingleGame(gameID: gameID, config: config)
    }

    private func record(result: SimResult, total: Int) {
        allResults.append(result)
        completedGames += 1
        if result.won { wins += 1 }
        totalMoves += result.moveCount
        totalUndos += result.undoCount
        totalDuration += result.duration

        switch result.stopReason {
        case .win: winCount += 1
        case .stalled: stalledCount += 1
        case .timeout: timeoutCount += 1
        }

        results.append(result)
        if results.count > 1500 {
            results.removeFirst(results.count - 1500)
        }

        statusText = "Running \(completedGames)/\(total)"
    }

    private func recordBatch(results batch: [SimResult], total: Int) {
        guard !batch.isEmpty else { return }

        allResults.append(contentsOf: batch)
        completedGames += batch.count
        wins += batch.reduce(0) { $0 + ($1.won ? 1 : 0) }
        totalMoves += batch.reduce(0) { $0 + $1.moveCount }
        totalUndos += batch.reduce(0) { $0 + $1.undoCount }
        totalDuration += batch.reduce(0) { $0 + $1.duration }

        winCount += batch.reduce(0) { $0 + ($1.stopReason == .win ? 1 : 0) }
        stalledCount += batch.reduce(0) { $0 + ($1.stopReason == .stalled ? 1 : 0) }
        timeoutCount += batch.reduce(0) { $0 + ($1.stopReason == .timeout ? 1 : 0) }

        results.append(contentsOf: batch)
        if results.count > 1500 {
            results.removeFirst(results.count - 1500)
        }

        statusText = "Running \(completedGames)/\(total)"
    }
}
