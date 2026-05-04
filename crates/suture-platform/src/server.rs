// Copyright 2025 Suture Pty Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later OR (AGPL-3.0-or-later WITH Suture-Commercial-1.0)
//
// Licensed under the AGPL-3.0-or-later license OR the
// Suture Commercial License (for enterprise features).
// See LICENSE-AGPL and LICENSE-COMMERCIAL in the repo root.

use axum::{
    extract::{FromRequestParts, State},
    http::request::Parts,
    middleware,
    routing::{delete, get, post, put},
    Extension, Json, Router,
};
use axum::http::StatusCode;
use std::sync::Arc;

use crate::auth::Claims;
use crate::db::PlatformDb;
use crate::Config;
use crate::rate_limit::RateLimiter;
use suture_wasm_plugin::PluginManager;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<PlatformDb>,
    pub config: Arc<Config>,
    pub rate_limiter: Arc<RateLimiter>,
    pub plugins: Arc<std::sync::Mutex<PluginManager>>,
}

impl FromRequestParts<Arc<AppState>> for Claims {
    type Rejection = axum::http::StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Self>()
            .cloned()
            .ok_or(axum::http::StatusCode::UNAUTHORIZED)
    }
}

pub async fn start(config: Config) -> anyhow::Result<()> {
    let db = PlatformDb::new(&config.db_path)?;
    let plugins = Arc::new(std::sync::Mutex::new(PluginManager::new()));

    if let Ok(entries) = std::fs::read_dir("plugins") {
        for entry in entries.flatten() {
            if entry.path().extension().is_some_and(|e| e == "wasm") {
                let path = entry.path().to_string_lossy().to_string();
                match plugins.lock().unwrap().load_file(&path) {
                    Ok(()) => tracing::info!("Loaded plugin: {}", path),
                    Err(e) => tracing::warn!("Failed to load plugin {}: {}", path, e),
                }
            }
        }
    }

    let state = AppState {
        db: Arc::new(db),
        config: Arc::new(config),
        rate_limiter: Arc::new(RateLimiter::new()),
        plugins,
    };

    let public_routes = Router::new()
        .route("/", get(crate::web_ui::serve_index))
        .route("/static/{*path}", get(crate::web_ui::serve_static))
        .route("/health", get(health_check))
        .route("/healthz", get(health_check))
        .route("/auth/register", post(crate::auth::register_handler))
        .route("/auth/login", post(crate::auth::login_handler))
        .route("/auth/oauth/start", get(crate::oauth::start_oauth))
        .route("/auth/google/callback", get(crate::oauth::google_callback))
        .route("/auth/github/callback", get(crate::oauth::github_callback))
        .route("/api/drivers", get(crate::merge_api::list_drivers))
        .route("/billing/webhook", post(crate::stripe::handle_webhook));

    let protected_routes = Router::new()
        .route("/auth/me", get(crate::auth::me_handler))
        .route("/auth/logout", post(crate::auth::logout_handler))
        .route("/api/merge", post(crate::merge_api::merge_files))
        .route("/api/usage", get(crate::billing::usage_handler))
        .route("/api/analytics", get(crate::analytics::analytics_handler))
        .route("/api/orgs", post(crate::orgs::create_org).get(crate::orgs::list_my_orgs))
        .route("/api/orgs/{org_id}/invite", post(crate::orgs::invite_member_handler))
        .route("/api/orgs/{org_id}/members", get(crate::orgs::list_members_handler))
        .route("/api/orgs/{org_id}/members/{user_id}", delete(crate::orgs::remove_member_handler))
        .route("/api/orgs/{org_id}/members/{user_id}/role", put(crate::orgs::update_member_role_handler))
        .route("/api/invitations", get(crate::orgs::list_invitations_handler))
        .route("/api/invitations/{invite_id}/accept", post(crate::orgs::accept_invitation_handler))
        .route("/api/plugins", get(crate::plugins::list_plugins))
        .route("/api/plugins/upload", post(crate::plugins::upload_plugin))
        .route("/api/plugins/merge", post(crate::plugins::merge_with_plugin))
        .route("/billing/checkout", post(crate::stripe::create_checkout_session))
        .route("/billing/subscription", get(crate::stripe::get_subscription))
        .route("/billing/portal", post(portal_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::require_auth,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::rate_limit::rate_limit,
        ));

    let auth_routes = Router::new()
        .route(
            "/admin/users",
            get(crate::auth::list_users_handler),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::require_auth,
        ));

    let addr = state.config.addr.clone();
    let cors_origins: Vec<String> = state.config.cors_origins.clone();
    let app = public_routes
        .merge(protected_routes)
        .merge(auth_routes)
        .with_state(state)
        .layer(crate::middleware::cors_layer(
            &cors_origins.iter().map(String::as_str).collect::<Vec<_>>(),
        ))
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Platform listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn health_check() -> &'static str {
    "ok"
}

async fn shutdown_signal() {
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            tracing::info!("shutdown signal received");
        }
        Err(e) => {
            tracing::error!("failed to install ctrl-c handler: {e}");
        }
    }
}

async fn portal_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<crate::stripe::PortalResponse>, (StatusCode, Json<serde_json::Value>)> {
    crate::stripe::create_portal_session_inner(&state, &claims).await
}
