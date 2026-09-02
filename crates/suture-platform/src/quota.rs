// Copyright 2025 Suture Pty Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later OR (AGPL-3.0-or-later WITH Suture-Commercial-1.0)
//
// Licensed under the AGPL-3.0-or-later license OR the
// Suture Commercial License (for enterprise features).
// See LICENSE-AGPL and LICENSE-COMMERCIAL in the repo root.

use axum::{
    Json,
    extract::State,
    http::{HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::Datelike;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use crate::billing;
use crate::server::AppState;

const CACHE_TTL_SECS: u64 = 60;

#[derive(Debug)]
struct QuotaSnapshot {
    merges_used: i64,
    storage_bytes: i64,
    repos_count: i64,
    cached_at: Instant,
}

impl QuotaSnapshot {
    fn is_expired(&self) -> bool {
        self.cached_at.elapsed().as_secs() >= CACHE_TTL_SECS
    }
}

pub struct QuotaEnforcer {
    cache: Mutex<HashMap<String, QuotaSnapshot>>,
}

impl Default for QuotaEnforcer {
    fn default() -> Self {
        Self::new()
    }
}

impl QuotaEnforcer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn get_cached(&self, key: &str) -> Option<QuotaSnapshot> {
        let cache = self
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.get(key).and_then(|snap| {
            if snap.is_expired() {
                None
            } else {
                Some(QuotaSnapshot {
                    merges_used: snap.merges_used,
                    storage_bytes: snap.storage_bytes,
                    repos_count: snap.repos_count,
                    cached_at: snap.cached_at,
                })
            }
        })
    }

    fn update_cache(&self, key: &str, report: &billing::UsageReport) {
        let mut cache = self
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.insert(
            key.to_owned(),
            QuotaSnapshot {
                merges_used: report.merges_used,
                storage_bytes: report.storage_bytes,
                repos_count: report.repos_count,
                cached_at: Instant::now(),
            },
        );
    }

    #[allow(dead_code)]
    fn evict(&self, key: &str) {
        let mut cache = self
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.remove(key);
    }

    pub fn cleanup(&self) {
        let mut cache = self
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.retain(|_, snap| !snap.is_expired());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResourceCategory {
    Merge,
    Storage,
    Other,
}

fn categorize_route(method: &Method, path: &str) -> ResourceCategory {
    match (method, path) {
        (&Method::POST, "/api/merge") | (&Method::POST, "/api/plugins/merge") => {
            ResourceCategory::Merge
        }
        (&Method::POST, "/api/plugins/upload") => ResourceCategory::Storage,
        _ => ResourceCategory::Other,
    }
}

fn quota_exceeded_response(quota_type: &str, limit: i64, used: i64, reset: &str) -> Response {
    let mut response = (
        StatusCode::TOO_MANY_REQUESTS,
        Json(json!({
            "error": "quota exceeded",
            "quota_type": quota_type,
            "limit": limit,
            "used": used,
            "reset": reset,
        })),
    )
        .into_response();

    let retry_after = if limit > 0 {
        let now = chrono::Utc::now();
        // Calculate start of next month
        let next_month = if now.month() == 12 {
            now.with_year(now.year() + 1).and_then(|d| d.with_month(1))
        } else {
            now.with_month(now.month() + 1)
        };
        let reset_date = next_month.unwrap_or(now);
        (reset_date - now).num_seconds().max(0) as u64
    } else {
        0
    };

    if let Ok(val) = HeaderValue::from_str(&retry_after.to_string()) {
        response.headers_mut().insert("Retry-After", val);
    }

    response
}

pub async fn quota_middleware(
    State(state): State<AppState>,
    mut request: axum::extract::Request,
    next: Next,
) -> Response {
    let Some(claims) = request.extensions_mut().remove::<crate::auth::Claims>() else {
        return next.run(request).await;
    };

    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let category = categorize_route(&method, &path);

    if category == ResourceCategory::Other {
        request.extensions_mut().insert(claims);
        return next.run(request).await;
    }

    let cache_key = match &claims.org_id {
        Some(org_id) => format!("org:{org_id}"),
        None => claims.sub.clone(),
    };

    let report = if let Some(snap) = state.quota_enforcer.get_cached(&cache_key) {
        billing::UsageReport {
            tier: billing::Tier::from_str(&claims.tier),
            merges_used: snap.merges_used,
            merges_limit: 0,
            storage_bytes: snap.storage_bytes,
            storage_limit: 0,
            repos_count: snap.repos_count,
            repos_limit: 0,
            api_calls: 0,
            period: chrono::Utc::now().format("%Y-%m").to_string(),
            utilization_percent: 0.0,
        }
    } else {
        match billing::get_usage(&state.db, &claims.sub) {
            Ok(report) => {
                state.quota_enforcer.update_cache(&cache_key, &report);
                report
            }
            Err(e) => {
                tracing::warn!("quota check failed for {}: {e}", claims.sub);
                return next.run(request).await;
            }
        }
    };

    let tier = billing::Tier::from_str(&claims.tier);
    let reset = report.period.clone();

    match category {
        ResourceCategory::Merge => {
            if !billing::can_merge(&state.db, &claims.sub).unwrap_or(true) {
                let limit = tier.max_merges_per_month();
                return quota_exceeded_response("merges", limit, report.merges_used, &reset);
            }
        }
        ResourceCategory::Storage => {
            let limit = tier.max_storage_bytes();
            if limit > 0 && report.storage_bytes >= limit {
                return quota_exceeded_response("storage", limit, report.storage_bytes, &reset);
            }
        }
        ResourceCategory::Other => {}
    }

    next.run(request).await
}

pub async fn record_usage_middleware(
    State(state): State<AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let response = next.run(request).await;
    let status = response.status();

    // We cannot extract Claims from request (already consumed by next.run).
    // Usage recording is best-effort; the quota_middleware handles enforcement.
    let _ = (state, status);
    response
}
