//! OpenTelemetry integration for Suture Hub.
//!
//! Provides a [`init_telemetry`] function that configures tracing with an
//! optional OpenTelemetry export layer controlled via the `SUTURE_OTEL_ENABLED`
//! environment variable.
//!
//! The telemetry span middleware is always available (uses only tracing spans).

use axum::{body::Body, extract::State, http::Request, middleware::Next, response::Response};
use std::sync::Arc;

use crate::server::SutureHubServer;

/// Guard that shuts down the OpenTelemetry tracer provider on drop.
/// When the `otel` feature is not enabled, this is a no-op.
pub struct TelemetryGuard {
    _private: (),
}

#[cfg(feature = "otel")]
impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        opentelemetry::global::shutdown_tracer_provider();
    }
}

/// Initialize the tracing subscriber with optional OpenTelemetry export.
///
/// Controlled by environment variables:
/// - `SUTURE_OTEL_ENABLED`: set to `"true"` to enable OTel export (default: `"false"`)
/// - `OTEL_EXPORTER_OTLP_ENDPOINT`: gRPC endpoint, e.g. `"http://localhost:4317"` (default)
/// - `OTEL_SERVICE_NAME`: service name sent to the collector (default: `"suture-hub"`)
///
/// Returns a [`TelemetryGuard`] that must be held for the duration of the process
/// to ensure a clean OTel shutdown.
#[cfg(feature = "otel")]
pub fn init_telemetry() -> Result<TelemetryGuard, Box<dyn std::error::Error + Send + Sync>> {
    let otel_enabled =
        std::env::var("SUTURE_OTEL_ENABLED").unwrap_or_else(|_| "false".to_string()) == "true";

    if otel_enabled {
        let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:4317".to_string());
        let service_name =
            std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "suture-hub".to_string());

        // opentelemetry-otlp tonic transport expects bare host:port
        let grpc_endpoint = endpoint
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .to_string();

        let exporter = opentelemetry_otlp::NewExporter::builder()
            .with_tonic()
            .with_endpoint(&grpc_endpoint)
            .build()?;

        let tracer = opentelemetry_sdk::trace::TracerProvider::builder()
            .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
            .with_resource(
                opentelemetry_sdk::resource::Resource::builder_empty()
                    .with_attribute(opentelemetry::KeyValue::new(
                        opentelemetry_semantic_conventions::resource::SERVICE_NAME,
                        service_name,
                    ))
                    .build(),
            )
            .build()
            .tracer("suture-hub");

        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        let filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

        let fmt_layer = tracing_subscriber::fmt::layer().with_target(false);

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .with(otel_layer)
            .init();

        Ok(TelemetryGuard { _private: () })
    } else {
        init_telemetry_fallback()
    }
}

/// Fallback: initialize without OTel (used when feature is disabled or OTel is off).
pub fn init_telemetry() -> Result<TelemetryGuard, Box<dyn std::error::Error + Send + Sync>> {
    init_telemetry_fallback()
}

fn init_telemetry_fallback() -> Result<TelemetryGuard, Box<dyn std::error::Error + Send + Sync>> {
    use tracing_subscriber::EnvFilter;
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("suture_hub=info".parse().unwrap()),
        )
        .with_target(false)
        .init();
    Ok(TelemetryGuard { _private: () })
}

/// Axum middleware that creates a tracing span for each HTTP request.
///
/// The span captures method, path, and response status code. When the `otel`
/// feature is enabled, these spans are exported to the configured OTel collector.
pub async fn telemetry_middleware(
    State(_hub): State<Arc<SutureHubServer>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let method = req.method().to_string();
    let path = req.uri().path().to_owned();

    let span = tracing::info_span!(
        "http_request",
        method = %method,
        path = %path,
        status = tracing::field::Empty,
    );

    let response = next.run(req).await;

    let status = response.status().as_u16();
    span.record("status", status);

    response
}
