//! Builds the Swift bridge to Apple Intelligence (`apple/RupertAI.swift`)
//! into a static library on macOS, and sets `cfg(apple_ai)` when it did.
//! Elsewhere, or with an SDK that lacks the Foundation Models framework, it
//! does nothing and `apple_ai.rs` reports Apple Intelligence as unavailable.

use std::path::{Path, PathBuf};
use std::process::Command;

// MARK: - Main

fn main() {
    println!("cargo::rustc-check-cfg=cfg(apple_ai)");
    println!("cargo::rerun-if-changed=apple/RupertAI.swift");
    println!("cargo::rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok("x86_64") => "x86_64",
        _ => return,
    };
    let Some(sdk) = xcrun(&["--sdk", "macosx", "--show-sdk-path"]) else {
        println!("cargo::warning=No macOS SDK found: building without Apple Intelligence.");
        return;
    };
    let sdk = PathBuf::from(sdk);
    if !sdk
        .join("System/Library/Frameworks/FoundationModels.framework")
        .exists()
    {
        println!(
            "cargo::warning=The macOS SDK has no Foundation Models (Xcode 26 or later needed): building without Apple Intelligence."
        );
        return;
    }
    // Rupert runs on macOS 13 (tauri.macos.conf.json); the Swift code checks
    // for macOS 26 at run time and weak-links the framework.
    let deployment =
        std::env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "13.0".to_string());
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let lib = out.join("librupert_ai.a");
    let status = Command::new("xcrun")
        .args([
            "swiftc",
            "-parse-as-library",
            "-static",
            "-emit-library",
            "-O",
        ])
        .args(["-swift-version", "5", "-module-name", "RupertAI"])
        .arg("-target")
        .arg(format!("{arch}-apple-macos{deployment}"))
        .arg("-sdk")
        .arg(&sdk)
        .arg("-o")
        .arg(&lib)
        .arg("apple/RupertAI.swift")
        .status()
        .expect("couldn't run swiftc");
    assert!(status.success(), "swiftc failed on apple/RupertAI.swift");

    println!("cargo::rustc-link-search=native={}", out.display());
    println!("cargo::rustc-link-lib=static=rupert_ai");
    // The Swift runtime is part of macOS; link against the SDK's stubs and
    // the toolchain's back-deployment libraries (for macOS 13–14).
    println!(
        "cargo::rustc-link-search=native={}",
        sdk.join("usr/lib/swift").display()
    );
    if let Some(toolchain) = swift_libs() {
        println!("cargo::rustc-link-search=native={}", toolchain.display());
    }
    // Where the Swift runtime is found at run time, for this crate's own
    // tests and examples (link arguments don't reach other crates' binaries:
    // src-tauri/build.rs adds the same for the app).
    println!("cargo::rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    println!("cargo::rustc-cfg=apple_ai");
}

// MARK: - Helpers

fn xcrun(args: &[&str]) -> Option<String> {
    let out = Command::new("xcrun").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The toolchain's `usr/lib/swift/macosx` (compatibility libraries).
fn swift_libs() -> Option<PathBuf> {
    let swiftc = xcrun(&["--find", "swiftc"])?;
    let dir = Path::new(&swiftc)
        .parent()?
        .parent()?
        .join("lib/swift/macosx");
    dir.exists().then_some(dir)
}
