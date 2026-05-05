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
use serde::{Deserialize, Serialize};

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
    let plugins = state.plugins.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let list = plugins.list();
    Json(PluginListResponse {
        count: list.len(),
        plugins: list,
    })
}

/// Query parameters for the plugin registry search endpoint.
#[derive(Debug, Deserialize, Serialize)]
pub struct PluginSearchQuery {
    /// Filter by driver name (exact match or prefix).
    pub driver: Option<String>,
    /// Filter by format extension (e.g., "json", "csv", "yaml").
    pub format: Option<String>,
    /// Minimum ABI version (e.g., "1").
    pub min_abi: Option<String>,
    /// Whether the plugin is built-in (not WASM).
    pub builtin: Option<String>,
}

/// Plugin registry search response with metadata.
#[derive(Debug, Serialize)]
pub struct PluginRegistryResponse {
    pub plugins: Vec<PluginRegistryEntry>,
    pub count: usize,
    pub query: PluginSearchQuery,
}

/// An entry in the plugin registry with enriched metadata.
#[derive(Debug, Serialize)]
pub struct PluginRegistryEntry {
    /// Plugin driver name (used as identifier).
    pub driver_name: String,
    /// Human-readable plugin name.
    pub name: String,
    /// List of file extensions this plugin handles.
    pub extensions: Vec<String>,
    /// Whether the plugin is a WASM plugin or built-in.
    pub is_wasm: bool,
}

/// Search the plugin registry with filters.
pub async fn search_plugins(
    State(state): State<AppState>,
    Json(query): Json<PluginSearchQuery>,
) -> Json<PluginRegistryResponse> {
    let plugins = state.plugins.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let list = plugins.list();

    let mut entries: Vec<PluginRegistryEntry> = list
        .into_iter()
        .filter_map(|info| {
            // Filter by driver name.
            if let Some(ref driver) = query.driver
                && !info.driver_name.contains(driver)
                && !info.name.contains(driver)
            {
                return None;
            }

            // Filter by ABI version.
            if let Some(ref _min_abi) = query.min_abi {
                // ABI version filtering not yet supported by PluginInfo.
            }

            Some(PluginRegistryEntry {
                driver_name: info.driver_name.clone(),
                name: info.name.clone(),
                extensions: info.extensions.clone(),
                is_wasm: true,
            })
        })
        .collect();

    // Filter by format extension.
    if let Some(ref format) = query.format {
        let format_lower = format.to_lowercase();
        entries.retain(|e| {
            e.extensions.iter().any(|f| f.to_lowercase() == format_lower)
        });
    }

    let count = entries.len();
    Json(PluginRegistryResponse {
        plugins: entries,
        count,
        query,
    })
}

/// Get detailed information about a specific plugin.
pub async fn get_plugin(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<PluginRegistryEntry>, (StatusCode, Json<serde_json::Value>)> {
    let driver_id = body
        .get("driver")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if driver_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "driver field is required"})),
        ));
    }

    let plugins = state.plugins.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let list = plugins.list();

    let entry = list.into_iter().find(|info| info.driver_name == driver_id);

    match entry {
        Some(info) => Ok(Json(PluginRegistryEntry {
            driver_name: info.driver_name,
            name: info.name,
            extensions: info.extensions,
            is_wasm: true,
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("plugin '{}' not found", driver_id)})),
        )),
    }
}

/// Delete a loaded plugin by driver ID.
pub async fn delete_plugin(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<serde_json::Value>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    if claims.tier != "enterprise" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "plugin management requires enterprise tier"})),
        ));
    }

    let driver_id = body
        .get("driver")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if driver_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "driver field is required"})),
        ));
    }

    let plugins = state.plugins.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if plugins.get(driver_id).is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("plugin '{}' not loaded", driver_id)})),
        ));
    }
    drop(plugins);

    let name = driver_id.replace("plugin-", "");

    // Remove persisted WASM file if it exists.
    let plugin_path = format!("plugins/{name}.wasm");
    tokio::spawn(async move {
        if let Err(e) = tokio::fs::remove_file(&plugin_path).await
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!("Failed to delete plugin file '{}': {}", plugin_path, e);
        }
    });

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "driver": driver_id,
            "status": "removed",
        })),
    ))
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

    let Some(field) = field else {
            return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "no file uploaded"}))));
        };

    let name = field.file_name().map_or_else(|| "unknown".into(), |s| s.replace(['/', '\\', '\0'], "_").replace("..", "_"));
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.') {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "plugin name must contain only alphanumeric characters, hyphens, underscores, and dots"}))));
    }
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

    match state.plugins.lock().unwrap_or_else(std::sync::PoisonError::into_inner).load(&name, &data) {
        Ok(()) => {
            let plugin_path = format!("plugins/{}.wasm", name.replace(".wasm", ""));
            let data_clone = data.clone();
            tokio::spawn(async move {
                if let Err(e) = tokio::task::spawn_blocking(move || {
                    std::fs::create_dir_all("plugins")?;
                    std::fs::write(&plugin_path, &data_clone)
                }).await.unwrap_or_else(|e| Err(std::io::Error::other(e))) {
                    tracing::warn!("Failed to persist plugin: {e}");
                }
            });

            Ok((
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "name": name,
                    "status": "loaded",
                    "driver": state.plugins.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(&format!("plugin-{}", name.replace(".wasm", ""))).map(|p| p.driver_name())
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
    let plugins = state.plugins.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
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
