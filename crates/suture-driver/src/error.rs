use thiserror::Error;

#[derive(Debug, Error)]
pub enum DriverError {
    #[error("unsupported file extension: {0}")]
    UnsupportedExtension(String),

    #[error("parse error: {0}")]
    ParseError(String),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("driver not found for extension: {0}")]
    DriverNotFound(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
