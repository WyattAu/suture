use axum::{
    extract::{Extension, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::auth::Claims;
use crate::billing;
use crate::server::AppState;

use suture_driver_csv::CsvDriver;
use suture_driver_docx::DocxDriver;
use suture_driver_example::PropertiesDriver;
use suture_driver_feed::FeedDriver;
use suture_driver_html::HtmlDriver;
use suture_driver_ical::IcalDriver;
use suture_driver_image::ImageDriver;
use suture_driver_json::JsonDriver;
use suture_driver_markdown::MarkdownDriver;
use suture_driver_otio::OtioDriver;
use suture_driver_pdf::PdfDriver;
use suture_driver_pptx::PptxDriver;
use suture_driver_sql::SqlDriver;
use suture_driver_svg::SvgDriver;
use suture_driver_toml::TomlDriver;
use suture_driver_xlsx::XlsxDriver;
use suture_driver_xml::XmlDriver;
use suture_driver_yaml::YamlDriver;

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
            name: "HTML".into(),
            extensions: vec![".html".into(), ".htm".into()],
        },
        DriverInfo {
            name: "Markdown".into(),
            extensions: vec![".md".into(), ".markdown".into(), ".mdown".into(), ".mkd".into()],
        },
        DriverInfo {
            name: "SVG".into(),
            extensions: vec![".svg".into()],
        },
        DriverInfo {
            name: "DOCX".into(),
            extensions: vec![".docx".into()],
        },
        DriverInfo {
            name: "FEED".into(),
            extensions: vec![".rss".into(), ".atom".into()],
        },
        DriverInfo {
            name: "ICAL".into(),
            extensions: vec![".ics".into(), ".ifb".into()],
        },
        DriverInfo {
            name: "Image".into(),
            extensions: vec![".png".into(), ".jpg".into(), ".jpeg".into(), ".gif".into(), ".bmp".into(), ".webp".into(), ".tiff".into(), ".tif".into(), ".ico".into(), ".avif".into()],
        },
        DriverInfo {
            name: "OpenTimelineIO".into(),
            extensions: vec![".otio".into()],
        },
        DriverInfo {
            name: "PDF".into(),
            extensions: vec![".pdf".into()],
        },
        DriverInfo {
            name: "PPTX".into(),
            extensions: vec![".pptx".into()],
        },
        DriverInfo {
            name: "XLSX".into(),
            extensions: vec![".xlsx".into()],
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
        "json" => merge_with::<JsonDriver>(&req),
        "yaml" | "yml" => merge_with::<YamlDriver>(&req),
        "toml" => merge_with::<TomlDriver>(&req),
        "xml" => merge_with::<XmlDriver>(&req),
        "csv" => merge_with::<CsvDriver>(&req),
        "sql" => merge_with::<SqlDriver>(&req),
        "properties" => merge_with::<PropertiesDriver>(&req),
        "html" | "htm" => merge_with::<HtmlDriver>(&req),
        "markdown" | "md" | "mdown" | "mkd" => merge_with::<MarkdownDriver>(&req),
        "svg" => merge_with::<SvgDriver>(&req),
        "docx" => merge_with::<DocxDriver>(&req),
        "feed" | "rss" | "atom" => merge_with::<FeedDriver>(&req),
        "ical" | "ics" | "ifb" => merge_with::<IcalDriver>(&req),
        "image" | "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tiff" | "tif" | "ico" | "avif" => {
            merge_with::<ImageDriver>(&req)
        }
        "otio" => merge_with::<OtioDriver>(&req),
        "pdf" => merge_with::<PdfDriver>(&req),
        "pptx" => merge_with::<PptxDriver>(&req),
        "xlsx" => merge_with::<XlsxDriver>(&req),
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
