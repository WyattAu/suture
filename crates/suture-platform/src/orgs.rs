// Copyright 2025 Suture Pty Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later OR (AGPL-3.0-or-later WITH Suture-Commercial-1.0)
//
// Licensed under the AGPL-3.0-or-later license OR the
// Suture Commercial License (for enterprise features).
// See LICENSE-AGPL and LICENSE-COMMERCIAL in the repo root.

use axum::{
    extract::{Extension, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::auth::Claims;
use crate::server::AppState;

#[derive(Debug, Serialize)]
pub struct OrgInfo {
    pub org_id: String,
    pub name: String,
    pub display_name: String,
    pub tier: String,
    pub member_count: i64,
    pub is_owner: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateOrgRequest {
    pub name: String,
    pub display_name: Option<String>,
}

pub async fn create_org(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<CreateOrgRequest>,
) -> Result<(StatusCode, Json<OrgInfo>), (StatusCode, Json<serde_json::Value>)> {
    if req.name.len() < 2
        || req.name.len() > 39
        || !req
            .name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "org name must be 2-39 alphanumeric characters (hyphens and underscores allowed)"})),
        ));
    }

    let org_id = uuid::Uuid::new_v4().to_string();
    let display_name = req.display_name.unwrap_or_else(|| req.name.clone());
    let conn = state.db.conn().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;

    conn.execute(
        "INSERT INTO orgs (org_id, name, display_name, owner_id) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![org_id, req.name, display_name, claims.sub],
    )
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("UNIQUE") {
            (
                StatusCode::CONFLICT,
                Json(serde_json::json!({"error": "organization name already taken"})),
            )
        } else {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": msg})),
            )
        }
    })?;

    conn.execute(
        "INSERT INTO org_members (org_id, user_id, role) VALUES (?1, ?2, 'owner')",
        rusqlite::params![org_id, claims.sub],
    )
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(OrgInfo {
            org_id,
            name: req.name,
            display_name,
            tier: "free".to_string(),
            member_count: 1,
            is_owner: true,
        }),
    ))
}

pub async fn list_my_orgs(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<OrgInfo>>, (StatusCode, Json<serde_json::Value>)> {
    let conn = state.db.conn().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;

    let mut stmt = conn
        .prepare(
            "SELECT o.org_id, o.name, o.display_name, o.tier,
                    (SELECT COUNT(*) FROM org_members WHERE org_id = o.org_id) as member_count,
                    om.role = 'owner' as is_owner
             FROM orgs o
             JOIN org_members om ON o.org_id = om.org_id
             WHERE om.user_id = ?1
             ORDER BY o.name",
        )
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;

    let orgs = stmt
        .query_map(rusqlite::params![claims.sub], |row| {
            Ok(OrgInfo {
                org_id: row.get(0)?,
                name: row.get(1)?,
                display_name: row.get(2)?,
                tier: row.get(3)?,
                member_count: row.get(4)?,
                is_owner: row.get::<_, bool>(5)?,
            })
        })
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;

    Ok(Json(orgs))
}
