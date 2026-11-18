use std::path::PathBuf;

// MARK: - Error

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("couldn't read or write settings: {0}")]
    Json(#[from] serde_json::Error),

    #[error("file watching failed: {0}")]
    Watch(#[from] notify_debouncer_mini::notify::Error),

    #[error("not a folder: {}", .0.display())]
    NotADirectory(PathBuf),

    #[error("this folder is already a source")]
    DuplicateSource,

    #[error("this folder overlaps the source \"{0}\"; sources can't be nested")]
    OverlappingSource(String),

    #[error("unknown source")]
    UnknownSource,

    #[error("invalid path: {0}")]
    InvalidPath(String),
}

pub type Result<T> = std::result::Result<T, Error>;
