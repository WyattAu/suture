//! Ed25519 patch signing and verification.
//!
//! Each patch is signed by its author's private key. The canonical
//! representation of a patch (operation type + touch set + target path
//! + payload + parent IDs + author + message + timestamp) is serialized
//!   to bytes and signed.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use suture_common::Hash;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SigningError {
    #[error("signature verification failed: {0}")]
    VerificationFailed(String),

    #[error("invalid signature: {0}")]
    InvalidSignature(#[from] ed25519_dalek::SignatureError),

    #[error("key error: {0}")]
    KeyError(String),


}

#[derive(Clone)]
pub struct SigningKeypair {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl SigningKeypair {
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    #[must_use] 
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }

    #[must_use] 
    pub fn private_key_bytes(&self) -> Vec<u8> {
        self.signing_key.to_bytes().to_vec()
    }

    #[must_use] 
    pub fn sign(&self, canonical_bytes: &[u8]) -> Signature {
        self.signing_key.sign(canonical_bytes)
    }

    #[must_use] 
    pub fn verifying_key(&self) -> &VerifyingKey {
        &self.verifying_key
    }
}

#[allow(clippy::too_many_arguments)]
#[must_use] 
pub fn canonical_patch_bytes(
    operation_type: &str,
    touch_set: &[String],
    target_path: &Option<String>,
    payload: &[u8],
    parent_ids: &[Hash],
    author: &str,
    message: &str,
    timestamp: u64,
) -> Vec<u8> {
    let mut buf = Vec::new();

    buf.extend_from_slice(operation_type.as_bytes());
    buf.push(0);

    let mut sorted_touches: Vec<&String> = touch_set.iter().collect();
    sorted_touches.sort();
    for touch in &sorted_touches {
        buf.extend_from_slice(touch.as_bytes());
        buf.push(0);
    }

    match target_path {
        Some(path) => {
            buf.extend_from_slice(path.as_bytes());
        }
        None => {
            buf.push(0xFF);
        }
    }
    buf.push(0);

    buf.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    buf.extend_from_slice(payload);

    let mut sorted_parents: Vec<&Hash> = parent_ids.iter().collect();
    sorted_parents.sort_by_key(|h| h.to_hex());
    for parent in &sorted_parents {
        buf.extend_from_slice(parent.to_hex().as_bytes());
        buf.push(0);
    }

    buf.extend_from_slice(&(timestamp.to_le_bytes()));
    buf.push(0);

    buf.extend_from_slice(author.as_bytes());
    buf.push(0);

    buf.extend_from_slice(message.as_bytes());

    buf
}

pub fn verify_signature(
    verifying_key_bytes: &[u8; 32],
    canonical_bytes: &[u8],
    signature_bytes: &[u8; 64],
) -> Result<(), SigningError> {
    let verifying_key = VerifyingKey::from_bytes(verifying_key_bytes)
        .map_err(|e| SigningError::VerificationFailed(e.to_string()))?;
    let signature = Signature::from_bytes(signature_bytes);
    verifying_key
        .verify(canonical_bytes, &signature)
        .map_err(|e| SigningError::VerificationFailed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::types::{OperationType, Patch, TouchSet};

    #[test]
    fn test_generate_keypair() {
        let kp = SigningKeypair::generate();
        assert_eq!(kp.public_key_bytes().len(), 32);
        assert_eq!(kp.private_key_bytes().len(), 32);
    }

    #[test]
    fn test_sign_and_verify() {
        let kp = SigningKeypair::generate();
        let data = b"hello, suture!";

        let signature = kp.sign(data);
        let result = verify_signature(&kp.public_key_bytes(), data, &signature.to_bytes());
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_wrong_data_fails() {
        let kp = SigningKeypair::generate();
        let data = b"hello, suture!";
        let wrong_data = b"hello, world!";

        let signature = kp.sign(data);
        let result = verify_signature(&kp.public_key_bytes(), wrong_data, &signature.to_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_wrong_key_fails() {
        let kp1 = SigningKeypair::generate();
        let kp2 = SigningKeypair::generate();
        let data = b"hello, suture!";

        let signature = kp1.sign(data);
        let result = verify_signature(&kp2.public_key_bytes(), data, &signature.to_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn test_canonical_patch_bytes_deterministic() {
        let bytes1 = canonical_patch_bytes(
            "Modify",
            &["a.txt".to_string(), "b.txt".to_string()],
            &Some("a.txt".to_string()),
            b"payload",
            &[],
            "alice",
            "test message",
            1000,
        );
        let bytes2 = canonical_patch_bytes(
            "Modify",
            &["b.txt".to_string(), "a.txt".to_string()],
            &Some("a.txt".to_string()),
            b"payload",
            &[],
            "alice",
            "test message",
            1000,
        );
        assert_eq!(bytes1, bytes2);
    }

    #[test]
    fn test_roundtrip_patch_signing() {
        let kp = SigningKeypair::generate();

        let patch = Patch::new(
            OperationType::Modify,
            TouchSet::single("test.txt"),
            Some("test.txt".to_string()),
            b"hello".to_vec(),
            vec![],
            "alice".to_string(),
            "test commit".to_string(),
        );

        let canonical = canonical_patch_bytes(
            &patch.operation_type.to_string(),
            &patch.touch_set.addresses(),
            &patch.target_path,
            &patch.payload,
            &patch.parent_ids,
            &patch.author,
            &patch.message,
            patch.timestamp,
        );

        let signature = kp.sign(&canonical);
        let result = verify_signature(&kp.public_key_bytes(), &canonical, &signature.to_bytes());
        assert!(result.is_ok());
    }
}
