use crate::config::S3Config;
use crate::error::S3Error;
use crate::signing::sign_request;
use suture_common::Hash;
use tracing::{debug, instrument};

pub struct S3BlobStore {
    config: S3Config,
    client: reqwest::Client,
}

impl S3BlobStore {
    #[must_use]
    pub fn new(config: S3Config) -> Self {
        let client = reqwest::Client::new();
        Self { config, client }
    }

    #[must_use]
    pub fn object_key(&self, hash: &Hash) -> String {
        let hex = hash.to_hex();
        format!("{}{}", self.config.prefix, hex)
    }

    #[instrument(skip(self, data), fields(hash = %hash))]
    pub async fn put_blob(&self, hash: &Hash, data: &[u8]) -> Result<(), S3Error> {
        let key = self.object_key(hash);
        let url = self.config.build_url(&key);
        debug!(%url, "PUT blob to S3");

        let mut request = self.client.put(&url).body(data.to_vec()).build()?;
        sign_request(&mut request, &self.config)?;

        let response = self.client.execute(request).await?;

        match response.status().as_u16() {
            200 | 201 => {
                debug!("PUT blob succeeded");
                Ok(())
            }
            403 => Err(S3Error::AccessDenied(format!("PUT {key}: access denied"))),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(S3Error::UnexpectedStatus(status, body))
            }
        }
    }

    #[instrument(skip(self), fields(hash = %hash))]
    pub async fn get_blob(&self, hash: &Hash) -> Result<Vec<u8>, S3Error> {
        let key = self.object_key(hash);
        let url = self.config.build_url(&key);
        debug!(%url, "GET blob from S3");

        let mut request = self.client.get(&url).build()?;
        sign_request(&mut request, &self.config)?;

        let response = self.client.execute(request).await?;

        match response.status().as_u16() {
            200 => {
                let data = response.bytes().await?.to_vec();
                debug!(size = data.len(), "GET blob succeeded");
                Ok(data)
            }
            404 => Err(S3Error::NotFound(key)),
            403 => Err(S3Error::AccessDenied(format!("GET {key}: access denied"))),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(S3Error::UnexpectedStatus(status, body))
            }
        }
    }

    #[instrument(skip(self), fields(hash = %hash))]
    pub async fn has_blob(&self, hash: &Hash) -> Result<bool, S3Error> {
        let key = self.object_key(hash);
        let url = self.config.build_url(&key);
        debug!(%url, "HEAD blob in S3");

        let mut request = self.client.head(&url).build()?;
        sign_request(&mut request, &self.config)?;

        let response = self.client.execute(request).await?;

        match response.status().as_u16() {
            200 => Ok(true),
            404 => Ok(false),
            403 => Err(S3Error::AccessDenied(format!("HEAD {key}: access denied"))),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(S3Error::UnexpectedStatus(status, body))
            }
        }
    }

    #[instrument(skip(self), fields(hash = %hash))]
    pub async fn delete_blob(&self, hash: &Hash) -> Result<(), S3Error> {
        let key = self.object_key(hash);
        let url = self.config.build_url(&key);
        debug!(%url, "DELETE blob from S3");

        let mut request = self.client.delete(&url).build()?;
        sign_request(&mut request, &self.config)?;

        let response = self.client.execute(request).await?;

        match response.status().as_u16() {
            204 => {
                debug!("DELETE blob succeeded");
                Ok(())
            }
            404 => Err(S3Error::NotFound(key)),
            403 => Err(S3Error::AccessDenied(format!(
                "DELETE {key}: access denied"
            ))),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(S3Error::UnexpectedStatus(status, body))
            }
        }
    }

    #[instrument(skip(self))]
    pub async fn list_blobs(&self) -> Result<Vec<Hash>, S3Error> {
        let url = self.config.list_url();
        debug!(%url, "LIST blobs from S3");

        let mut request = self.client.get(&url).build()?;
        sign_request(&mut request, &self.config)?;

        let response = self.client.execute(request).await?;

        match response.status().as_u16() {
            200 => {
                let body = response.text().await?;
                let hashes = parse_list_response(&body, &self.config.prefix);
                debug!(count = hashes.len(), "LIST blobs succeeded");
                Ok(hashes)
            }
            403 => Err(S3Error::AccessDenied("LIST: access denied".into())),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(S3Error::UnexpectedStatus(status, body))
            }
        }
    }
}

