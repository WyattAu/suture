use crate::config::S3Config;
use crate::error::S3Error;
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

const S3_SERVICE: &str = "s3";
const AWS4_REQUEST: &str = "aws4_request";
const AWS4_HMAC_SHA256: &str = "AWS4-HMAC-SHA256";

pub fn sign_request(request: &mut reqwest::Request, config: &S3Config) -> Result<(), S3Error> {
    let now = Utc::now();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date_stamp = now.format("%Y%m%d").to_string();

    let payload_hash = if let Some(body) = request.body().as_ref() {
        match body.as_bytes() {
            Some(bytes) => hex::encode(Sha256::digest(bytes)),
            None => {
                let hash = Sha256::digest([]);
                hex::encode(hash)
            }
        }
    } else {
        let hash = Sha256::digest([]);
        hex::encode(hash)
    };

    let uri = request.url().path();
    let query = request.url().query().unwrap_or("");

    let canonical_uri = uri_encode(uri, false);
    let canonical_querystring = canonicalize_query(query);

    let mut headers: Vec<(String, String)> = request
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_lowercase(),
                v.to_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    headers.sort_by(|a, b| a.0.cmp(&b.0));

    let signed_headers_list: Vec<&str> = headers.iter().map(|(k, _)| k.as_str()).collect();
    let signed_headers = signed_headers_list.join(";");

    let canonical_headers: String = headers.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();

    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        request.method().as_str(),
        canonical_uri,
        canonical_querystring,
        canonical_headers,
        signed_headers,
        payload_hash
    );

    let credential_scope = format!(
        "{}/{}/{}/{}",
        date_stamp, config.region, S3_SERVICE, AWS4_REQUEST
    );

    let string_to_sign = format!(
        "{}\n{}\n{}\n{}",
        AWS4_HMAC_SHA256,
        amz_date,
        credential_scope,
        hex::encode(Sha256::digest(canonical_request.as_bytes()))
    );

    let signing_key = derive_signing_key(&config.secret_key, &date_stamp, &config.region)?;
    let signature = compute_hmac_hex(&signing_key, string_to_sign.as_bytes());

    let authorization = format!(
        "{} Credential={}/{}, SignedHeaders={}, Signature={}",
        AWS4_HMAC_SHA256, config.access_key, credential_scope, signed_headers, signature
    );

    request.headers_mut().insert(
        "x-amz-date",
        amz_date
            .parse()
            .map_err(|e: reqwest::header::InvalidHeaderValue| S3Error::Signing(e.to_string()))?,
    );
    request.headers_mut().insert(
        "x-amz-content-sha256",
        payload_hash
            .parse()
            .map_err(|e: reqwest::header::InvalidHeaderValue| S3Error::Signing(e.to_string()))?,
    );
    request.headers_mut().insert(
        "Authorization",
        authorization
            .parse()
            .map_err(|e: reqwest::header::InvalidHeaderValue| S3Error::Signing(e.to_string()))?,
    );

    Ok(())
}

fn derive_signing_key(
    secret_key: &str,
    date_stamp: &str,
    region: &str,
) -> Result<Vec<u8>, S3Error> {
    let k_date = compute_hmac(
        format!("AWS4{secret_key}").as_bytes(),
        date_stamp.as_bytes(),
    );
    let k_region = compute_hmac(&k_date, region.as_bytes());
    let k_service = compute_hmac(&k_region, S3_SERVICE.as_bytes());
    let k_signing = compute_hmac(&k_service, AWS4_REQUEST.as_bytes());
    Ok(k_signing)
}

fn compute_hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn compute_hmac_hex(key: &[u8], data: &[u8]) -> String {
    hex::encode(compute_hmac(key, data))
}

