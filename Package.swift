// swift-tools-version: 6.1
import PackageDescription

let package = Package(
    name: "SolverLab",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(name: "SolverLabApp", targets: ["SolverLabApp"])
    ],
    targets: [
        .binaryTarget(
            name: "SolverCoreRS",
            path: "SolverCoreRS.xcframework"
        ),
        .target(
            name: "SolverCoreBridge",
            dependencies: ["SolverCoreRS"],
            path: "Sources/SolverCoreRS"
        ),
        .executableTarget(
            name: "SolverLabApp",
            dependencies: ["SolverCoreBridge"],
            path: "Sources",
            exclude: ["SolverCoreRS"],
            sources: ["SolverLabApp"]
        )
    ]
)
