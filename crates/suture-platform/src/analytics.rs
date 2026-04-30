// Copyright 2025 Suture Pty Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later OR (AGPL-3.0-or-later WITH Suture-Commercial-1.0)
//
// Licensed under the AGPL-3.0-or-later license OR the
// Suture Commercial License (for enterprise features).
// See LICENSE-AGPL and LICENSE-COMMERCIAL in the repo root.

use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::collections::HashMap;

use crate::auth::Claims;
use crate::server::AppState;

pub fn log_merge(
    state: &AppState,
    user_id: &str,
    driver: &str,
    base_size: usize,
    ours_size: usize,
    theirs_size: usize,
    result_size: usize,
    has_conflict: bool,
    conflict_count: i64,
    merge_time_ms: i64,
) -> anyhow::Result<()> {
    let conn = state.db.conn().map_err(|e| anyhow::anyhow!("{e}"))?;
    conn.execute(
        "INSERT INTO merge_logs (user_id, driver, base_size, ours_size, theirs_size, result_size, has_conflict, conflict_count, merge_time_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            user_id,
            driver,
            base_size as i64,
            ours_size as i64,
            theirs_size as i64,
            result_size as i64,
            has_conflict as i64,
            conflict_count,
            merge_time_ms,
        ],
    )?;
    Ok(())
}

fn get_user_tier(state: &AppState, user_id: &str) -> Option<String> {
    let conn = state.db.conn().ok()?;
    conn.query_row(
        "SELECT tier FROM accounts WHERE user_id = ?1",
        rusqlite::params![user_id],
        |row| row.get::<_, String>(0),
    )
    .ok()
}

pub async fn analytics_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> impl IntoResponse {
    let tier = get_user_tier(&state, &claims.sub).unwrap_or_else(|| "free".to_string());
    if tier == "free" {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Analytics available on Pro plan"})),
        )
            .into_response();
    }

    let conn = match state.db.conn() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e})),
            )
                .into_response();
        }
    };

    let total_merges: i64 = conn
        .query_row("SELECT COUNT(*) FROM merge_logs WHERE user_id = ?1", rusqlite::params![claims.sub], |row| row.get(0))
        .unwrap_or(0);

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let merges_today: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM merge_logs WHERE user_id = ?1 AND date(created_at) = ?2",
            rusqlite::params![claims.sub, today],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let week_ago = (chrono::Utc::now() - chrono::Duration::days(7)).format("%Y-%m-%d").to_string();
    let merges_this_week: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM merge_logs WHERE user_id = ?1 AND date(created_at) >= ?2",
            rusqlite::params![claims.sub, week_ago],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let merges_by_driver: HashMap<String, i64> = conn
        .prepare("SELECT driver, COUNT(*) FROM merge_logs WHERE user_id = ?1 GROUP BY driver")
        .and_then(|mut stmt| {
            let rows: Vec<(String, i64)> = stmt
                .query_map(rusqlite::params![claims.sub], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows.into_iter().collect())
        })
        .unwrap_or_default();

    let merges_by_day: Vec<serde_json::Value> = conn
        .prepare(
            "SELECT date(created_at) as d, COUNT(*) FROM merge_logs WHERE user_id = ?1 AND date(created_at) >= date('now', '-30 days') GROUP BY d ORDER BY d",
        )
        .and_then(|mut stmt| {
            let rows: Vec<(String, i64)> = stmt
                .query_map(rusqlite::params![claims.sub], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows
                .into_iter()
                .map(|(date, count)| serde_json::json!({"date": date, "count": count}))
                .collect())
        })
        .unwrap_or_default();

    let conflicts_resolved: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM merge_logs WHERE user_id = ?1 AND has_conflict = 0",
            rusqlite::params![claims.sub],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let conflicts_detected: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM merge_logs WHERE user_id = ?1 AND has_conflict = 1",
            rusqlite::params![claims.sub],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let avg_merge_time: f64 = conn
        .query_row(
            "SELECT COALESCE(AVG(merge_time_ms), 0) FROM merge_logs WHERE user_id = ?1",
            rusqlite::params![claims.sub],
            |row| row.get(0),
        )
        .unwrap_or(0.0);

    let active_users_today: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT user_id) FROM merge_logs WHERE date(created_at) = ?1",
            rusqlite::params![today],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Json(serde_json::json!({
        "total_merges": total_merges,
        "merges_today": merges_today,
        "merges_this_week": merges_this_week,
        "merges_by_driver": merges_by_driver,
        "merges_by_day": merges_by_day,
        "conflicts_resolved": conflicts_resolved,
        "conflicts_detected": conflicts_detected,
        "avg_merge_time_ms": avg_merge_time,
        "active_users_today": active_users_today,
    }))
    .into_response()
}
