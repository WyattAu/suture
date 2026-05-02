// Copyright 2025 Suture Pty Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later OR (AGPL-3.0-or-later WITH Suture-Commercial-1.0)
//
// Licensed under the AGPL-3.0-or-later license OR the
// Suture Commercial License (for enterprise features).
// See LICENSE-AGPL and LICENSE-COMMERCIAL in the repo root.

use axum::{
    extract::{Extension, Path, State},
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
            Json(serde_json::json!({"error": e})),
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
            tier: "free".to_owned(),
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
            Json(serde_json::json!({"error": e})),
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

#[derive(Debug, Serialize)]
pub struct InviteResponse {
    pub status: String,
    pub user_id: Option<String>,
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct MemberInfo {
    pub user_id: String,
    pub role: String,
    pub joined_at: Option<String>,
    pub email: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct InviteMemberRequest {
    pub email: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleRequest {
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct InvitationInfo {
    pub invite_id: String,
    pub org_id: String,
    pub org_name: Option<String>,
    pub email: String,
    pub role: String,
    pub invited_by: String,
    pub created_at: String,
    pub expires_at: String,
    pub accepted_at: Option<String>,
}

#[derive(Debug)]
pub enum OrgError {
    NotFound(String),
    NotAdmin,
    NotMember,
    LastAdmin,
    InvitationNotFound,
    EmailMismatch,
    AlreadyMember,
    InvalidRole(String),
    Db(String),
}

impl std::fmt::Display for OrgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "Organization not found: {id}"),
            Self::NotAdmin => write!(f, "Not an admin of this organization"),
            Self::NotMember => write!(f, "Not a member of this organization"),
            Self::LastAdmin => write!(f, "Cannot remove the last admin"),
            Self::InvitationNotFound => write!(f, "Invitation not found or expired"),
            Self::EmailMismatch => write!(f, "Email does not match invitation"),
            Self::AlreadyMember => write!(f, "Already a member of this organization"),
            Self::InvalidRole(r) => write!(f, "Invalid role: {r}"),
            Self::Db(e) => write!(f, "Database error: {e}"),
        }
    }
}

fn db_err(e: rusqlite::Error) -> OrgError {
    OrgError::Db(e.to_string())
}

fn org_error_response(e: OrgError) -> (StatusCode, Json<serde_json::Value>) {
    let status = match &e {
        OrgError::NotAdmin | OrgError::NotMember => StatusCode::FORBIDDEN,
        OrgError::LastAdmin | OrgError::AlreadyMember => StatusCode::CONFLICT,
        OrgError::NotFound(_) | OrgError::InvitationNotFound => StatusCode::NOT_FOUND,
        OrgError::EmailMismatch | OrgError::InvalidRole(_) => StatusCode::BAD_REQUEST,
        OrgError::Db(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(serde_json::json!({"error": e.to_string()})))
}

const VALID_ROLES: &[&str] = &["owner", "admin", "member", "viewer"];

fn validate_role(role: &str) -> Result<(), OrgError> {
    if VALID_ROLES.contains(&role) {
        Ok(())
    } else {
        Err(OrgError::InvalidRole(role.to_owned()))
    }
}

fn check_org_admin(state: &AppState, claims: &Claims, org_id: &str) -> Result<(), OrgError> {
    let conn = state.db.conn().map_err(OrgError::Db)?;
    let role: Option<String> = conn
        .query_row(
            "SELECT role FROM org_members WHERE org_id = ?1 AND user_id = ?2",
            rusqlite::params![org_id, claims.sub],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    match role.as_deref() {
        Some("admin" | "owner") => Ok(()),
        _ => Err(OrgError::NotAdmin),
    }
}

fn check_org_member(state: &AppState, claims: &Claims, org_id: &str) -> Result<(), OrgError> {
    let conn = state.db.conn().map_err(OrgError::Db)?;
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM org_members WHERE org_id = ?1 AND user_id = ?2",
            rusqlite::params![org_id, claims.sub],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if exists {
        Ok(())
    } else {
        Err(OrgError::NotMember)
    }
}

pub fn invite_member(
    state: &AppState,
    claims: &Claims,
    org_id: &str,
    email: &str,
    role: &str,
) -> Result<InviteResponse, OrgError> {
    check_org_admin(state, claims, org_id)?;
    validate_role(role)?;

    let conn = state.db.conn().map_err(OrgError::Db)?;

    let user_id: Option<String> = conn
        .query_row(
            "SELECT user_id FROM accounts WHERE email = ?1",
            rusqlite::params![email],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    if let Some(user_id) = user_id {
        let already: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM org_members WHERE org_id = ?1 AND user_id = ?2",
                rusqlite::params![org_id, user_id],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if already {
            return Err(OrgError::AlreadyMember);
        }

        conn.execute(
            "INSERT OR IGNORE INTO org_members (org_id, user_id, role, invited_by, joined_at) VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            rusqlite::params![org_id, user_id, role, claims.sub],
        )
        .map_err(db_err)?;

        Ok(InviteResponse {
            status: "added".to_owned(),
            user_id: Some(user_id),
            email: email.to_owned(),
        })
    } else {
        let invite_id = format!("inv_{}", uuid::Uuid::new_v4());
        conn.execute(
            "INSERT INTO org_invitations (invite_id, org_id, email, role, invited_by, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, datetime('now', '+7 days'))",
            rusqlite::params![invite_id, org_id, email, role, claims.sub],
        )
        .map_err(db_err)?;

        Ok(InviteResponse {
            status: "invited".to_owned(),
            user_id: None,
            email: email.to_owned(),
        })
    }
}

pub fn remove_member(
    state: &AppState,
    claims: &Claims,
    org_id: &str,
    user_id: &str,
) -> Result<(), OrgError> {
    check_org_admin(state, claims, org_id)?;

    let conn = state.db.conn().map_err(OrgError::Db)?;

    let admin_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM org_members WHERE org_id = ?1 AND role IN ('admin', 'owner')",
            rusqlite::params![org_id],
            |row| row.get(0),
        )
        .map_err(db_err)?;

    let target_role: Option<String> = conn
        .query_row(
            "SELECT role FROM org_members WHERE org_id = ?1 AND user_id = ?2",
            rusqlite::params![org_id, user_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    let is_privileged = matches!(target_role.as_deref(), Some("admin" | "owner"));

    if is_privileged && admin_count <= 1 {
        return Err(OrgError::LastAdmin);
    }

    conn.execute(
        "DELETE FROM org_members WHERE org_id = ?1 AND user_id = ?2",
        rusqlite::params![org_id, user_id],
    )
    .map_err(db_err)?;

    Ok(())
}

pub fn update_member_role(
    state: &AppState,
    claims: &Claims,
    org_id: &str,
    user_id: &str,
    new_role: &str,
) -> Result<(), OrgError> {
    check_org_admin(state, claims, org_id)?;
    validate_role(new_role)?;

    let conn = state.db.conn().map_err(OrgError::Db)?;

    let rows = conn
        .execute(
            "UPDATE org_members SET role = ?1 WHERE org_id = ?2 AND user_id = ?3",
            rusqlite::params![new_role, org_id, user_id],
        )
        .map_err(db_err)?;

    if rows == 0 {
        return Err(OrgError::NotFound(format!(
            "member {user_id} not found in org {org_id}"
        )));
    }

    Ok(())
}

pub fn list_members(
    state: &AppState,
    claims: &Claims,
    org_id: &str,
) -> Result<Vec<MemberInfo>, OrgError> {
    check_org_member(state, claims, org_id)?;

    let conn = state.db.conn().map_err(OrgError::Db)?;
    let mut stmt = conn
        .prepare(
            "SELECT om.user_id, om.role, om.joined_at, a.email, a.display_name
             FROM org_members om
             LEFT JOIN accounts a ON om.user_id = a.user_id
             WHERE om.org_id = ?1
             ORDER BY om.joined_at",
        )
        .map_err(db_err)?;

    let members = stmt
        .query_map(rusqlite::params![org_id], |row| {
            Ok(MemberInfo {
                user_id: row.get(0)?,
                role: row.get(1)?,
                joined_at: row.get(2)?,
                email: row.get(3)?,
                display_name: row.get(4)?,
            })
        })
        .map_err(db_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_err)?;

    Ok(members)
}

pub fn accept_invitation(
    state: &AppState,
    claims: &Claims,
    invite_id: &str,
) -> Result<(), OrgError> {
    let conn = state.db.conn().map_err(OrgError::Db)?;

    let (org_id, role, email): (String, String, String) = conn
        .query_row(
            "SELECT org_id, role, email FROM org_invitations WHERE invite_id = ?1 AND accepted_at IS NULL AND expires_at > datetime('now')",
            rusqlite::params![invite_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| OrgError::InvitationNotFound)?;

    if email != claims.email {
        return Err(OrgError::EmailMismatch);
    }

    conn.execute(
        "INSERT OR IGNORE INTO org_members (org_id, user_id, role, joined_at) VALUES (?1, ?2, ?3, datetime('now'))",
        rusqlite::params![org_id, claims.sub, role],
    )
    .map_err(db_err)?;

    conn.execute(
        "UPDATE org_invitations SET accepted_at = datetime('now') WHERE invite_id = ?1",
        rusqlite::params![invite_id],
    )
    .map_err(db_err)?;

    Ok(())
}

pub fn list_invitations(
    state: &AppState,
    claims: &Claims,
) -> Result<Vec<InvitationInfo>, OrgError> {
    let conn = state.db.conn().map_err(OrgError::Db)?;
    let mut stmt = conn
        .prepare(
            "SELECT i.invite_id, i.org_id, o.display_name, i.email, i.role, i.invited_by, i.created_at, i.expires_at, i.accepted_at
             FROM org_invitations i
             LEFT JOIN orgs o ON i.org_id = o.org_id
             WHERE i.org_id IN (SELECT org_id FROM org_members WHERE user_id = ?1 AND role IN ('admin', 'owner'))
                OR i.email = ?2
             ORDER BY i.created_at DESC",
        )
        .map_err(db_err)?;

    let invitations = stmt
        .query_map(rusqlite::params![claims.sub, claims.email], |row| {
            Ok(InvitationInfo {
                invite_id: row.get(0)?,
                org_id: row.get(1)?,
                org_name: row.get(2)?,
                email: row.get(3)?,
                role: row.get(4)?,
                invited_by: row.get(5)?,
                created_at: row.get(6)?,
                expires_at: row.get(7)?,
                accepted_at: row.get(8)?,
            })
        })
        .map_err(db_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_err)?;

    Ok(invitations)
}

pub async fn invite_member_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
    Json(req): Json<InviteMemberRequest>,
) -> Result<(StatusCode, Json<InviteResponse>), (StatusCode, Json<serde_json::Value>)> {
    invite_member(&state, &claims, &org_id, &req.email, &req.role)
        .map(|r| (StatusCode::OK, Json(r)))
        .map_err(org_error_response)
}

pub async fn remove_member_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, user_id)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    remove_member(&state, &claims, &org_id, &user_id)
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(org_error_response)
}

pub async fn update_member_role_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, user_id)): Path<(String, String)>,
    Json(req): Json<UpdateRoleRequest>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    update_member_role(&state, &claims, &org_id, &user_id, &req.role)
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(org_error_response)
}

pub async fn list_members_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> Result<Json<Vec<MemberInfo>>, (StatusCode, Json<serde_json::Value>)> {
    list_members(&state, &claims, &org_id)
        .map(Json)
        .map_err(org_error_response)
}

pub async fn accept_invitation_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(invite_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    accept_invitation(&state, &claims, &invite_id)
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(org_error_response)
}

pub async fn list_invitations_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<InvitationInfo>>, (StatusCode, Json<serde_json::Value>)> {
    list_invitations(&state, &claims)
        .map(Json)
        .map_err(org_error_response)
}
