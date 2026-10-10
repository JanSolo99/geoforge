use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid {format} data: {message}")]
    Format {
        format: &'static str,
        message: String,
    },
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("limit exceeded: {0}")]
    Limit(String),
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }

    pub fn format(format: &'static str, message: impl Into<String>) -> Self {
        Error::Format {
            format,
            message: message.into(),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
