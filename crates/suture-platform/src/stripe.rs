// Copyright 2025 Suture Pty Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later OR (AGPL-3.0-or-later WITH Suture-Commercial-1.0)
//
// Licensed under the AGPL-3.0-or-later license OR the
// Suture Commercial License (for enterprise features).
// See LICENSE-AGPL and LICENSE-COMMERCIAL in the repo root.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Extension, Json,
};
use serde::{Deserialize, Serialize};

use crate::auth::Claims;
use crate::server::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutRequest {
    pub tier: String,
    pub success_url: Option<String>,
    pub cancel_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutResponse {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortalRequest {
    pub return_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortalResponse {
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionInfo {
    pub tier: String,
    pub status: String,
    pub current_period_end: Option<String>,
    pub cancel_at_period_end: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StripeWebhookEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub data: StripeEventData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StripeEventData {
    pub object: serde_json::Value,
}

fn stripe_key(state: &AppState) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    match &state.config.stripe_key {
        Some(key) if !key.is_empty() => Ok(key.clone()),
        _ => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "billing is not configured", "hint": "set STRIPE_KEY environment variable"})),
        )),
    }
}

fn platform_url(state: &AppState) -> String {
    if state.config.platform_url.is_empty() {
        "http://localhost:8080".to_owned()
    } else {
        state.config.platform_url.clone()
    }
}

/// Resolve a Stripe price ID from environment variables.
/// Returns an error if the price is not configured.
fn get_price_id(tier: &str) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    let env_key = match tier {
        "pro" => "STRIPE_PRICE_PRO",
        "enterprise" => "STRIPE_PRICE_ENTERPRISE",
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid tier", "valid": ["pro", "enterprise"]})),
            ));
        }
    };

    let price_id = std::env::var(env_key).unwrap_or_default();
    if price_id.is_empty() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": format!("price not configured — set {} environment variable", env_key)
            })),
        ));
    }

    Ok(price_id)
}

async fn get_or_create_customer(
    state: &AppState,
    user_id: &str,
    email: &str,
) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    let key = stripe_key(state)?;

    let existing: Option<String> = {
        let conn = state
            .db
            .conn()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))))?;
        conn.query_row(
            "SELECT stripe_customer_id FROM accounts WHERE user_id = ?1 AND stripe_customer_id IS NOT NULL",
            rusqlite::params![user_id],
            |row| row.get(0),
        )
        .ok()
        .flatten()
    };

    if let Some(id) = existing {
        return Ok(id);
    }

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.stripe.com/v1/customers")
        .basic_auth(&key, None::<&str>)
        .form(&[("email", email), ("metadata[user_id]", user_id)])
        .send()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    let customer: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    let customer_id = customer["id"].as_str().unwrap_or("").to_owned();
    if customer_id.is_empty() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "failed to create Stripe customer"})),
        ));
    }

    let conn = state
        .db
        .conn()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))))?;
    conn.execute(
        "UPDATE accounts SET stripe_customer_id = ?1, updated_at = datetime('now') WHERE user_id = ?2",
        rusqlite::params![customer_id, user_id],
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    Ok(customer_id)
}

