// Copyright 2025 Suture Pty Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later OR (AGPL-3.0-or-later WITH Suture-Commercial-1.0)
//
// Licensed under the AGPL-3.0-or-later license OR the
// Suture Commercial License (for enterprise features).
// See LICENSE-AGPL and LICENSE-COMMERCIAL in the repo root.

use axum::{
    extract::{Extension, Multipart, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;

use crate::auth::Claims;
use crate::server::AppState;

#[derive(Debug, Serialize)]
pub struct PluginListResponse {
    pub plugins: Vec<suture_wasm_plugin::PluginInfo>,
    pub count: usize,
}

pub async fn list_plugins(
    State(state): State<AppState>,
) -> Json<PluginListResponse> {
    let plugins = state.plugins.lock().unwrap();
    let list = plugins.list();
    Json(PluginListResponse {
        count: list.len(),
        plugins: list,
    })
}

pub async fn upload_plugin(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    if claims.tier != "enterprise" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "plugin uploads require enterprise tier"})),
        ));
    }

    let field = multipart.next_field().await.map_err(|e| {
        (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()})))
    })?;

    let field = match field {
        Some(f) => f,
        None => {
            return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "no file uploaded"}))));
        }
    };

    let name = field.file_name().unwrap_or("unknown").to_string();
    let data = field.bytes().await.map_err(|e| {
        (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()})))
    })?;

    match suture_wasm_plugin::validate_plugin(&data) {
        Ok(warnings) => {
            if warnings.is_empty() {
                tracing::info!("Plugin '{}' validated successfully", name);
            } else {
                tracing::warn!("Plugin '{}' warnings: {:?}", name, warnings);
            }
        }
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("invalid Wasm module: {}", e)})),
            ));
        }
    }

    match state.plugins.lock().unwrap().load(&name, &data) {
        Ok(_) => {
            if let Err(e) = std::fs::create_dir_all("plugins") {
                tracing::warn!("Failed to create plugins directory: {}", e);
            }
            let plugin_path = format!("plugins/{}.wasm", name.replace(".wasm", ""));
            if let Err(e) = std::fs::write(&plugin_path, &data) {
                tracing::warn!("Failed to persist plugin: {}", e);
            }

            Ok((
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "name": name,
                    "status": "loaded",
                    "driver": state.plugins.lock().unwrap().get(&format!("plugin-{}", name.replace(".wasm", ""))).map(|p| p.driver_name())
                })),
            ))
        }
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("failed to load plugin: {}", e)})),
        )),
    }
}

pub async fn merge_with_plugin(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(req): Json<crate::merge_api::MergeRequest>,
) -> Result<Json<crate::merge_api::MergeResponse>, (StatusCode, Json<serde_json::Value>)> {
    let plugins = state.plugins.lock().unwrap();
    let plugin = plugins.get(&req.driver).ok_or_else(|| {
        (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": format!("plugin '{}' not loaded", req.driver)})))
    })?;

    match plugin.merge(&req.base, &req.ours, &req.theirs) {
        Ok(result) => Ok(Json(crate::merge_api::MergeResponse {
            result: result.merged,
            driver: req.driver,
            conflicts: result.conflicts,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}
