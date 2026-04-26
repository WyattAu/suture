// Allow collapsible_match: Rust 1.94/1.95 lint conflict (see suture-cli/src/main.rs)
#![allow(clippy::collapsible_match)]

pub mod config;
pub mod error;
pub mod signing;
pub mod store;

pub use config::S3Config;
pub use error::S3Error;
pub use store::S3BlobStore;
