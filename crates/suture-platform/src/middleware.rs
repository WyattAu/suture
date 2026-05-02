// Copyright 2025 Suture Pty Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later OR (AGPL-3.0-or-later WITH Suture-Commercial-1.0)
//
// Licensed under the AGPL-3.0-or-later license OR the
// Suture Commercial License (for enterprise features).
// See LICENSE-AGPL and LICENSE-COMMERCIAL in the repo root.

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::server::AppState;

pub async fn require_auth(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "missing or invalid authorization header"})),
            )
                .into_response();
        }
    };

    match crate::auth::verify_jwt(token, &state.config.jwt_secret) {
        Ok(claims) => {
            if crate::auth::is_token_revoked(&state.db, &claims.sub) {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": "token has been revoked"})),
                )
                    .into_response();
            }
            request.extensions_mut().insert(claims);
            next.run(request).await
        }
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "invalid or expired token"})),
        )
            .into_response(),
    }
}

pub async fn optional_auth(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    if let Some(token) = request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        && let Ok(claims) = crate::auth::verify_jwt(token, &state.config.jwt_secret)
        && !crate::auth::is_token_revoked(&state.db, &claims.sub)
    {
        request.extensions_mut().insert(claims);
    }
    next.run(request).await
}

pub fn cors_layer(allowed_origins: &[&str]) -> tower_http::cors::CorsLayer {
    use tower_http::cors::AllowOrigin;

    if allowed_origins.is_empty() || allowed_origins.contains(&"*") {
        tower_http::cors::CorsLayer::permissive()
    } else {
        tower_http::cors::CorsLayer::new()
            .allow_origin(
                AllowOrigin::list(
                    allowed_origins
                        .iter()
                        .filter_map(|o| o.parse().ok()),
                ),
            )
            .allow_methods(tower_http::cors::Any)
            .allow_headers(tower_http::cors::Any)
    }
}
