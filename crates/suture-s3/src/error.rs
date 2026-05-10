use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum S3Error {
    #[error("blob not found: {0}")]
    NotFound(String),

    #[error("access denied: {0}")]
    AccessDenied(String),

    #[error("connection error: {0}")]
    Connection(String),

    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("signing error: {0}")]
    Signing(String),

    #[error("I/O error: {0}")]
    Io(String),

    #[error("unexpected status {0}: {1}")]
    UnexpectedStatus(u16, String),
}

impl From<reqwest::Error> for S3Error {
    fn from(err: reqwest::Error) -> Self {
        if err.is_connect() {
            Self::Connection(err.to_string())
        } else {
            Self::Connection(format!("{err:#}"))
        }
    }
}
