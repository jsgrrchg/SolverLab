import SwiftUI

struct ContentView: View {
    @StateObject private var vm = SimulationViewModel()
    @State private var sortOrder: [KeyPathComparator<SimResult>] = [
        KeyPathComparator(\SimResult.id, order: .reverse)
    ]

    private var descendingSortOrderBinding: Binding<[KeyPathComparator<SimResult>]> {
        Binding(
            get: { sortOrder },
            set: { newValue in
                guard !newValue.isEmpty else {
                    sortOrder = [KeyPathComparator(\SimResult.id, order: .reverse)]
                    return
                }
                sortOrder = newValue.map { comparator in
                    var forced = comparator
                    forced.order = .reverse
                    return forced
                }
            }
        )
    }

    var body: some View {
        VStack(spacing: 14) {
            controls
            progressBar
            metrics
            resultsTable
        }
        .padding(16)
    }

    private var controls: some View {
        HStack(alignment: .center, spacing: 10) {
            Picker("", selection: $vm.config.gameType) {
                ForEach(SimGameType.allCases) { type in
                    Text(type.uiLabel).tag(type)
                }
            }
            .labelsHidden()
            .frame(width: 180)

            TextField("Sims", value: $vm.config.simulations, format: .number)
                .textFieldStyle(.roundedBorder)
                .frame(width: 70)

            TextField("Parallel", value: $vm.config.parallelGames, format: .number)
                .textFieldStyle(.roundedBorder)
                .frame(width: 60)
                .disabled(vm.config.autoParallel)

            Toggle("Auto", isOn: $vm.config.autoParallel)
                .toggleStyle(.switch)
                .fixedSize()

            Divider().frame(height: 22)

            Button {
                vm.isRunning ? vm.stop() : vm.start()
            } label: {
                Label(
                    vm.isRunning ? "Stop" : "Start",
                    systemImage: vm.isRunning ? "stop.fill" : "play.fill")
            }
            .buttonStyle(.borderedProminent)
            .tint(vm.isRunning ? .red : .accentColor)

            Button("Export CSV") { vm.exportCSV() }
                .disabled(vm.results.isEmpty)

            Button("Clear Data") { vm.clearData() }
                .disabled(vm.results.isEmpty && vm.completedGames == 0)

            Spacer()

            Label(vm.statusText, systemImage: "circle.fill")
                .font(.subheadline)
                .foregroundStyle(vm.isRunning ? .green : .secondary)
                .imageScale(.small)
                .lineLimit(1)
        }
    }

    private var metrics: some View {
        HStack(spacing: 12) {
            // Estado
            HStack(spacing: 8) {
                MetricBox(title: "Games", value: "\(vm.completedGames)", tint: .blue)
                MetricBox(title: "Parallel used", value: "\(vm.currentParallelUsed)", tint: .blue)
                MetricBox(title: "ETA", value: vm.estimatedRemainingDisplay, tint: .blue)
                MetricBox(title: "Active timeout", value: vm.countdownDisplay, tint: .blue)
            }

            Divider().frame(height: 40)

            // Resultados
            HStack(spacing: 8) {
                MetricBox(
                    title: "Win rate", value: String(format: "%.2f%%", vm.winRate * 100),
                    tint: .green)
                MetricBox(title: "Wins", value: "\(vm.winCount)", tint: .green)
                MetricBox(title: "Stalled", value: "\(vm.stalledCount)", tint: .orange)
                MetricBox(title: "Timeout", value: "\(vm.timeoutCount)", tint: .orange)
            }

            Divider().frame(height: 40)

            // Promedios
            HStack(spacing: 8) {
                MetricBox(
                    title: "Avg moves", value: String(format: "%.2f", vm.averageMoves),
                    tint: .purple)
                MetricBox(
                    title: "Avg undos", value: String(format: "%.2f", vm.averageUndos),
                    tint: .purple)
                MetricBox(
                    title: "Avg duration", value: String(format: "%.2fs", vm.averageDuration),
                    tint: .purple)
            }

            Spacer()
        }
    }

    private var progressBar: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text("Progress")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
                Text(
                    "\(vm.completedGames)/\(max(1, vm.config.simulations)) (\(Int((vm.progressFraction * 100).rounded()))%)"
                )
                .font(.caption.monospacedDigit())
                .foregroundStyle(.secondary)
            }
            ProgressView(value: vm.progressFraction, total: 1.0)
                .progressViewStyle(.linear)
        }
    }

    private var resultsTable: some View {
        let rows = vm.results.sorted(using: sortOrder)

        return Table(rows, sortOrder: descendingSortOrderBinding) {
            TableColumn("Game ID", value: \.id) { row in
                Text("\(row.id)")
            }
            .width(70)

            TableColumn("Moves", value: \.moveCount) { row in
                Text("\(row.moveCount)")
            }
            .width(70)

            TableColumn("Undos", value: \.undoCount) { row in
                Text("\(row.undoCount)")
            }
            .width(70)

            TableColumn("Checkpoints", value: \.checkpointCountSortValue) { row in
                Text(row.checkpointCountLabel)
            }
            .width(95)

            TableColumn("Result", value: \.resultLabel) { row in
                Label(
                    row.resultLabel,
                    systemImage: row.won ? "checkmark.circle.fill" : "xmark.circle.fill"
                )
                .foregroundStyle(row.won ? .green : .red)
            }
            .width(80)

            TableColumn("Stop Reason", value: \.stopReasonLabel) { row in
                Text(row.stopReasonLabel)
            }
            .width(110)

            TableColumn("Duration", value: \.duration) { row in
                Text(String(format: "%.2fs", row.duration))
            }
            .width(90)

            TableColumn("Score", value: \.score) { row in
                Text("\(row.score)")
            }
            .width(80)
        }
        .frame(minHeight: 350)
    }
}
