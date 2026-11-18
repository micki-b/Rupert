//! Rupert's library engine.
//!
//! Everything that is not UI lives here so it can be tested without Tauri and
//! reused by other front ends (mobile viewer, CLI). See `docs/DESIGN.md`.

pub mod file_kind;

pub use file_kind::{FileKind, is_ignored_name};

/// Version of the library engine.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
