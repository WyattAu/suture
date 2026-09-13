//! Integration tests for the `suture-platform` auth seams over the estate
//! crates: `salting` (password hashing) and `tokenkit` (JWT sign/verify).
//!
//! These lock the contracts `auth.rs` delegates to the kits: password
//! hash/verify round-trips, JWT issue/verify round-trips, and rejection of
//! tampered tokens and wrong secrets.

use suture_platform::auth::{create_jwt, hash_password, verify_jwt, verify_password};

const SECRET: &str = "integration-test-signing-secret";

#[test]
fn password_hash_and_verify_round_trip() {
    let hash = hash_password("correct horse battery staple").expect("hash");
    assert_ne!(
        hash, "correct horse battery staple",
        "hash is not plaintext"
    );
    assert!(verify_password("correct horse battery staple", &hash).expect("verify"));
}

#[test]
fn password_verify_rejects_wrong_password() {
    let hash = hash_password("right password").expect("hash");
    assert!(!verify_password("wrong password", &hash).expect("verify"));
}

#[test]
fn password_hashes_are_salted_per_call() {
    let a = hash_password("same input").expect("hash a");
    let b = hash_password("same input").expect("hash b");
    assert_ne!(a, b, "per-call salt must produce distinct hashes");
    assert!(verify_password("same input", &a).expect("verify a"));
    assert!(verify_password("same input", &b).expect("verify b"));
}

#[test]
fn jwt_issue_and_verify_round_trip() {
    let token = create_jwt("user-1", "user-1@suture.test", "pro", None, "admin", SECRET)
        .expect("create token");
    let claims = verify_jwt(&token, SECRET).expect("verify token");
    assert_eq!(claims.sub, "user-1");
    assert_eq!(claims.email, "user-1@suture.test");
    assert_eq!(claims.tier, "pro");
    assert_eq!(claims.role, "admin");
    assert!(claims.jti.is_some(), "jti is required for revocation");
}

#[test]
fn jwt_verify_rejects_wrong_secret() {
    let token = create_jwt(
        "user-2",
        "user-2@suture.test",
        "free",
        None,
        "member",
        SECRET,
    )
    .expect("create token");
    assert!(verify_jwt(&token, "not-the-signing-secret").is_err());
}

#[test]
fn jwt_verify_rejects_tampered_payload() {
    let free_token = create_jwt(
        "user-3",
        "user-3@suture.test",
        "free",
        None,
        "member",
        SECRET,
    )
    .expect("free");
    let pro_token = create_jwt(
        "user-4",
        "user-4@suture.test",
        "pro",
        None,
        "member",
        SECRET,
    )
    .expect("pro");

    // Splice the "pro" payload onto the "free" token's signature: a real
    // privilege-escalation shape. The signature check must reject it.
    let free_parts: Vec<&str> = free_token.split('.').collect();
    let pro_parts: Vec<&str> = pro_token.split('.').collect();
    assert_eq!(free_parts.len(), 3);
    assert_eq!(pro_parts.len(), 3);

    let spliced = format!("{}.{}.{}", free_parts[0], pro_parts[1], free_parts[2]);
    assert!(
        verify_jwt(&spliced, SECRET).is_err(),
        "spliced token must fail verification"
    );
}
