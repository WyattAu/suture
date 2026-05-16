// Copyright 2025 Suture Pty Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later OR (AGPL-3.0-or-later WITH Suture-Commercial-1.0)
//
// Licensed under the AGPL-3.0-or-later license OR the
// Suture Commercial License (for enterprise features).
// See LICENSE-AGPL and LICENSE-COMMERCIAL in the repo root.

use axum::{
    Json,
    extract::State,
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use crate::server::AppState;

const FREE_RATE_LIMIT: u32 = 30;
const PRO_RATE_LIMIT: u32 = 300;
const ENTERPRISE_RATE_LIMIT: u32 = 3000;
const ANONYMOUS_RATE_LIMIT: u32 = 10;

#[derive(Debug)]
struct RateLimitEntry {
    count: u32,
    window_start: Instant,
}

pub struct RateLimiter {
    entries: Mutex<HashMap<String, RateLimitEntry>>,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub fn check(&self, key: &str, limit: u32) -> (bool, u32, u64) {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();
        let window_duration = std::time::Duration::from_secs(60);

        if let Some(entry) = entries.get_mut(key) {
            if now.duration_since(entry.window_start) >= window_duration {
                entry.count = 1;
                entry.window_start = now;
                (true, limit - 1, 60)
            } else if entry.count < limit {
                entry.count += 1;
                let remaining = limit - entry.count;
                let reset_after = 60 - now.duration_since(entry.window_start).as_secs();
                (true, remaining, reset_after)
            } else {
                let reset_after = 60 - now.duration_since(entry.window_start).as_secs();
                (false, 0, reset_after)
            }
        } else {
            entries.insert(
                key.to_owned(),
                RateLimitEntry {
                    count: 1,
                    window_start: now,
                },
            );
            (true, limit - 1, 60)
        }
    }

    pub fn cleanup(&self) {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();
        let window_duration = std::time::Duration::from_secs(120);
        entries.retain(|_, entry| now.duration_since(entry.window_start) < window_duration);
    }
}

fn tier_rate_limit(tier: &str) -> u32 {
    match tier {
        "pro" => PRO_RATE_LIMIT,
        "enterprise" => ENTERPRISE_RATE_LIMIT,
        _ => FREE_RATE_LIMIT,
    }
}

pub async fn rate_limit(
    State(state): State<AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let (key, limit) = request
        .extensions()
        .get::<crate::auth::Claims>()
        .map_or_else(
            || {
                let ip = request
                    .headers()
                    .get("x-forwarded-for")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|h| h.split(',').next())
                    .unwrap_or("unknown");
                (format!("ip:{ip}"), ANONYMOUS_RATE_LIMIT)
            },
            |claims| {
                (
                    format!("user:{}", claims.sub),
                    tier_rate_limit(&claims.tier),
                )
            },
        );

    let (allowed, remaining, reset_after) = state.rate_limiter.check(&key, limit);

    if !allowed {
        let mut response = (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "error": "rate limit exceeded",
                "retry_after_seconds": reset_after,
                "limit": limit,
            })),
        )
            .into_response();

        let headers = response.headers_mut();
        headers.insert("X-RateLimit-Limit", HeaderValue::from(limit));
        headers.insert("X-RateLimit-Remaining", HeaderValue::from(0u32));
        headers.insert("X-RateLimit-Reset", HeaderValue::from(reset_after));
        headers.insert("Retry-After", HeaderValue::from(reset_after));

        return response;
    }

    let mut response = next.run(request).await;

    let headers = response.headers_mut();
    headers.insert("X-RateLimit-Limit", HeaderValue::from(limit));
    headers.insert("X-RateLimit-Remaining", HeaderValue::from(remaining));
    headers.insert("X-RateLimit-Reset", HeaderValue::from(reset_after));

    response
}