fn uri_encode(uri: &str, encode_slash: bool) -> String {
    let mut out = String::new();
    for byte in uri.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b'/' if !encode_slash => {
                out.push('/');
            }
            _ => {
                out.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    out
}

fn canonicalize_query(query: &str) -> String {
    if query.is_empty() {
        return String::new();
    }
    let mut pairs: Vec<(&str, &str)> = query
        .split('&')
        .filter(|s| !s.is_empty())
        .filter_map(|s| {
            let mut parts = s.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next().unwrap_or("");
            Some((key, value))
        })
        .collect();
    pairs.sort_by(|a, b| {
        let ka = uri_encode(a.0, true);
        let kb = uri_encode(b.0, true);
        ka.cmp(&kb)
            .then_with(|| uri_encode(a.1, true).cmp(&uri_encode(b.1, true)))
    });
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", uri_encode(k, true), uri_encode(v, true)))
        .collect::<Vec<_>>()
        .join("&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signing_key_derivation() {
        let key = derive_signing_key(
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            "20150830",
            "us-east-1",
        )
        .unwrap();
        // Verify the signing key is 32 bytes (SHA-256 HMAC output)
        assert_eq!(key.len(), 32, "signing key must be 32 bytes");
        // Verify deterministic: same inputs produce same output
        let key2 = derive_signing_key(
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            "20150830",
            "us-east-1",
        )
        .unwrap();
        assert_eq!(key, key2, "signing key derivation must be deterministic");
        // Verify different inputs produce different keys
        let key3 = derive_signing_key("different-secret-key", "20150830", "us-east-1").unwrap();
        assert_ne!(key, key3, "different secrets must produce different keys");
    }

    #[test]
    fn test_uri_encode_basic() {
        assert_eq!(uri_encode("/foo/bar", false), "/foo/bar");
        assert_eq!(uri_encode("/foo/bar", true), "%2Ffoo%2Fbar");
        assert_eq!(uri_encode("/foo?bar=baz", false), "/foo%3Fbar%3Dbaz");
    }

    #[test]
    fn test_canonical_query_string() {
        let result = canonicalize_query("prefix=suture/blobs/&list-type=2");
        assert_eq!(result, "list-type=2&prefix=suture%2Fblobs%2F");
    }

    #[test]
    fn test_canonical_query_empty() {
        assert_eq!(canonicalize_query(""), "");
    }

    #[test]
    fn test_signature_roundtrip() {
        let config = S3Config {
            endpoint: "https://examplestorage.com".to_string(),
            bucket: "test-bucket".to_string(),
            region: "us-east-1".to_string(),
            access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".to_string(),
            prefix: "suture/blobs/".to_string(),
            force_path_style: true,
        };

        let client = reqwest::Client::new();
        let url = format!("{}/test-bucket/suture/blobs/testkey", config.endpoint);
        let mut request = client.put(&url).body("test data").build().unwrap();

        sign_request(&mut request, &config).unwrap();

        let auth = request
            .headers()
            .get("Authorization")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(auth.starts_with("AWS4-HMAC-SHA256"));
        assert!(auth.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(auth.contains("us-east-1"));
        assert!(auth.contains("SignedHeaders="));
        assert!(auth.contains("Signature="));

        assert!(request.headers().contains_key("x-amz-date"));
        assert!(request.headers().contains_key("x-amz-content-sha256"));
    }

    #[test]
    fn test_canonical_request() {
        let config = S3Config {
            endpoint: "https://example.com".to_string(),
            bucket: "bucket".to_string(),
            region: "us-east-1".to_string(),
            access_key: "key".to_string(),
            secret_key: "secret".to_string(),
            prefix: "prefix/".to_string(),
            force_path_style: true,
        };

        let client = reqwest::Client::new();
        let mut request = client
            .get("https://example.com/bucket/prefix/key123")
            .build()
            .unwrap();

        sign_request(&mut request, &config).unwrap();

        let auth = request
            .headers()
            .get("Authorization")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(auth.starts_with("AWS4-HMAC-SHA256 Credential=key/"));
    }
}
