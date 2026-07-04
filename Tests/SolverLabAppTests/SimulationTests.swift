import XCTest
@testable import SolverLabApp

final class SimulationTests: XCTestCase {
    private func result(
        id: Int,
        moves: Int,
        undos: Int,
        checkpoints: Int? = nil,
        won: Bool,
        stopReason: SimStopReason,
        duration: TimeInterval,
        score: Int = 0,
        movesDetail: String = ""
    ) -> SimResult {
        SimResult(
            id: id,
            moveCount: moves,
            undoCount: undos,
            checkpointCount: checkpoints,
            won: won,
            stopReason: stopReason,
            duration: duration,
            score: score,
            movesDetail: movesDetail
        )
    }

    @MainActor
    func testAutoParallelInitial() {
        let viewModel = SimulationViewModel()
        let cpu = ProcessInfo.processInfo.activeProcessorCount
        let recommended = max(1, cpu / 2)

        XCTAssertEqual(viewModel.autoParallelInitial(totalGames: 0), 0)
        XCTAssertEqual(viewModel.autoParallelInitial(totalGames: 1), min(1, recommended))
        XCTAssertEqual(viewModel.autoParallelInitial(totalGames: max(1, recommended - 1)), max(1, min(recommended - 1, recommended)))
        XCTAssertEqual(viewModel.autoParallelInitial(totalGames: cpu * 4), recommended)
    }

    @MainActor
    func testAutoParallelNext() {
        let viewModel = SimulationViewModel()
        let cpu = max(1, ProcessInfo.processInfo.activeProcessorCount)

        XCTAssertEqual(
            viewModel.autoParallelNext(
                current: 4,
                completed: 10,
                totalGames: 100,
                batchStats: SimBatchStats(completed: 10, wins: 1, timeouts: 8)
            ),
            3
        )

        XCTAssertEqual(
            viewModel.autoParallelNext(
                current: 2,
                completed: 10,
                totalGames: 100,
                batchStats: SimBatchStats(completed: 10, wins: 5, timeouts: 1)
            ),
            min(3, cpu)
        )

        XCTAssertLessThanOrEqual(
            viewModel.autoParallelNext(
                current: cpu * 2,
                completed: 98,
                totalGames: 100,
                batchStats: SimBatchStats(completed: 10, wins: 9, timeouts: 0)
            ),
            2
        )
    }

    @MainActor
    func testMetrics() {
        let viewModel = SimulationViewModel()
        viewModel.config.simulations = 4
        viewModel.completedGames = 2
        viewModel.wins = 1
        viewModel.totalMoves = 30
        viewModel.totalDuration = 12
        viewModel.totalUndos = 4

        XCTAssertEqual(viewModel.winRate, 0.5)
        XCTAssertEqual(viewModel.averageMoves, 15)
        XCTAssertEqual(viewModel.averageDuration, 6)
        XCTAssertEqual(viewModel.averageUndos, 2)
        XCTAssertEqual(viewModel.progressFraction, 0.5)
    }

    @MainActor
    func testStopClearsRunningStateAndTimers() {
        let viewModel = SimulationViewModel()
        viewModel.isRunning = true
        viewModel.countdownRemaining = 10
        viewModel.estimatedRemainingTotal = 20
        viewModel.runStartedAt = Date()

        viewModel.stop()

        XCTAssertFalse(viewModel.isRunning)
        XCTAssertEqual(viewModel.countdownRemaining, 0)
        XCTAssertEqual(viewModel.estimatedRemainingTotal, 0)
        XCTAssertNil(viewModel.runStartedAt)
        XCTAssertEqual(viewModel.statusText, "Stopped")
    }