pub async fn create_checkout_session(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<CheckoutRequest>,
) -> Result<Json<CheckoutResponse>, (StatusCode, Json<serde_json::Value>)> {
    let key = stripe_key(&state)?;
    let tier = req.tier.to_lowercase();
    let price_id = get_price_id(&tier)?;

    let customer_id = get_or_create_customer(&state, &claims.sub, &claims.email).await?;

    let base_url = platform_url(&state);
    let success_url = req
        .success_url
        .unwrap_or_else(|| format!("{base_url}/billing?success=true"));
    let cancel_url = req
        .cancel_url
        .unwrap_or_else(|| format!("{base_url}/billing?canceled=true"));

    let client = reqwest::Client::new();

    let resp = client
        .post("https://api.stripe.com/v1/checkout/sessions")
        .basic_auth(&key, None::<&str>)
        .form(&[
            ("mode", "subscription"),
            ("payment_method_types[0]", "card"),
            ("line_items[0][price]", price_id.as_str()),
            ("line_items[0][quantity]", "1"),
            ("customer", customer_id.as_str()),
            ("success_url", success_url.as_str()),
            ("cancel_url", cancel_url.as_str()),
            ("metadata[tier]", tier.as_str()),
            ("metadata[user_id]", claims.sub.as_str()),
        ])
        .send()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    let session: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    let url = session["url"].as_str().unwrap_or("").to_owned();
    let session_id = session["id"].as_str().unwrap_or("").to_owned();

    if url.is_empty() || session_id.is_empty() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "failed to create checkout session"})),
        ));
    }

    if let Ok(conn) = state.db.conn()
        && let Err(e) = conn.execute(
            "INSERT INTO checkout_sessions (session_id, user_id, tier, status, created_at) VALUES (?1, ?2, ?3, 'created', datetime('now'))",
            rusqlite::params![session_id, claims.sub, tier],
        )
    {
        tracing::error!("Failed to track checkout session: {}", e);
    }

    Ok(Json(CheckoutResponse { url }))
}

pub async fn create_portal_session_inner(
    state: &AppState,
    claims: &Claims,
) -> Result<Json<PortalResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let key = stripe_key(state)?;

    let customer_id: String = {
        let conn = state
            .db
            .conn()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))))?;

        conn.query_row(
            "SELECT stripe_customer_id FROM accounts WHERE user_id = ?1 AND stripe_customer_id IS NOT NULL",
            rusqlite::params![claims.sub],
            |row| row.get(0),
        )
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "no Stripe customer on file \u{2014} complete a checkout first"})),
            )
        })?
    };

    let base_url = platform_url(state);
    let return_url = format!("{base_url}/billing");

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.stripe.com/v1/billing_portal/sessions")
        .basic_auth(&key, None::<&str>)
        .form(&[("customer", customer_id.as_str()), ("return_url", return_url.as_str())])
        .send()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    let session: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    let url = session["url"].as_str().unwrap_or("").to_owned();
    if url.is_empty() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "failed to create portal session"})),
        ));
    }

    Ok(Json(PortalResponse { url }))
}

/// Handle incoming Stripe webhooks with signature verification.
pub async fn handle_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    match &state.config.stripe_key {
        Some(key) if !key.is_empty() => {
            let _ = key;
        }
        _ => {
            return Ok(Json(serde_json::json!({"received": true, "note": "billing not configured"})));
        }
    }

    let webhook_secret = std::env::var("STRIPE_WEBHOOK_SECRET").unwrap_or_default();
    if webhook_secret.is_empty() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "STRIPE_WEBHOOK_SECRET not configured"})),
        ));
    } else {
        let sig_header = headers
            .get("stripe-signature")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if let Err(e) = verify_stripe_signature(&body, sig_header, &webhook_secret) {
            tracing::warn!("Webhook signature verification failed: {}", e);
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "invalid webhook signature"})),
            ));
        }
    }

    let event: Result<StripeWebhookEvent, _> = serde_json::from_str(&body);
    match event {
        Ok(event) => {
            tracing::info!("Stripe webhook: {}", event.event_type);

            match event.event_type.as_str() {
                "checkout.session.completed" => {
                    handle_checkout_completed(&state, &event.data.object).await;
                }
                "customer.subscription.updated" => {
                    handle_subscription_updated(&state, &event.data.object).await;
                }
                "customer.subscription.deleted" => {
                    handle_subscription_deleted(&state, &event.data.object).await;
                }
                "invoice.payment_failed" => {
                    handle_payment_failed(&state, &event.data.object).await;
                }
                _ => {
                    tracing::debug!("Unhandled Stripe event: {}", event.event_type);
                }
            }

            Ok(Json(serde_json::json!({"received": true})))
        }
        Err(e) => {
            tracing::warn!("Invalid webhook payload: {}", e);
            Ok(Json(serde_json::json!({"received": true, "error": "invalid payload"})))
        }
    }
}

