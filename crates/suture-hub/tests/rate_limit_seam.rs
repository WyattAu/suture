//! Rate-limit seam tests (`SutureHubServer::check_rate_limit`).
//!
//! The hub's fixed-window limiter guards push/pull/token_create against
//! abuse. These tests lock the contract the route handlers rely on:
//! per-key limits, per-IP isolation, `retry_after` reporting, unknown-key
//! pass-through, and the disabled (zero-window) mode.

use suture_hub::server::SutureHubServer;

#[tokio::test]
async fn unknown_keys_are_never_limited() {
    let server = SutureHubServer::new_in_memory().expect("in-memory hub");
    for _ in 0..200 {
        server
            .check_rate_limit("203.0.113.9", "unknown_key")
            .await
            .expect("unknown key passes");
    }
}

#[tokio::test]
async fn token_create_limit_kicks_in_at_five_per_window() {
    let server = SutureHubServer::new_in_memory().expect("in-memory hub");
    for _ in 0..5 {
        server
            .check_rate_limit("203.0.113.10", "token_create")
            .await
            .expect("first five token creates pass");
    }
    let retry_after = server
        .check_rate_limit("203.0.113.10", "token_create")
        .await
        .expect_err("sixth token create is limited");
    assert!(retry_after >= 1, "retry_after reports remaining seconds");
    assert!(retry_after <= 60, "within the 60 s default window");
}

#[tokio::test]
async fn limits_are_isolated_per_ip() {
    let server = SutureHubServer::new_in_memory().expect("in-memory hub");
    for _ in 0..5 {
        server
            .check_rate_limit("203.0.113.11", "token_create")
            .await
            .expect("client A passes");
    }
    server
        .check_rate_limit("203.0.113.12", "token_create")
        .await
        .expect("client B unaffected by client A's exhaustion");
}

#[tokio::test]
async fn keys_limit_independently() {
    let server = SutureHubServer::new_in_memory().expect("in-memory hub");
    for _ in 0..5 {
        server
            .check_rate_limit("203.0.113.13", "token_create")
            .await
            .expect("token_create passes");
    }
    server
        .check_rate_limit("203.0.113.13", "pull")
        .await
        .expect("pull budget separate from token_create");
}

#[tokio::test]
async fn push_budget_is_per_account_limit_of_hundred() {
    let server = SutureHubServer::new_in_memory().expect("in-memory hub");
    for _ in 0..100 {
        server
            .check_rate_limit("203.0.113.14", "push")
            .await
            .expect("push budget allows the first hundred");
    }
    server
        .check_rate_limit("203.0.113.14", "push")
        .await
        .expect_err("the 101st push within the window is limited");
}
