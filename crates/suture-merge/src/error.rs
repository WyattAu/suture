use suture_driver::{DriverError, SutureDriver};

#[derive(Debug, Clone)]
pub struct MergeResult {
    pub merged: String,
    pub status: MergeStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStatus {
    Clean,
    Conflict,
}

#[derive(Debug)]
pub enum MergeError {
    UnsupportedFormat(String),
    ParseError(String),
    NoDriver(String),
}

impl std::fmt::Display for MergeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedFormat(fmt) => write!(f, "unsupported format: {fmt}"),
            Self::ParseError(msg) => write!(f, "parse error: {msg}"),
            Self::NoDriver(ext) => write!(f, "no driver available for extension: {ext}"),
        }
    }
}

impl std::error::Error for MergeError {}

impl From<DriverError> for MergeError {
    fn from(err: DriverError) -> Self {
        match err {
            DriverError::DriverNotFound(ext) => Self::NoDriver(ext),
            DriverError::UnsupportedExtension(ext) => Self::UnsupportedFormat(ext),
            DriverError::ParseError(msg) | DriverError::SerializationError(msg) => Self::ParseError(msg),
            DriverError::IoError(e) => Self::ParseError(e.to_string()),
        }
    }
}

pub fn perform_merge(
    driver: &dyn SutureDriver,
    base: &str,
    ours: &str,
    theirs: &str,
) -> Result<MergeResult, MergeError> {
    Ok(driver.merge(base, ours, theirs)?.map_or_else(
        || MergeResult {
            merged: ours.to_owned(),
            status: MergeStatus::Conflict,
        },
        |merged| MergeResult {
            merged,
            status: MergeStatus::Clean,
        },
    ))
}