/// Verify a Stripe webhook signature using HMAC-SHA256.
///
/// Stripe signs webhooks with a `Stripe-Signature` header containing:
/// `t=<timestamp>,v1=<signature>` where signature = HMAC-SHA256(secret, timestamp + "." + payload).
fn verify_stripe_signature(payload: &str, sig_header: &str, secret: &str) -> Result<(), String> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut timestamp = None;
    let mut signatures = Vec::new();

    for part in sig_header.split(',') {
        let part = part.trim();
        if let Some(t) = part.strip_prefix("t=") {
            timestamp = Some(t.to_owned());
        } else if let Some(v) = part.strip_prefix("v1=") {
            signatures.push(v.to_owned());
        }
    }

    let timestamp = timestamp.ok_or_else(|| "missing timestamp in Stripe-Signature header".to_owned())?;
    if signatures.is_empty() {
        return Err("missing signature in Stripe-Signature header".to_owned());
    }

    if let Ok(ts) = timestamp.parse::<i64>() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        if (now - ts).abs() > 300 {
            return Err("webhook timestamp too old (possible replay attack)".to_owned());
        }
    }

    let signed_payload = format!("{timestamp}.{payload}");
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).map_err(|e| format!("HMAC error: {e}"))?;
    mac.update(signed_payload.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());

    fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        let result = a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y));
        result == 0
    }

    let expected_bytes = expected.as_bytes();
    for sig in &signatures {
        let sig_bytes = sig.as_bytes();
        if constant_time_eq(expected_bytes, sig_bytes) {
            return Ok(());
        }
    }

    Err("signature mismatch".to_owned())
}

async fn handle_checkout_completed(state: &AppState, object: &serde_json::Value) {
    let customer_id = object["customer"].as_str().unwrap_or("");
    let subscription_id = object["subscription"].as_str().unwrap_or("");
    let tier = object["metadata"]["tier"].as_str().unwrap_or("free");
    let user_id = object["metadata"]["user_id"].as_str().unwrap_or("");
    let session_id = object["id"].as_str().unwrap_or("");

    tracing::info!(
        "Checkout completed: customer={}, subscription={}, tier={}, user={}",
        customer_id,
        subscription_id,
        tier,
        user_id,
    );

    if let Ok(conn) = state.db.conn() {
        if !user_id.is_empty() {
            if let Err(e) = conn.execute(
                "UPDATE accounts SET tier = ?1, stripe_customer_id = ?2, stripe_subscription_id = ?3, payment_grace_until = NULL, updated_at = datetime('now') WHERE user_id = ?4",
                rusqlite::params![tier, customer_id, subscription_id, user_id],
            ) {
                tracing::error!("Failed to update account after checkout: {}", e);
            }

            if let Err(e) = conn.execute(
                "INSERT OR REPLACE INTO subscriptions (user_id, stripe_subscription_id, stripe_customer_id, tier, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'active', datetime('now'), datetime('now'))",
                rusqlite::params![user_id, subscription_id, customer_id, tier],
            ) {
                tracing::error!("Failed to insert subscription record: {}", e);
            }
        }

        if !session_id.is_empty()
            && let Err(e) = conn.execute(
                "UPDATE checkout_sessions SET status = 'completed' WHERE session_id = ?1",
                rusqlite::params![session_id],
            )
        {
            tracing::error!("Failed to update checkout session status: {}", e);
        }
    }
}

