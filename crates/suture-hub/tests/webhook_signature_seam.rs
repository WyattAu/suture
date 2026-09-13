//! Webhook signature seam tests (`suture_hub::webhooks::WebhookManager`).
//!
//! Locks the signature contract the delivery paths rely on: `sign_payload`
//! returns the bare hex digest, delivery prefixes it as `sha256=` (GitHub
//! wire format) in the `X-Suture-Signature` header, and `verify_signature`
//! accepts exactly that form. Verification delegates to the `webhookkit`
//! estate crate's constant-time comparison — these tests pin round-trips,
//! secret/payload sensitivity, prefix discipline, and malformed inputs.

use suture_hub::webhooks::WebhookManager;

const PAYLOAD: &str = r#"{"event":"push","repo_id":"r1","timestamp":1}"#;
const SECRET: &str = "whsec-shared-secret";

/// Mirrors the delivery paths: sign, then prefix for the wire.
fn wire_signature(manager: &WebhookManager, payload: &str, secret: &str) -> String {
    format!(
        "sha256={}",
        manager
            .sign_payload(payload, secret)
            .expect("HMAC accepts any key size")
    )
}

#[test]
fn sign_then_verify_round_trips_over_the_wire() {
    let manager = WebhookManager::new();
    let digest = manager.sign_payload(PAYLOAD, SECRET).expect("signs");
    assert_eq!(digest.len(), 64, "bare hex SHA-256 digest");

    let signature = wire_signature(&manager, PAYLOAD, SECRET);
    assert!(signature.starts_with("sha256="), "GitHub wire format");
    assert_eq!(signature.len(), "sha256=".len() + 64);
    assert!(
        WebhookManager::verify_signature(PAYLOAD, SECRET, &signature),
        "honest signature verifies"
    );
}

#[test]
fn signatures_are_deterministic_and_secret_bound() {
    let manager = WebhookManager::new();
    let first = manager.sign_payload(PAYLOAD, SECRET).expect("signs");
    let again = manager.sign_payload(PAYLOAD, SECRET).expect("signs");
    assert_eq!(first, again, "same input, same signature");

    let other_secret = manager.sign_payload(PAYLOAD, "whsec-other").expect("signs");
    assert_ne!(first, other_secret, "secret changes the signature");
}

#[test]
fn wrong_secret_rejects() {
    let manager = WebhookManager::new();
    let signature = wire_signature(&manager, PAYLOAD, SECRET);
    assert!(!WebhookManager::verify_signature(
        PAYLOAD,
        "whsec-wrong",
        &signature
    ));
}

#[test]
fn tampered_payload_rejects() {
    let manager = WebhookManager::new();
    let signature = wire_signature(&manager, PAYLOAD, SECRET);
    let tampered = PAYLOAD.replace("\"timestamp\":1", "\"timestamp\":2");
    assert_ne!(tampered, PAYLOAD);
    assert!(!WebhookManager::verify_signature(
        &tampered, SECRET, &signature
    ));
}

#[test]
fn tampered_signature_rejects() {
    let manager = WebhookManager::new();
    let signature = wire_signature(&manager, PAYLOAD, SECRET);
    let mut chars: Vec<char> = signature.chars().collect();
    let last = chars.len() - 1;
    chars[last] = if chars[last] == '0' { '1' } else { '0' };
    let flipped: String = chars.into_iter().collect();
    assert!(!WebhookManager::verify_signature(PAYLOAD, SECRET, &flipped));
}

#[test]
fn missing_prefix_rejects() {
    let manager = WebhookManager::new();
    let digest = manager.sign_payload(PAYLOAD, SECRET).expect("signs");
    assert!(
        !WebhookManager::verify_signature(PAYLOAD, SECRET, &digest),
        "bare hex without the sha256= prefix is not accepted"
    );
    assert!(!WebhookManager::verify_signature(
        PAYLOAD,
        SECRET,
        "v1=deadbeef"
    ));
}

#[test]
fn malformed_signatures_reject() {
    for bad in [
        "",
        "sha256=",
        "sha256=not-hex",
        "sha256=00",
        "sha256=0000000000000000000000000000000000000000000000000000000000000000",
    ] {
        assert!(
            !WebhookManager::verify_signature(PAYLOAD, SECRET, bad),
            "malformed signature {bad:?} must reject"
        );
    }
}

#[test]
fn empty_payload_and_secret_still_round_trip() {
    let manager = WebhookManager::new();
    let signature = wire_signature(&manager, "", SECRET);
    assert!(WebhookManager::verify_signature("", SECRET, &signature));
    assert!(!WebhookManager::verify_signature("", "other", &signature));
}
