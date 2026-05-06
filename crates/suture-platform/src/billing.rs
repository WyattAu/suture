// Copyright 2025 Suture Pty Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later OR (AGPL-3.0-or-later WITH Suture-Commercial-1.0)
//
// Licensed under the AGPL-3.0-or-later license OR the
// Suture Commercial License (for enterprise features).
// See LICENSE-AGPL and LICENSE-COMMERCIAL in the repo root.

use axum::{Extension, Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};

use crate::auth::Claims;
use crate::db::PlatformDb;
use crate::server::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pro,
    Enterprise,
}

impl Tier {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Pro => "pro",
            Self::Enterprise => "enterprise",
        }
    }

    #[allow(clippy::should_implement_trait)]
    #[must_use]
    pub fn from_str(s: &str) -> Self {
        match s {
            "pro" => Self::Pro,
            "enterprise" => Self::Enterprise,
            _ => Self::Free,
        }
    }

    #[must_use]
    pub fn max_repos(&self) -> i64 {
        match self {
            Self::Free => 5,
            Self::Pro | Self::Enterprise => -1,
        }
    }

    #[must_use]
    pub fn max_merges_per_month(&self) -> i64 {
        match self {
            Self::Free => 100,
            Self::Pro => 10_000,
            Self::Enterprise => -1,
        }
    }

    #[must_use]
    pub fn max_storage_bytes(&self) -> i64 {
        match self {
            Self::Free => 100 * 1024 * 1024,
            Self::Pro => 10 * 1024 * 1024 * 1024,
            Self::Enterprise => 100 * 1024 * 1024 * 1024,
        }
    }

    #[must_use]
    pub fn max_drivers(&self) -> i64 {
        match self {
            Self::Free => 5,
            Self::Pro | Self::Enterprise => -1,
        }
    }

    #[must_use]
    pub fn price_cents_per_seat(&self) -> i64 {
        match self {
            Self::Free => 0,
            Self::Pro => 900,
            Self::Enterprise => 2900,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageReport {
    pub tier: Tier,
    pub merges_used: i64,
    pub merges_limit: i64,
    pub storage_bytes: i64,
    pub storage_limit: i64,
    pub repos_count: i64,
    pub repos_limit: i64,
    pub api_calls: i64,
    pub period: String,
    pub utilization_percent: f64,
}

pub fn get_usage(db: &PlatformDb, account_id: &str) -> anyhow::Result<UsageReport> {
    let conn = db
        .conn()
        .map_err(|e| anyhow::anyhow!("failed to get db connection for usage report: {e}"))?;

    let tier_str: String = conn.query_row(
        "SELECT tier FROM accounts WHERE user_id = ?1",
        rusqlite::params![account_id],
        |row| row.get(0),
    )?;
    let tier = Tier::from_str(&tier_str);

    let month = chrono::Utc::now().format("%Y-%m").to_string();
    let usage: (i64, i64, i64) = conn
        .query_row(
            "SELECT merges_used, storage_bytes, api_calls FROM usage WHERE account_id = ?1 AND month = ?2",
            rusqlite::params![account_id, month],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap_or((0, 0, 0));

    let merges_limit = tier.max_merges_per_month();
    let storage_limit = tier.max_storage_bytes();
    let repos_limit = tier.max_repos();

    let utilization_percent = if merges_limit > 0 {
        (usage.0 as f64 / merges_limit as f64) * 100.0
    } else {
        0.0
    };

    Ok(UsageReport {
        tier,
        merges_used: usage.0,
        merges_limit: if merges_limit > 0 { merges_limit } else { -1 },
        storage_bytes: usage.1,
        storage_limit,
        repos_count: 0,
        repos_limit: if repos_limit > 0 { repos_limit } else { -1 },
        api_calls: usage.2,
        period: month,
        utilization_percent,
    })
}

pub fn record_merge(db: &PlatformDb, account_id: &str) -> anyhow::Result<()> {
    let conn = db
        .conn()
        .map_err(|e| anyhow::anyhow!("failed to get db connection for recording merge: {e}"))?;
    let month = chrono::Utc::now().format("%Y-%m").to_string();
    conn.execute(
        "INSERT INTO usage (account_id, month, merges_used) VALUES (?1, ?2, 1)
         ON CONFLICT(account_id, month) DO UPDATE SET merges_used = merges_used + 1",
        rusqlite::params![account_id, month],
    )?;
    Ok(())
}

pub fn record_api_call(db: &PlatformDb, account_id: &str) -> anyhow::Result<()> {
    let conn = db
        .conn()
        .map_err(|e| anyhow::anyhow!("failed to get db connection for recording api call: {e}"))?;
    let month = chrono::Utc::now().format("%Y-%m").to_string();
    conn.execute(
        "INSERT INTO usage (account_id, month, api_calls) VALUES (?1, ?2, 1)
         ON CONFLICT(account_id, month) DO UPDATE SET api_calls = api_calls + 1",
        rusqlite::params![account_id, month],
    )?;
    Ok(())
}

pub fn can_merge(db: &PlatformDb, account_id: &str) -> anyhow::Result<bool> {
    let report = get_usage(db, account_id)?;
    if report.merges_limit < 0 {
        return Ok(true);
    }
    Ok(report.merges_used < report.merges_limit)
}

pub async fn usage_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<UsageReport>, (StatusCode, Json<serde_json::Value>)> {
    match get_usage(&state.db, &claims.sub) {
        Ok(report) => Ok(Json(report)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}
