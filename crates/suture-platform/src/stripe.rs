// Copyright 2025 Suture Pty Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later OR (AGPL-3.0-or-later WITH Suture-Commercial-1.0)
//
// Licensed under the AGPL-3.0-or-later license OR the
// Suture Commercial License (for enterprise features).
// See LICENSE-AGPL and LICENSE-COMMERCIAL in the repo root.

use axum::{
    extract::State,
    http::StatusCode,
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

const PRICE_PRO_MONTHLY: &str = "price_placeholder_pro";
const PRICE_ENTERPRISE_MONTHLY: &str = "price_placeholder_enterprise";

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
    let price_id = match tier.as_str() {
        "pro" => PRICE_PRO_MONTHLY,
        "enterprise" => PRICE_ENTERPRISE_MONTHLY,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid tier", "valid": ["pro", "enterprise"]})),
            ));
        }
    };

    let success_url = req.success_url.unwrap_or_else(|| "http://localhost:8080/billing?success=true".into());
    let cancel_url = req.cancel_url.unwrap_or_else(|| "http://localhost:8080/billing?canceled=true".into());

    let client = reqwest::Client::new();

    let resp = client
        .post("https://api.stripe.com/v1/checkout/sessions")
        .basic_auth(&stripe_key, None::<&str>)
        .form(&[
            ("mode", "subscription"),
            ("payment_method_types[0]", "card"),
            ("line_items[0][price]", price_id),
            ("line_items[0][quantity]", "1"),
            ("success_url", &success_url),
            ("cancel_url", &cancel_url),
            ("metadata[tier]", &tier),
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

pub async fn create_portal_session(
    State(state): State<AppState>,
    Json(_req): Json<PortalRequest>,
) -> Result<Json<PortalResponse>, (StatusCode, Json<serde_json::Value>)> {
    match &state.config.stripe_key {
        Some(key) if !key.is_empty() => { let _ = key; }
        _ => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "billing is not configured"})),
            ));
        }
    };

    Err((
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "error": "billing portal requires Stripe customer setup",
            "hint": "complete a checkout session first"
        })),
    ))
}

pub async fn handle_webhook(
    State(state): State<AppState>,
    body: String,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    match &state.config.stripe_key {
        Some(key) if !key.is_empty() => { let _ = key; }
        _ => {
            return Ok(Json(serde_json::json!({"received": true, "note": "billing not configured"})));
        }
    };

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
                    tracing::warn!("Payment failed for subscription");
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

async fn handle_checkout_completed(state: &AppState, object: &serde_json::Value) {
    let customer_id = object["customer"].as_str().unwrap_or("");
    let subscription_id = object["subscription"].as_str().unwrap_or("");
    let tier = object["metadata"]["tier"].as_str().unwrap_or("free");

    tracing::info!(
        "Checkout completed: customer={}, subscription={}, tier={}",
        customer_id, subscription_id, tier
    );

    if let Some(user_id) = object["metadata"]["user_id"].as_str()
        && let Ok(conn) = state.db.conn()
    {
        let _ = conn.execute(
            "UPDATE accounts SET tier = ?1, stripe_customer_id = ?2, stripe_subscription_id = ?3, updated_at = datetime('now') WHERE user_id = ?4",
            rusqlite::params![tier, customer_id, subscription_id, user_id],
        );
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
        let _ = conn.execute(
            "UPDATE accounts SET tier = ?1, updated_at = datetime('now') WHERE stripe_customer_id = ?2",
            rusqlite::params![tier, customer_id],
        );
    }
}

async fn handle_subscription_deleted(state: &AppState, object: &serde_json::Value) {
    tracing::info!("Subscription deleted");

    if let Some(customer_id) = object["customer"].as_str()
        && let Ok(conn) = state.db.conn()
    {
        let _ = conn.execute(
            "UPDATE accounts SET tier = 'free', stripe_subscription_id = NULL, updated_at = datetime('now') WHERE stripe_customer_id = ?1",
            rusqlite::params![customer_id],
        );
    }
}

pub async fn get_subscription(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<SubscriptionInfo>, (StatusCode, Json<serde_json::Value>)> {
    let conn = state.db.conn()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))))?;

    let tier: String = conn.query_row(
        "SELECT tier FROM accounts WHERE user_id = ?1",
        rusqlite::params![claims.sub],
        |row| row.get(0),
    ).unwrap_or_else(|_| "free".to_string());

    let has_subscription = conn.query_row(
        "SELECT stripe_subscription_id FROM accounts WHERE user_id = ?1 AND stripe_subscription_id IS NOT NULL",
        rusqlite::params![claims.sub],
        |row| row.get::<_, Option<String>>(0),
    ).unwrap_or(None).is_some();

    Ok(Json(SubscriptionInfo {
        tier: tier.clone(),
        status: if has_subscription { "active".to_string() } else { "inactive".to_string() },
        current_period_end: None,
        cancel_at_period_end: false,
    }))
}