    @MainActor
    func testClearDataResetsCountersAndResults() {
        let viewModel = SimulationViewModel()
        viewModel.completedGames = 3
        viewModel.wins = 2
        viewModel.totalMoves = 50
        viewModel.totalUndos = 7
        viewModel.totalDuration = 9
        viewModel.timeoutCount = 1
        viewModel.stalledCount = 1
        viewModel.winCount = 1
        viewModel.results = [result(id: 1, moves: 10, undos: 0, won: true, stopReason: .win, duration: 1)]
        viewModel.allResults = viewModel.results

        viewModel.clearData()

        XCTAssertEqual(viewModel.completedGames, 0)
        XCTAssertEqual(viewModel.wins, 0)
        XCTAssertEqual(viewModel.totalMoves, 0)
        XCTAssertEqual(viewModel.totalUndos, 0)
        XCTAssertEqual(viewModel.totalDuration, 0)
        XCTAssertEqual(viewModel.timeoutCount, 0)
        XCTAssertEqual(viewModel.stalledCount, 0)
        XCTAssertEqual(viewModel.winCount, 0)
        XCTAssertTrue(viewModel.results.isEmpty)
        XCTAssertTrue(viewModel.allResults.isEmpty)
        XCTAssertEqual(viewModel.statusText, "Data cleared")
    }

    @MainActor
    func testFormatDuration() {
        let viewModel = SimulationViewModel()

        XCTAssertEqual(viewModel.formatDuration(-5), "00:00")
        XCTAssertEqual(viewModel.formatDuration(61.1), "01:02")
        XCTAssertEqual(viewModel.formatDuration(3661), "01:01:01")
    }

    @MainActor
    func testCSVSortsRowsSummarizesAndEscapesConfig() {
        let viewModel = SimulationViewModel()
        let rows = [
            result(id: 2, moves: 20, undos: 4, checkpoints: nil, won: false, stopReason: .timeout, duration: 3.5, score: 7),
            result(id: 1, moves: 10, undos: 2, checkpoints: 6, won: true, stopReason: .win, duration: 1.5, score: 9),
        ]
        let config = SimConfig(
            gameType: .spider2Suit,
            simulations: 2,
            parallelGames: 3,
            autoParallel: true,
            maxUndos: 5,
            timeoutSeconds: 12.345,
            maxDepth: 88
        )

        let csv = viewModel.makeCSV(from: rows, timeoutActiveDisplay: "00:12", runConfig: config)

        XCTAssertTrue(csv.contains("\"Spider 2 Suits\",\"A*\",2,3,true,5,12.35,88"))
        XCTAssertTrue(csv.contains("2,00:12,50.00%,15.00,3.00,3.00,2.50s,1,0,1"))
        XCTAssertLessThan(csv.range(of: "\n1,10,2,6,won,win,1.500,9")!.lowerBound, csv.range(of: "\n2,20,4,,lost,timeout,3.500,7")!.lowerBound)
        XCTAssertEqual(viewModel.csvEscape("a,\"b\""), "\"a,\"\"b\"\"\"")
    }

    func testSimCoordinatorReturnsSequentialIDsThenNil() async {
        let coordinator = SimCoordinator(total: 2)

        let first = await coordinator.takeNextGameID()
        let second = await coordinator.takeNextGameID()
        let exhausted = await coordinator.takeNextGameID()

        XCTAssertEqual(first, 1)
        XCTAssertEqual(second, 2)
        XCTAssertNil(exhausted)
    }

    func testSimActiveGamesClockTracksOldestStart() async throws {
        let clock = SimActiveGamesClock()

        let empty = await clock.oldestStart()
        XCTAssertNil(empty)

        await clock.begin(gameID: 1)
        let maybeFirst = await clock.oldestStart()
        let first = try XCTUnwrap(maybeFirst)
        try await Task.sleep(nanoseconds: 5_000_000)
        await clock.begin(gameID: 2)
        let oldestWithTwoGames = await clock.oldestStart()
        XCTAssertEqual(oldestWithTwoGames, first)

        await clock.end(gameID: 1)
        let maybeSecond = await clock.oldestStart()
        let second = try XCTUnwrap(maybeSecond)
        XCTAssertGreaterThan(second, first)

        await clock.clear()
        let cleared = await clock.oldestStart()
        XCTAssertNil(cleared)
    }
}