async fn handle_subscription_updated(state: &AppState, object: &serde_json::Value) {
    let status = object["status"].as_str().unwrap_or("unknown");
    let tier = match status {
        "active" | "trialing" => object["metadata"]["tier"].as_str().unwrap_or("free"),
        _ => "free",
    };
    let customer_id = object["customer"].as_str().unwrap_or("");
    let subscription_id = object["id"].as_str().unwrap_or("");

    tracing::info!("Subscription updated: status={}, tier={}", status, tier);

    if let Ok(conn) = state.db.conn() {
        if let Err(e) = conn.execute(
            "UPDATE accounts SET tier = ?1, updated_at = datetime('now') WHERE stripe_customer_id = ?2",
            rusqlite::params![tier, customer_id],
        ) {
            tracing::error!("Failed to update account after subscription change: {}", e);
        }

        if let Err(e) = conn.execute(
            "UPDATE subscriptions SET status = ?1, tier = ?2, updated_at = datetime('now') WHERE stripe_subscription_id = ?3",
            rusqlite::params![status, tier, subscription_id],
        ) {
            tracing::error!("Failed to update subscription record: {}", e);
        }
    }
}

async fn handle_subscription_deleted(state: &AppState, object: &serde_json::Value) {
    let customer_id = object["customer"].as_str().unwrap_or("");
    let subscription_id = object["id"].as_str().unwrap_or("");

    tracing::info!("Subscription deleted: customer={}", customer_id);

    if let Ok(conn) = state.db.conn() {
        if let Err(e) = conn.execute(
            "UPDATE accounts SET tier = 'free', stripe_subscription_id = NULL, payment_grace_until = NULL, updated_at = datetime('now') WHERE stripe_customer_id = ?1",
            rusqlite::params![customer_id],
        ) {
            tracing::error!("Failed to downgrade account after subscription deletion: {}", e);
        }

        if let Err(e) = conn.execute(
            "UPDATE subscriptions SET status = 'canceled', updated_at = datetime('now') WHERE stripe_subscription_id = ?1",
            rusqlite::params![subscription_id],
        ) {
            tracing::error!("Failed to update subscription status to canceled: {}", e);
        }
    }
}

/// Handle payment failure by setting a 7-day grace period.
/// Users keep their tier during the grace period, then are downgraded.
async fn handle_payment_failed(state: &AppState, object: &serde_json::Value) {
    let customer_id = object["customer"].as_str().unwrap_or("");
    let subscription_id = object["subscription"].as_str().unwrap_or("");
    tracing::warn!("Payment failed for customer: {}", customer_id);

    if let Ok(conn) = state.db.conn() {
        if let Err(e) = conn.execute(
            "UPDATE accounts SET payment_grace_until = datetime('now', '+7 days'), updated_at = datetime('now') WHERE stripe_customer_id = ?1 AND tier != 'free'",
            rusqlite::params![customer_id],
        ) {
            tracing::error!("Failed to set payment grace period: {}", e);
        }

        if !subscription_id.is_empty()
            && let Err(e) = conn.execute(
                "UPDATE subscriptions SET status = 'past_due', updated_at = datetime('now') WHERE stripe_subscription_id = ?1",
                rusqlite::params![subscription_id],
            )
        {
            tracing::error!("Failed to update subscription to past_due: {}", e);
        }
    }
}

pub async fn get_subscription(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<SubscriptionInfo>, (StatusCode, Json<serde_json::Value>)> {
    let conn = state
        .db
        .conn()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))))?;

    let tier: String = conn
        .query_row(
            "SELECT tier FROM accounts WHERE user_id = ?1",
            rusqlite::params![claims.sub],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "free".to_owned());

    let sub_status: Option<String> = conn
        .query_row(
            "SELECT status FROM subscriptions WHERE user_id = ?1",
            rusqlite::params![claims.sub],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    Ok(Json(SubscriptionInfo {
        tier: tier.clone(),
        status: sub_status.unwrap_or_else(|| {
            if tier == "free" {
                "inactive".to_owned()
            } else {
                "active".to_owned()
            }
        }),
        current_period_end: None,
        cancel_at_period_end: false,
    }))
}


