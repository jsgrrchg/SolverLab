import SwiftUI

@main
struct SolverLabApp: App {
    var body: some Scene {
        WindowGroup("Solver Lab") {
            ContentView()
                .frame(minWidth: 1200, minHeight: 760)
        }
        .windowResizability(.contentSize)
    }
}
