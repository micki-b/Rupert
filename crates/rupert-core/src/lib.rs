//! Rupert's library engine.
//!
//! Everything that is not UI lives here so it can be tested without Tauri and
//! reused by other front ends (mobile viewer, CLI). See `docs/DESIGN.md`.

pub mod error;
pub mod file_kind;
pub mod listing;
pub mod meta;
pub mod pdf_meta;
pub mod sidecar;
pub mod source;
pub mod watch;
pub mod xmp;

pub use error::{Error, Result};
pub use file_kind::{FileKind, is_ignored_name};
pub use listing::{CloudState, Entry, cloud_state, list_dir, resolve};
pub use meta::{DocMeta, MetaRecord, Storage, read_meta, write_meta};
pub use source::{Provider, Source, SourceStore};
pub use watch::{DirChange, LibraryWatcher};

/// Version of the library engine.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
