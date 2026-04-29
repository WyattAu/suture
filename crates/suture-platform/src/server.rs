use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    middleware,
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use crate::auth::Claims;
use crate::db::PlatformDb;
use crate::Config;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<PlatformDb>,
    pub config: Arc<Config>,
}

impl FromRequestParts<Arc<AppState>> for Claims {
    type Rejection = axum::http::StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Claims>()
            .cloned()
            .ok_or(axum::http::StatusCode::UNAUTHORIZED)
    }
}

pub async fn start(config: Config) -> anyhow::Result<()> {
    let db = PlatformDb::new(&config.db_path)?;
    let state = AppState {
        db: Arc::new(db),
        config: Arc::new(config),
    };

    let public_routes = Router::new()
        .route("/", get(crate::web_ui::serve_index))
        .route("/static/{*path}", get(crate::web_ui::serve_static))
        .route("/healthz", get(health_check))
        .route("/auth/register", post(crate::auth::register_handler))
        .route("/auth/login", post(crate::auth::login_handler))
        .route("/api/drivers", get(crate::merge_api::list_drivers));

    let protected_routes = Router::new()
        .route("/auth/me", get(crate::auth::me_handler))
        .route("/api/merge", post(crate::merge_api::merge_files))
        .route("/api/usage", get(crate::billing::usage_handler))
        .route("/api/orgs", post(crate::orgs::create_org).get(crate::orgs::list_my_orgs))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::optional_auth,
        ));

    let admin_routes = Router::new()
        .route(
            "/admin/users",
            get(crate::auth::list_users_handler),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::require_auth,
        ));

    let addr = state.config.addr.clone();
    let app = public_routes
        .merge(protected_routes)
        .merge(admin_routes)
        .with_state(state)
        .layer(crate::middleware::cors_layer())
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
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install ctrl-c handler");
    tracing::info!("shutdown signal received");
}
