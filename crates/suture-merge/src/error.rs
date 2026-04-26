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
            MergeError::UnsupportedFormat(fmt) => write!(f, "unsupported format: {fmt}"),
            MergeError::ParseError(msg) => write!(f, "parse error: {msg}"),
            MergeError::NoDriver(ext) => write!(f, "no driver available for extension: {ext}"),
        }
    }
}

impl std::error::Error for MergeError {}

impl From<DriverError> for MergeError {
    fn from(err: DriverError) -> Self {
        match err {
            DriverError::ParseError(msg) => MergeError::ParseError(msg),
            DriverError::DriverNotFound(ext) => MergeError::NoDriver(ext),
            DriverError::UnsupportedExtension(ext) => MergeError::UnsupportedFormat(ext),
            DriverError::SerializationError(msg) => MergeError::ParseError(msg),
            DriverError::IoError(e) => MergeError::ParseError(e.to_string()),
        }
    }
}

pub(crate) fn perform_merge(
    driver: &dyn SutureDriver,
    base: &str,
    ours: &str,
    theirs: &str,
) -> Result<MergeResult, MergeError> {
    match driver.merge(base, ours, theirs)? {
        Some(merged) => Ok(MergeResult {
            merged,
            status: MergeStatus::Clean,
        }),
        None => Ok(MergeResult {
            merged: ours.to_string(),
            status: MergeStatus::Conflict,
        }),
    }
}
