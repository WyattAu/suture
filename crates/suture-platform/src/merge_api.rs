use axum::{
    extract::{Extension, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::auth::Claims;
use crate::billing;
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct MergeRequest {
    pub driver: String,
    pub base: String,
    pub ours: String,
    pub theirs: String,
}

#[derive(Debug, Serialize)]
pub struct MergeResponse {
    pub result: Option<String>,
    pub driver: String,
    pub conflicts: bool,
}

#[derive(Debug, Serialize)]
pub struct SupportedDriversResponse {
    pub drivers: Vec<DriverInfo>,
}

#[derive(Debug, Serialize)]
pub struct DriverInfo {
    pub name: String,
    pub extensions: Vec<String>,
}

pub async fn list_drivers(State(_state): State<AppState>) -> Json<SupportedDriversResponse> {
    let drivers = vec![
        DriverInfo {
            name: "JSON".into(),
            extensions: vec![".json".into()],
        },
        DriverInfo {
            name: "YAML".into(),
            extensions: vec![".yaml".into(), ".yml".into()],
        },
        DriverInfo {
            name: "TOML".into(),
            extensions: vec![".toml".into()],
        },
        DriverInfo {
            name: "XML".into(),
            extensions: vec![".xml".into()],
        },
        DriverInfo {
            name: "CSV".into(),
            extensions: vec![".csv".into()],
        },
        DriverInfo {
            name: "SQL".into(),
            extensions: vec![".sql".into()],
        },
        DriverInfo {
            name: "Properties".into(),
            extensions: vec![".properties".into()],
        },
        DriverInfo {
            name: "INI".into(),
            extensions: vec![".ini".into(), ".cfg".into()],
        },
        DriverInfo {
            name: "HTML".into(),
            extensions: vec![".html".into(), ".htm".into()],
        },
        DriverInfo {
            name: "Markdown".into(),
            extensions: vec![".md".into()],
        },
    ];
    Json(SupportedDriversResponse { drivers })
}

pub async fn merge_files(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<MergeRequest>,
) -> Result<Json<MergeResponse>, (StatusCode, Json<serde_json::Value>)> {
    if let Err(e) = billing::record_merge(&state.db, &claims.sub) {
        tracing::warn!("failed to record merge usage: {}", e);
    }

    match billing::can_merge(&state.db, &claims.sub) {
        Ok(true) => {}
        Ok(false) => {
            return Err((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "merge limit reached for this billing period",
                    "tier": claims.tier,
                    "upgrade_url": "/billing"
                })),
            ));
        }
        Err(e) => {
            tracing::warn!("failed to check merge limit: {}", e);
        }
    }

    let result = match req.driver.to_lowercase().as_str() {
        "json" => merge_with::<suture_driver_json::JsonDriver>(&req),
        "yaml" | "yml" => merge_with::<suture_driver_yaml::YamlDriver>(&req),
        "toml" => merge_with::<suture_driver_toml::TomlDriver>(&req),
        "xml" => merge_with::<suture_driver_xml::XmlDriver>(&req),
        "csv" => merge_with::<suture_driver_csv::CsvDriver>(&req),
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("unsupported driver: {}", req.driver)})),
            ));
        }
    };

    match result {
        Ok(merged) => {
            let conflicts = merged.is_none();
            Ok(Json(MergeResponse {
                result: merged,
                driver: req.driver,
                conflicts,
            }))
        }
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}

fn merge_with<D: suture_driver::SutureDriver + Default>(
    req: &MergeRequest,
) -> Result<Option<String>, String> {
    let driver = D::default();
    driver
        .merge(&req.base, &req.ours, &req.theirs)
        .map_err(|e| e.to_string())
}
