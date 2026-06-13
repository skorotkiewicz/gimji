use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON error at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid content path: {0}")]
    InvalidPath(String),
    #[error("unsupported {kind} version {version}")]
    UnsupportedVersion { kind: &'static str, version: u32 },
    #[error("note not found: {0}")]
    NoteNotFound(String),
    #[error("tab not found: {0}")]
    TabNotFound(String),
    #[error("wrong content type for tab: expected {expected}, got {actual}")]
    WrongContentType {
        expected: &'static str,
        actual: &'static str,
    },
}

impl AppError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn json(path: impl Into<PathBuf>, source: serde_json::Error) -> Self {
        Self::Json {
            path: path.into(),
            source,
        }
    }
}
