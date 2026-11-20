use std::path::PathBuf;

use crate::i18n::{tr, trf};

// MARK: - Error

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("{}", trf("couldn't read or write settings: {0}", &[&.0]))]
    Json(#[from] serde_json::Error),

    #[error("{}", trf("file watching failed: {0}", &[&.0]))]
    Watch(#[from] notify_debouncer_mini::notify::Error),

    #[error("{}", trf("not a folder: {0}", &[&.0.display()]))]
    NotADirectory(PathBuf),

    #[error("{}", tr("this folder is already a source"))]
    DuplicateSource,

    #[error("{}", trf("this folder overlaps the source \"{0}\"; sources can't be nested", &[&.0]))]
    OverlappingSource(String),

    #[error("{}", tr("unknown source"))]
    UnknownSource,

    #[error("{}", trf("invalid path: {0}", &[&.0]))]
    InvalidPath(String),

    #[error("{0}")]
    InvalidValue(String),

    #[error("{}", trf("search index: {0}", &[&.0]))]
    Index(String),

    /// The PDF couldn't be parsed or the update didn't verify.
    #[error("{}", trf("PDF problem: {0}", &[&.0]))]
    Pdf(String),

    /// Rupert deliberately won't modify this file (e.g. encrypted or signed PDF).
    #[error("{0}")]
    Unsupported(String),

    /// The user stopped it.
    #[error("{}", tr("stopped"))]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, Error>;