fn parse_list_response(xml: &str, prefix: &str) -> Vec<Hash> {
    let mut hashes = Vec::new();

    for part in xml.split("<Key>") {
        let part = part.trim();
        if let Some(end) = part.find("</Key>") {
            let key = &part[..end];
            if let Some(hex_str) = key.strip_prefix(prefix)
                && hex_str.len() == 64
                && let Ok(hash) = Hash::from_hex(hex_str)
            {
                hashes.push(hash);
            }
        }
    }

    hashes
}

#[cfg_attr(not(test), allow(dead_code))]
fn parse_error_response(xml: &str) -> Option<(String, String)> {
    let code = xml
        .split("<Code>")
        .nth(1)
        .and_then(|s| s.split("</Code>").next())
        .map(|s| s.trim().to_owned());
    let message = xml
        .split("<Message>")
        .nth(1)
        .and_then(|s| s.split("</Message>").next())
        .map(|s| s.trim().to_owned());
    code.zip(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> S3Config {
        S3Config {
            endpoint: "http://localhost:9000".to_string(),
            bucket: "test-bucket".to_string(),
            region: "us-east-1".to_string(),
            access_key: "test-key".to_string(),
            secret_key: "test-secret".to_string(),
            prefix: "suture/blobs/".to_string(),
            force_path_style: true,
        }
    }

    #[test]
    fn test_object_key_generation() {
        let store = S3BlobStore::new(make_config());
        let hex_str = "a".repeat(64);
        let hash = Hash::from_hex(&hex_str).unwrap();
        let key = store.object_key(&hash);
        assert_eq!(key, format!("suture/blobs/{hex_str}"));
    }

    #[test]
    fn test_object_key_custom_prefix() {
        let mut config = make_config();
        config.prefix = "custom/prefix/".to_string();
        let store = S3BlobStore::new(config);

        let hex_str = "f".repeat(64);
        let hash = Hash::from_hex(&hex_str).unwrap();
        let key = store.object_key(&hash);
        assert_eq!(key, format!("custom/prefix/{hex_str}"));
    }

    #[test]
    fn test_parse_list_response() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Name>test-bucket</Name>
    <Prefix>suture/blobs/</Prefix>
    <KeyCount>2</KeyCount>
    <Contents>
        <Key>suture/blobs/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa</Key>
        <Size>1024</Size>
    </Contents>
    <Contents>
        <Key>suture/blobs/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb</Key>
        <Size>2048</Size>
    </Contents>
    <Contents>
        <Key>suture/blobs/invalid</Key>
        <Size>100</Size>
    </Contents>
</ListBucketResult>"#;

        let hashes = parse_list_response(xml, "suture/blobs/");
        assert_eq!(hashes.len(), 2);

        let all_a = "a".repeat(64);
        let all_b = "b".repeat(64);
        assert_eq!(hashes[0], Hash::from_hex(&all_a).unwrap());
        assert_eq!(hashes[1], Hash::from_hex(&all_b).unwrap());
    }

    #[test]
    fn test_parse_list_response_empty() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Name>test-bucket</Name>
    <Prefix>suture/blobs/</Prefix>
    <KeyCount>0</KeyCount>
</ListBucketResult>"#;

        let hashes = parse_list_response(xml, "suture/blobs/");
        assert!(hashes.is_empty());
    }

    #[test]
    fn test_parse_error_response() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error>
    <Code>NoSuchKey</Code>
    <Message>The specified key does not exist.</Message>
    <Key>suture/blobs/abc123</Key>
</Error>"#;

        let (code, message) = parse_error_response(xml).unwrap();
        assert_eq!(code, "NoSuchKey");
        assert_eq!(message, "The specified key does not exist.");
    }

    #[test]
    fn test_parse_error_response_access_denied() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error>
    <Code>AccessDenied</Code>
    <Message>Access Denied</Message>
</Error>"#;

        let (code, message) = parse_error_response(xml).unwrap();
        assert_eq!(code, "AccessDenied");
        assert_eq!(message, "Access Denied");
    }

    #[test]
    fn test_store_new() {
        let store = S3BlobStore::new(make_config());
        let hex_str = "c".repeat(64);
        let hash = Hash::from_hex(&hex_str).unwrap();
        assert_eq!(store.object_key(&hash), format!("suture/blobs/{hex_str}"));
    }
}
