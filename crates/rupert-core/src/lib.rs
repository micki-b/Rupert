//! Rupert's library engine.
//!
//! Everything that is not UI lives here so it can be tested without Tauri and
//! reused by other front ends (mobile viewer, CLI). See `docs/DESIGN.md`.

pub mod error;
pub mod file_kind;
pub mod listing;
pub mod source;
pub mod watch;

pub use error::{Error, Result};
pub use file_kind::{FileKind, is_ignored_name};
pub use listing::{CloudState, Entry, list_dir, resolve};
pub use source::{Provider, Source, SourceStore};
pub use watch::{DirChange, LibraryWatcher};

/// Version of the library engine.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
