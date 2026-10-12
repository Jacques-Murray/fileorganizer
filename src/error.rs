use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FoError {
    #[error("config not found at {0}; run `fo init` to create one")]
    ConfigNotFound(PathBuf),

    #[error("invalid config: {0}")]
    InvalidConfig(String),

    #[error("config file already exists at {0} (use --force to overwrite)")]
    ConfigExists(PathBuf),

    #[error("{0} is not a directory")]
    NotADirectory(PathBuf),

    #[error("no journals found in {0}")]
    NoJournal(PathBuf),

    #[error("could not determine the platform config/home directory")]
    NoHomeDir,

    #[error("I/O error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl FoError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        FoError::Io {
            path: path.into(),
            source,
        }
    }
}
