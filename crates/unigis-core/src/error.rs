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
    /// A value cannot be stored in the target (e.g. too wide for a dBASE column).
    /// ArcGIS skips such rows with WARNING 160277 "The row contains a bad value".
    #[error("the row contains a bad value: {0}")]
    BadValue(String),
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
