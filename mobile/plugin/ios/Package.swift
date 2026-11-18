// swift-tools-version:5.5

import PackageDescription

let package = Package(
  name: "tauri-plugin-rupert-ios",
  platforms: [
    .iOS(.v15)
  ],
  products: [
    .library(
      name: "tauri-plugin-rupert-ios",
      type: .static,
      targets: ["tauri-plugin-rupert-ios"])
  ],
  dependencies: [
    .package(name: "Tauri", path: "../.tauri/tauri-api")
  ],
  targets: [
    .target(
      name: "tauri-plugin-rupert-ios",
      dependencies: [
        .byName(name: "Tauri")
      ],
      path: "Sources")
  ]
)
