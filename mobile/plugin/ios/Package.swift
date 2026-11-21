// swift-tools-version:5.5

import PackageDescription

let package = Package(
  name: "tauri-plugin-rupert-phone",
  platforms: [
    .iOS(.v15)
  ],
  products: [
    .library(
      name: "tauri-plugin-rupert-phone",
      type: .static,
      targets: ["tauri-plugin-rupert-phone"])
  ],
  dependencies: [
    .package(name: "Tauri", path: "../.tauri/tauri-api")
  ],
  targets: [
    .target(
      name: "tauri-plugin-rupert-phone",
      dependencies: [
        .byName(name: "Tauri")
      ],
      path: "Sources")
  ]
)
