//! Rupert's library engine.
//!
//! Everything that is not UI lives here so it can be tested without Tauri and
//! reused by other front ends (mobile viewer, CLI). See `docs/DESIGN.md`.

pub mod apple_ai;
pub mod assistant;
pub mod bookmark;
pub mod camera;
pub mod catalog;
pub mod chatgpt;
pub mod cloud;
pub mod error;
pub mod file_kind;
pub mod fileops;
pub mod find;
pub mod folder_info;
#[cfg(feature = "gdrive")]
pub mod gdrive;
pub mod i18n;
pub mod import;
pub mod index;
pub mod indexer;
pub mod intake;
pub mod listing;
pub mod llm;
pub mod markdown;
pub mod meta;
pub mod oauth;
pub mod ocr;
#[cfg(feature = "onedrive")]
pub mod onedrive;
pub mod pdf_meta;
pub mod print_entry;
pub mod qr;
pub mod remote;
pub mod rename;
pub mod scansnap;
pub mod settings;
pub mod share;
pub mod sidecar;
pub mod source;
pub mod tag_edit;
pub mod text;
pub mod text_layer;
pub mod thumbs;
pub mod watch;
pub mod xmp;

pub use error::{Error, Result};
pub use file_kind::{FileKind, is_ignored_name};
pub use index::{
    DocInfo, FolderMeta, Hit, Index, IndexStats, SearchQuery, TagCount, TagList, TagMode,
};
pub use indexer::{IndexerHandle, Job, Mode, Options, Progress};
pub use listing::{CloudState, Entry, all_dirs, cloud_state, list_dir, resolve};
pub use meta::{DocMeta, MetaRecord, Storage, read_meta, write_meta};
pub use source::{Provider, Source, SourceStore};
pub use watch::{DirChange, LibraryWatcher};

/// Version of the library engine.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
