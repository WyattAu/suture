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

pub async fn create_checkout_session(
    State(state): State<AppState>,
    Json(req): Json<CheckoutRequest>,
) -> Result<Json<CheckoutResponse>, (StatusCode, Json<serde_json::Value>)> {
    let stripe_key = match &state.config.stripe_key {
        Some(key) if !key.is_empty() => key.clone(),
        _ => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "billing is not configured", "hint": "set STRIPE_KEY environment variable"})),
            ));
        }
    };

    let tier = req.tier.to_lowercase();
    let price_id = get_price_id(&tier)?;

    let success_url = req.success_url.unwrap_or_else(|| "http://localhost:8080/billing?success=true".into());
    let cancel_url = req.cancel_url.unwrap_or_else(|| "http://localhost:8080/billing?canceled=true".into());

    let client = reqwest::Client::new();

    let resp = client
        .post("https://api.stripe.com/v1/checkout/sessions")
        .basic_auth(&stripe_key, None::<&str>)
        .form(&[
            ("mode", "subscription"),
            ("payment_method_types[0]", "card"),
            ("line_items[0][price]", price_id.as_str()),
            ("line_items[0][quantity]", "1"),
            ("success_url", success_url.as_str()),
            ("cancel_url", cancel_url.as_str()),
            ("metadata[tier]", tier.as_str()),
        ])
        .send()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    let session: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    let url = session["url"]
        .as_str()
        .unwrap_or("")
        .to_string();

    if url.is_empty() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "failed to create checkout session"})),
        ));
    }

    Ok(Json(CheckoutResponse { url }))
}

/// Create a Stripe Billing Portal session for managing subscriptions.
/// Called by the wrapper handler in server.rs which handles auth extraction.
pub async fn create_portal_session_inner(
    state: &AppState,
    claims: &Claims,
) -> Result<Json<PortalResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let stripe_key = match &state.config.stripe_key {
        Some(key) if !key.is_empty() => key.clone(),
        _ => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "billing is not configured"})),
            ));
        }
    };

    let conn = state
        .db
        .conn()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))))?;

    let customer_id: String = conn
        .query_row(
            "SELECT stripe_customer_id FROM accounts WHERE user_id = ?1 AND stripe_customer_id IS NOT NULL",
            rusqlite::params![claims.sub],
            |row| row.get(0),
        )
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "no Stripe customer on file — complete a checkout first"})),
            )
        })?;

    let return_url = "http://localhost:8080/billing".to_string();

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.stripe.com/v1/billing_portal/sessions")
        .basic_auth(&stripe_key, None::<&str>)
        .form(&[("customer", customer_id.as_str()), ("return_url", return_url.as_str())])
        .send()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    let session: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    let url = session["url"].as_str().unwrap_or("").to_string();
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
    };

    // Verify Stripe webhook signature to prevent forgery
    let webhook_secret = std::env::var("STRIPE_WEBHOOK_SECRET").unwrap_or_default();
    if !webhook_secret.is_empty() {
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
    } else {
        tracing::warn!("STRIPE_WEBHOOK_SECRET not set — skipping signature verification (INSECURE)");
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

    // Parse t=timestamp and v1=signature from the header
    let mut timestamp = None;
    let mut signatures = Vec::new();

    for part in sig_header.split(',') {
        let part = part.trim();
        if let Some(t) = part.strip_prefix("t=") {
            timestamp = Some(t.to_string());
        } else if let Some(v) = part.strip_prefix("v1=") {
            signatures.push(v.to_string());
        }
    }

    let timestamp = timestamp.ok_or_else(|| "missing timestamp in Stripe-Signature header".to_string())?;
    if signatures.is_empty() {
        return Err("missing signature in Stripe-Signature header".to_string());
    }

    // Reject events older than 5 minutes (replay attack protection)
    if let Ok(ts) = timestamp.parse::<i64>() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        if (now - ts).abs() > 300 {
            return Err("webhook timestamp too old (possible replay attack)".to_string());
        }
    }

    // Compute expected signature: HMAC-SHA256(secret, timestamp + "." + payload)
    let signed_payload = format!("{}.{}", timestamp, payload);
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).map_err(|e| format!("HMAC error: {}", e))?;
    mac.update(signed_payload.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());

    // Constant-time comparison against any provided signature
    let expected_bytes = expected.as_bytes();
    for sig in &signatures {
        let sig_bytes = sig.as_bytes();
        if expected_bytes.len() == sig_bytes.len() {
            let match_count = expected_bytes
                .iter()
                .zip(sig_bytes.iter())
                .filter(|(a, b)| a == b)
                .count();
            if match_count == expected_bytes.len() {
                return Ok(());
            }
        }
    }

    Err("signature mismatch".to_string())
}

