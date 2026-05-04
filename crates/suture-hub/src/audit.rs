//! Audit logging middleware for Suture Hub.
//!
//! Logs all mutating (POST, PUT, DELETE, PATCH) requests to the audit_log table.

use axum::{
    body::Body,
    extract::State,
    http::Request,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::server::SutureHubServer;

/// Axum middleware that logs mutating requests to the audit log.
pub async fn audit_middleware(
    State(hub): State<Arc<SutureHubServer>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let client_ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(',')
        .next()
        .unwrap_or("")
        .trim()
        .to_owned();

    // Run the actual handler
    let response = next.run(req).await;

    // Only audit mutating methods
    let is_mutating = matches!(
        method,
        axum::http::Method::POST
            | axum::http::Method::PUT
            | axum::http::Method::DELETE
            | axum::http::Method::PATCH
    );

    if is_mutating {
        let status = response.status();
        let status_str = if status.is_success() { "success" } else { "failure" };

        let action = format!("{} {}", method, uri.path());
        let (resource_type, resource_id) = classify_resource(uri.path());
        let details = format!("status={status}");

        let store = hub.storage.read().await;
        // Ignore audit write failures — don't break the request
        let _ = store.write_audit_entry(
            "",
            &action,
            &resource_type,
            &resource_id,
            status_str,
            &details,
            &request_id,
            &client_ip,
        );
    }

    response
}

/// Classify the resource type and ID from a URL path.
fn classify_resource(path: &str) -> (String, String) {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    match segments.as_slice() {
        // /push, /pull
        ["push" | "pull"] => ("repo".to_owned(), String::new()),
        // /v2/push, /v2/pull
        ["v2", "push" | "pull"] => ("repo".to_owned(), String::new()),
        // /repos/{repo_id}/...
        ["repos", repo_id, ..] => ("repo".to_owned(), (*repo_id).to_owned()),
        // /auth/token, /auth/login, /auth/register
        ["auth", "token"] => ("auth".to_owned(), "token".to_owned()),
        ["auth", "login"] => ("auth".to_owned(), "login".to_owned()),
        ["auth", "register"] => ("auth".to_owned(), "register".to_owned()),
        // /users/{username}
        ["users", username] => ("user".to_owned(), (*username).to_owned()),
        // /mirrors/{id}
        ["mirrors", id] => ("mirror".to_owned(), (*id).to_owned()),
        // /mirror/setup, /mirror/sync
        ["mirror", "setup" | "sync" | "status"] => ("mirror".to_owned(), String::new()),
        // /replication/peers
        ["replication", "peers" | "sync"] => ("replication".to_owned(), String::new()),
        // /webhooks/{repo_id}/{id}
        ["webhooks", repo_id, id] => ("webhook".to_owned(), format!("{repo_id}/{id}")),
        // /webhooks/{repo_id}
        ["webhooks", repo_id] => ("webhook".to_owned(), (*repo_id).to_owned()),
        // /lfs/batch
        ["lfs", "batch"] => ("lfs".to_owned(), String::new()),
        // /lfs/objects/{repo_id}/{oid}
        ["lfs", "objects", repo_id, oid] => ("lfs".to_owned(), format!("{repo_id}/{oid}")),
        // /sso/providers/{name}
        ["sso", "providers", name] => ("sso_provider".to_owned(), (*name).to_owned()),
        // /sso/providers, /sso/authorize, /sso/callback
        ["sso", ..] => ("sso".to_owned(), String::new()),
        // Default
        _ => ("unknown".to_owned(), String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_resource_repos() {
        let (rt, ri) = classify_resource("/repos/my-repo/branches");
        assert_eq!(rt, "repo");
        assert_eq!(ri, "my-repo");
    }

    #[test]
    fn test_classify_resource_push() {
        let (rt, ri) = classify_resource("/push");
        assert_eq!(rt, "repo");
        assert_eq!(ri, "");
    }

    #[test]
    fn test_classify_resource_auth() {
        let (rt, ri) = classify_resource("/auth/token");
        assert_eq!(rt, "auth");
        assert_eq!(ri, "token");
    }

    #[test]
    fn test_classify_resource_sso() {
        let (rt, ri) = classify_resource("/sso/providers/google");
        assert_eq!(rt, "sso_provider");
        assert_eq!(ri, "google");
    }

    #[test]
    fn test_classify_resource_lfs() {
        let (rt, ri) = classify_resource("/lfs/objects/repo1/abc123");
        assert_eq!(rt, "lfs");
        assert_eq!(ri, "repo1/abc123");
    }
}
