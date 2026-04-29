use axum::{
    extract::State,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
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
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub fn check(&self, key: &str, limit: u32) -> (bool, u32, u64) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
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
                key.to_string(),
                RateLimitEntry {
                    count: 1,
                    window_start: now,
                },
            );
            (true, limit - 1, 60)
        }
    }

    pub fn cleanup(&self) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
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
    let (key, limit) = if let Some(claims) = request.extensions().get::<crate::auth::Claims>() {
        (format!("user:{}", claims.sub), tier_rate_limit(&claims.tier))
    } else {
        let ip = request
            .headers()
            .get("x-forwarded-for")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.split(',').next())
            .unwrap_or("unknown");
        (format!("ip:{}", ip), ANONYMOUS_RATE_LIMIT)
    };

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
        headers.insert("X-RateLimit-Limit", limit.to_string().parse().unwrap());
        headers.insert("X-RateLimit-Remaining", 0u32.to_string().parse().unwrap());
        headers.insert("X-RateLimit-Reset", reset_after.to_string().parse().unwrap());
        headers.insert("Retry-After", reset_after.to_string().parse().unwrap());

        return response;
    }

    let mut response = next.run(request).await;

    let headers = response.headers_mut();
    headers.insert("X-RateLimit-Limit", limit.to_string().parse().unwrap());
    headers.insert(
        "X-RateLimit-Remaining",
        remaining.to_string().parse().unwrap(),
    );
    headers.insert("X-RateLimit-Reset", reset_after.to_string().parse().unwrap());

    response
}