async fn handle_checkout_completed(state: &AppState, object: &serde_json::Value) {
    let customer_id = object["customer"].as_str().unwrap_or("");
    let subscription_id = object["subscription"].as_str().unwrap_or("");
    let tier = object["metadata"]["tier"].as_str().unwrap_or("free");

    tracing::info!(
        "Checkout completed: customer={}, subscription={}, tier={}",
        customer_id,
        subscription_id,
        tier
    );

    if let Some(user_id) = object["metadata"]["user_id"].as_str()
        && let Ok(conn) = state.db.conn()
    {
        if let Err(e) = conn.execute(
            "UPDATE accounts SET tier = ?1, stripe_customer_id = ?2, stripe_subscription_id = ?3, updated_at = datetime('now') WHERE user_id = ?4",
            rusqlite::params![tier, customer_id, subscription_id, user_id],
        ) {
            tracing::error!("Failed to update account after checkout: {}", e);
        }
    }
}

async fn handle_subscription_updated(state: &AppState, object: &serde_json::Value) {
    let status = object["status"].as_str().unwrap_or("unknown");
    let tier = match status {
        "active" | "trialing" => object["metadata"]["tier"].as_str().unwrap_or("free"),
        _ => "free",
    };

    tracing::info!("Subscription updated: status={}, tier={}", status, tier);

    if let Some(customer_id) = object["customer"].as_str()
        && let Ok(conn) = state.db.conn()
    {
        if let Err(e) = conn.execute(
            "UPDATE accounts SET tier = ?1, updated_at = datetime('now') WHERE stripe_customer_id = ?2",
            rusqlite::params![tier, customer_id],
        ) {
            tracing::error!("Failed to update account after subscription change: {}", e);
        }
    }
}

async fn handle_subscription_deleted(state: &AppState, object: &serde_json::Value) {
    tracing::info!("Subscription deleted");

    if let Some(customer_id) = object["customer"].as_str()
        && let Ok(conn) = state.db.conn()
    {
        if let Err(e) = conn.execute(
            "UPDATE accounts SET tier = 'free', stripe_subscription_id = NULL, updated_at = datetime('now') WHERE stripe_customer_id = ?1",
            rusqlite::params![customer_id],
        ) {
            tracing::error!("Failed to downgrade account after subscription deletion: {}", e);
        }
    }
}

/// Handle payment failure by setting a 7-day grace period.
/// Users keep their tier during the grace period, then are downgraded.
async fn handle_payment_failed(state: &AppState, object: &serde_json::Value) {
    let customer_id = object["customer"].as_str().unwrap_or("");
    tracing::warn!("Payment failed for customer: {}", customer_id);

    if let Ok(conn) = state.db.conn() {
        if let Err(e) = conn.execute(
            "UPDATE accounts SET payment_grace_until = datetime('now', '+7 days'), updated_at = datetime('now') WHERE stripe_customer_id = ?1 AND tier != 'free'",
            rusqlite::params![customer_id],
        ) {
            tracing::error!("Failed to set payment grace period: {}", e);
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
        .unwrap_or_else(|_| "free".to_string());

    let has_subscription = conn
        .query_row(
            "SELECT stripe_subscription_id FROM accounts WHERE user_id = ?1 AND stripe_subscription_id IS NOT NULL",
            rusqlite::params![claims.sub],
            |row| row.get::<_, Option<String>>(0),
        )
        .unwrap_or(None)
        .is_some();

    Ok(Json(SubscriptionInfo {
        tier: tier.clone(),
        status: if has_subscription {
            "active".to_string()
        } else {
            "inactive".to_string()
        },
        current_period_end: None,
        cancel_at_period_end: false,
    }))
}

/// Extract user_id from the Authorization header by re-verifying the JWT.
/// This works around an Axum 0.8 extractor compatibility issue with
/// State + Extension + Json body extractors in a single handler.
fn extract_user_id_from_headers(
    headers: &HeaderMap,
    jwt_secret: &str,
) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    let auth_header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "missing authorization header"})),
            )
        })?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "invalid authorization format"})),
            )
        })?;

    let claims = crate::auth::verify_jwt(token, jwt_secret).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "invalid or expired token"})),
        )
    })?;

    Ok(claims.sub)
}
