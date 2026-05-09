use crate::error::S3Error;

use std::fmt::Write;
#[derive(Debug, Clone)]
pub struct S3Config {
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub prefix: String,
    pub force_path_style: bool,
}

impl Default for S3Config {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:9000".to_owned(),
            bucket: String::new(),
            region: "us-east-1".to_owned(),
            access_key: String::new(),
            secret_key: String::new(),
            prefix: "suture/blobs/".to_owned(),
            force_path_style: true,
        }
    }
}

impl S3Config {
    pub fn from_env() -> Result<Self, S3Error> {
        let endpoint =
            std::env::var("S3_ENDPOINT").unwrap_or_else(|_| "http://localhost:9000".to_owned());
        let bucket = std::env::var("S3_BUCKET")
            .map_err(|_| S3Error::InvalidConfig("S3_BUCKET environment variable not set".into()))?;
        let region = std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_owned());
        let access_key = std::env::var("S3_ACCESS_KEY").map_err(|_| {
            S3Error::InvalidConfig("S3_ACCESS_KEY environment variable not set".into())
        })?;
        let secret_key = std::env::var("S3_SECRET_KEY").map_err(|_| {
            S3Error::InvalidConfig("S3_SECRET_KEY environment variable not set".into())
        })?;
        let prefix = std::env::var("S3_PREFIX").unwrap_or_else(|_| "suture/blobs/".to_owned());
        let force_path_style = std::env::var("S3_FORCE_PATH_STYLE")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(true);

        Ok(Self {
            endpoint,
            bucket,
            region,
            access_key,
            secret_key,
            prefix,
            force_path_style,
        })
    }

    pub fn validate(&self) -> Result<(), S3Error> {
        if self.bucket.is_empty() {
            return Err(S3Error::InvalidConfig(
                "bucket name must not be empty".into(),
            ));
        }
        if self.access_key.is_empty() {
            return Err(S3Error::InvalidConfig(
                "access key must not be empty".into(),
            ));
        }
        if self.secret_key.is_empty() {
            return Err(S3Error::InvalidConfig(
                "secret key must not be empty".into(),
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn build_url(&self, object_key: &str) -> String {
        if self.force_path_style {
            format!(
                "{}/{}/{}",
                self.endpoint.trim_end_matches('/'),
                self.bucket,
                object_key
            )
        } else {
            let host = self
                .endpoint
                .trim_start_matches("https://")
                .trim_start_matches("http://");
            let scheme = if self.endpoint.starts_with("https") {
                "https"
            } else {
                "http"
            };
            format!("{scheme}://{}.{}/{}", self.bucket, host, object_key)
        }
    }

    #[must_use]
    pub fn list_url(&self) -> String {
        if self.force_path_style {
            format!(
                "{}/{}/?list-type=2&prefix={}",
                self.endpoint.trim_end_matches('/'),
                self.bucket,
                urlencoding(&self.prefix)
            )
        } else {
            let host = self
                .endpoint
                .trim_start_matches("https://")
                .trim_start_matches("http://");
            let scheme = if self.endpoint.starts_with("https") {
                "https"
            } else {
                "http"
            };
            format!(
                "{scheme}://{host}.{}/?list-type=2&prefix={}",
                host,
                urlencoding(&self.prefix)
            )
        }
    }
}

fn urlencoding(s: &str) -> String {
    let mut out = String::new();
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char);
            }
            _ => {
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = S3Config::default();
        assert_eq!(config.endpoint, "http://localhost:9000");
        assert_eq!(config.region, "us-east-1");
        assert_eq!(config.prefix, "suture/blobs/");
        assert!(config.force_path_style);
        assert!(config.bucket.is_empty());
    }

    #[test]
    fn test_config_from_env() {
        // SAFETY: `std::env::set_var`/`remove_var` is marked unsafe in Rust
        // because it modifies process-global state. This is acceptable here
        // because S3 configuration is loaded once at startup in a
        // single-threaded context before the server starts.
        unsafe {
            std::env::set_var("S3_ENDPOINT", "http://minio:9000");
            std::env::set_var("S3_BUCKET", "test-bucket");
            std::env::set_var("S3_REGION", "eu-west-1");
            std::env::set_var("S3_ACCESS_KEY", "test-key");
            std::env::set_var("S3_SECRET_KEY", "test-secret");
            std::env::set_var("S3_PREFIX", "custom/");
            std::env::set_var("S3_FORCE_PATH_STYLE", "false");
        }

        let config = S3Config::from_env().unwrap();
        assert_eq!(config.endpoint, "http://minio:9000");
        assert_eq!(config.bucket, "test-bucket");
        assert_eq!(config.region, "eu-west-1");
        assert_eq!(config.access_key, "test-key");
        assert_eq!(config.secret_key, "test-secret");
        assert_eq!(config.prefix, "custom/");
        assert!(!config.force_path_style);

        // SAFETY: `std::env::set_var`/`remove_var` is marked unsafe in Rust
        // because it modifies process-global state. This is acceptable here
        // because S3 configuration is loaded once at startup in a
        // single-threaded context before the server starts.
        unsafe {
            std::env::remove_var("S3_ENDPOINT");
            std::env::remove_var("S3_BUCKET");
            std::env::remove_var("S3_REGION");
            std::env::remove_var("S3_ACCESS_KEY");
            std::env::remove_var("S3_SECRET_KEY");
            std::env::remove_var("S3_PREFIX");
            std::env::remove_var("S3_FORCE_PATH_STYLE");
        }
    }

    #[test]
    fn test_config_validate() {
        let mut config = S3Config::default();
        assert!(config.validate().is_err());

        config.bucket = "my-bucket".to_string();
        assert!(config.validate().is_err());

        config.access_key = "key".to_string();
        assert!(config.validate().is_err());

        config.secret_key = "secret".to_string();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_build_url_path_style() {
        let config = S3Config {
            endpoint: "http://localhost:9000".to_string(),
            bucket: "my-bucket".to_string(),
            prefix: "suture/blobs/".to_string(),
            force_path_style: true,
            ..Default::default()
        };
        let url = config.build_url("suture/blobs/abc123");
        assert_eq!(url, "http://localhost:9000/my-bucket/suture/blobs/abc123");
    }

    #[test]
    fn test_build_url_virtual_hosted() {
        let config = S3Config {
            endpoint: "https://s3.amazonaws.com".to_string(),
            bucket: "my-bucket".to_string(),
            prefix: "suture/blobs/".to_string(),
            force_path_style: false,
            ..Default::default()
        };
        let url = config.build_url("suture/blobs/abc123");
        assert_eq!(
            url,
            "https://my-bucket.s3.amazonaws.com/suture/blobs/abc123"
        );
    }

    #[test]
    fn test_list_url() {
        let config = S3Config {
            endpoint: "http://localhost:9000".to_string(),
            bucket: "my-bucket".to_string(),
            prefix: "suture/blobs/".to_string(),
            force_path_style: true,
            ..Default::default()
        };
        let url = config.list_url();
        assert_eq!(
            url,
            "http://localhost:9000/my-bucket/?list-type=2&prefix=suture/blobs/"
        );
    }
}
