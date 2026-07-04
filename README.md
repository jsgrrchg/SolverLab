# SolverLab

SolverLab is a Rust solitaire solver with deterministic solver logic and a SwiftUI dashboard for prototyping integration with native Swift games. The Rust core is exposed through UniFFI and is designed to balance speed with CPU and memory usage so it can run on modern processors, including phones, while streaming useful progress back to a graphical host app through replayable moves, partial results, progress tokens, and adopted checkpoints.

## Supported Games

- Klondike, draw 1 and draw 3
- FreeCell
- Pyramid
- TriPeaks
- Spider, 1-suit, 2-suit, and 4-suit

## Features

- Batch simulations with configurable parallelism
- Per-game timeout, search depth, and undo limits
- Deterministic solver logic: the same board and settings produce the same solver path
- Resource-aware search designed to balance speed, CPU usage, and memory pressure
- Checkpoint-oriented progress reporting for graphical game integrations
- Solver metrics, stop reasons, scores, checkpoint counts, and CSV export
- Rust search core with Swift bindings packaged as `SolverCoreRS.xcframework`

## Integration Model

The solvers are built for native UI integration. Each engine exposes a Swift-friendly API to solve with `allow_partial`, replay returned moves with `apply_move`, compare progress with `progress_token`, and inspect how many checkpoints were adopted with `last_checkpoints_adopted`.

This lets a graphical game use the Rust solver as a deterministic planning engine while keeping animation, state display, and user interaction in a graphical game.

Note: the solver core does not depend on SwiftUI. The dashboard is only a prototyping host, and the Rust library can be packaged for other native apps.

## Win Rate

- Klondike draw 1: approximately 80% in current internal runs, with solve times typically between 2 and 5 seconds per game
- Klondike draw 3: approximately 65% in current internal runs, with solve times typically between 2 and 5 seconds per game
- FreeCell: approximately 95% in current internal runs, with solve times typically under 1 second per game
- Pyramid: approximately 75% in current internal runs, with solve times typically under 1 second per game
- TriPeaks: approximately 87% in current internal runs, with solve times typically under 1 second per game
- Spider 1-suit: approximately 95% in current internal runs, with solve times typically between 1 and 5 seconds per game
- Spider 2-suit: approximately 60% in current internal runs, with solve times typically between 2 and 60 seconds per game; there is still room for optimization
- Spider 4-suit: currently under study, with poor win rates and solve times; it is not usable yet and is technically the hardest variant to solve deterministically
- More game variants and benchmark numbers will be added in the future.

Win rates vary by game variant, timeout, search limits, and resource budget.

## Requirements

- macOS 13 or newer
- Xcode with Swift 6.1 toolchain support
- Rust stable toolchain with Cargo
- Apple Silicon or Intel Mac for local development

## Quick Start

```bash
swift run -c release SolverLabApp
```

To rebuild the Rust core and regenerate the Swift bindings:

```bash
make xcframework
```

## Tests

Run the default fast local test suite:

```bash
cd solver-core-rs && cargo test
swift test
```

Run the slower ignored Rust smoke tests when validating full solver behavior:

```bash
cd solver-core-rs && cargo test --test integration_tests -- --ignored
```

## Project Layout

```text
Sources/                  SwiftUI app and Swift bridge code
solver-core-rs/           Rust solver core and UniFFI definitions
SolverCoreRS.xcframework  Packaged native solver library
Tests/                    Swift test target
scripts/                  Local helper scripts
```

## Sponsorship

If you use SolverLab in a consumer game, please consider sponsoring the project to support continued development, higher win rates, and better CPU and memory efficiency.

## License

Apache-2.0
