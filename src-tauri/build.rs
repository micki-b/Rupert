fn main() {
    // Apple Intelligence (rupert-core's Swift bridge): the Swift runtime is
    // part of macOS, at /usr/lib/swift, and the bridge refers to it there
    // through the run-path.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo::rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }
    tauri_build::build()
}
