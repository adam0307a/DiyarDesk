// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "DiyarDeskMac",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .library(name: "DiyarDeskCore", targets: ["DiyarDeskCore"]),
        .executable(name: "DiyarDeskMac", targets: ["DiyarDeskMac"])
    ],
    targets: [
        .target(
            name: "DiyarDeskCore",
            path: "Sources/DiyarDeskCore"
        ),
        .executableTarget(
            name: "DiyarDeskMac",
            dependencies: ["DiyarDeskCore"],
            path: "Sources/DiyarDeskMac",
            linkerSettings: [
                .linkedFramework("AVFoundation"),
                .linkedFramework("OpenGL"),
                .linkedFramework("QuartzCore")
            ]
        ),
        .testTarget(
            name: "DiyarDeskMacTests",
            dependencies: ["DiyarDeskCore"],
            path: "Tests/DiyarDeskMacTests"
        )
    ]
)
