//! Rupert's library engine.
//!
//! Everything that is not UI lives here so it can be tested without Tauri and
//! reused by other front ends (mobile viewer, CLI). See `docs/DESIGN.md`.

pub mod catalog;
pub mod cloud;
pub mod error;
pub mod escl;
pub mod file_kind;
pub mod fileops;
pub mod index;
pub mod indexer;
pub mod intake;
pub mod listing;
pub mod llm;
pub mod meta;
pub mod ocr;
pub mod pdf_meta;
pub mod scansnap;
pub mod settings;
pub mod sidecar;
pub mod source;
pub mod text;
pub mod text_layer;
pub mod watch;
pub mod xmp;

pub use error::{Error, Result};
pub use file_kind::{FileKind, is_ignored_name};
pub use index::{FolderMeta, Hit, Index, IndexStats, SearchQuery, TagCount};
pub use indexer::{IndexerHandle, Job, Progress};
pub use listing::{CloudState, Entry, cloud_state, list_dir, resolve};
pub use meta::{DocMeta, MetaRecord, Storage, read_meta, write_meta};
pub use source::{Provider, Source, SourceStore};
pub use watch::{DirChange, LibraryWatcher};

/// Version of the library engine.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
