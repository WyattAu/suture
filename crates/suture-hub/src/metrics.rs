use axum::{
    body::Body,
    extract::State,
    http::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::server::SutureHubServer;

type RequestKey = (String, String, String);
type RequestCounts = HashMap<RequestKey, u64>;

const MAX_DURATION_SAMPLES: usize = 4096;

#[derive(Clone, Default)]
pub struct HubMetrics {
    request_counts: Arc<Mutex<RequestCounts>>,
    request_duration_ms: Arc<Mutex<Vec<u64>>>,
}

impl HubMetrics {
    pub fn new() -> Self {
        Self {
            request_counts: Arc::new(Mutex::new(HashMap::new())),
            request_duration_ms: Arc::new(Mutex::new(Vec::with_capacity(MAX_DURATION_SAMPLES))),
        }
    }

    pub fn record_request(&self, method: &str, path: &str, status: u16) {
        let normalized = normalize_path(path);
        let key = (method.to_string(), normalized, status.to_string());
        let mut counts = self
            .request_counts
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *counts.entry(key).or_insert(0) += 1;
    }

    pub fn record_duration(&self, elapsed_ms: u64) {
        let mut durations = self
            .request_duration_ms
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if durations.len() >= MAX_DURATION_SAMPLES {
            // Drop the oldest half to make room
            let drop_count = MAX_DURATION_SAMPLES / 2;
            durations.drain(..drop_count);
        }
        durations.push(elapsed_ms);
    }

    pub fn snapshot_request_counts(&self) -> Vec<(String, String, String, u64)> {
        let counts = self
            .request_counts
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        counts
            .iter()
            .map(|((method, path, status), count)| {
                (method.clone(), path.clone(), status.clone(), *count)
            })
            .collect()
    }

    pub fn snapshot_durations_ms(&self) -> Vec<u64> {
        self.request_duration_ms
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

pub async fn metrics_handler(State(hub): State<Arc<SutureHubServer>>) -> impl IntoResponse {
    let mut lines: Vec<String> = Vec::new();

    let store = hub.storage.read().await;

    let repo_count = store.repo_count().unwrap_or(0);
    lines.push("# HELP suture_repos_total Total number of repositories".to_string());
    lines.push("# TYPE suture_repos_total gauge".to_string());
    lines.push(format!("suture_repos_total {repo_count}"));
    lines.push(String::new());

    let patch_count = store.total_patch_count().unwrap_or(0);
    lines.push("# HELP suture_patches_total Total number of patches across all repos".to_string());
    lines.push("# TYPE suture_patches_total gauge".to_string());
    lines.push(format!("suture_patches_total {patch_count}"));
    lines.push(String::new());

    let (blob_count, blob_size) = store.total_blob_stats().unwrap_or((0, 0));
    lines.push("# HELP suture_blobs_total Total number of blobs stored".to_string());
    lines.push("# TYPE suture_blobs_total gauge".to_string());
    lines.push(format!("suture_blobs_total {blob_count}"));
    lines.push(String::new());

    lines.push("# HELP suture_blobs_size_bytes Total size of all blobs in bytes".to_string());
    lines.push("# TYPE suture_blobs_size_bytes gauge".to_string());
    lines.push(format!("suture_blobs_size_bytes {blob_size}"));
    lines.push(String::new());

    let user_count = store.user_count().unwrap_or(0);
    lines.push("# HELP suture_active_users_total Total number of registered users".to_string());
    lines.push("# TYPE suture_active_users_total gauge".to_string());
    lines.push(format!("suture_active_users_total {user_count}"));
    lines.push(String::new());

    drop(store);

    let request_counts = hub.request_metrics.snapshot_request_counts();
    lines.push("# HELP suture_requests_total Total HTTP requests served".to_string());
    lines.push("# TYPE suture_requests_total counter".to_string());
    for (method, path, status, count) in &request_counts {
        lines.push(format!(
            "suture_requests_total{{method=\"{method}\",path=\"{path}\",status=\"{status}\"}} {count}"
        ));
    }
    if request_counts.is_empty() {
        lines.push("suture_requests_total{method=\"\",path=\"\",status=\"\"} 0".to_string());
    }
    lines.push(String::new());

    let durations = hub.request_metrics.snapshot_durations_ms();
    let total_count = durations.len() as u64;
    let total_sum_ms: u64 = durations.iter().sum();
    let total_sum_secs = total_sum_ms as f64 / 1000.0;

    // Bucket boundaries in milliseconds: 10ms, 100ms, 1s, +Inf
    let bucket_boundaries_ms: &[(f64, u64)] = &[
        (0.01, 10),  // 10ms
        (0.1, 100),  // 100ms
        (1.0, 1000), // 1s
    ];

    let mut cumulative: u64 = 0;
    lines.push("# HELP suture_request_duration_seconds HTTP request duration".to_string());
    lines.push("# TYPE suture_request_duration_seconds histogram".to_string());
    for (le_secs, le_ms) in bucket_boundaries_ms {
        cumulative += durations.iter().filter(|&&d| d <= *le_ms).count() as u64;
        lines.push(format!(
            "suture_request_duration_seconds_bucket{{le=\"{le_secs}\"}} {cumulative}"
        ));
    }
    // +Inf bucket includes everything
    lines.push(format!(
        "suture_request_duration_seconds_bucket{{le=\"+Inf\"}} {total_count}"
    ));
    lines.push(format!(
        "suture_request_duration_seconds_sum {total_sum_secs}"
    ));
    lines.push(format!(
        "suture_request_duration_seconds_count {total_count}"
    ));

    let body = lines.join("\n");
    (
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
}

pub async fn metrics_middleware(
    State(hub): State<Arc<SutureHubServer>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_owned();

    let start = std::time::Instant::now();
    let response = next.run(req).await;
    let elapsed_ms = start.elapsed().as_millis() as u64;

    let status = response.status().as_u16();
    hub.request_metrics
        .record_request(method.as_str(), &path, status);
    hub.request_metrics.record_duration(elapsed_ms);

    response
}

fn normalize_path(path: &str) -> String {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let known: &[&str] = &[
        "healthz",
        "metrics",
        "push",
        "pull",
        "repos",
        "repo",
        "handshake",
        "v2",
        "auth",
        "token",
        "verify",
        "login",
        "register",
        "mirror",
        "setup",
        "sync",
        "status",
        "branches",
        "patches",
        "tree",
        "protect",
        "unprotect",
        "blobs",
        "users",
        "search",
        "activity",
        "mirrors",
        "webhooks",
        "replication",
        "peers",
        "lfs",
        "batch",
        "objects",
        "sso",
        "providers",
        "authorize",
        "callback",
        "raft",
        "audit",
        "log",
        "static",
        "compressed",
    ];
    let mut out = Vec::with_capacity(segments.len());
    for seg in &segments {
        if known.contains(seg) {
            out.push(*seg);
        } else {
            out.push(":id");
        }
    }
    format!("/{}", out.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_static_paths() {
        assert_eq!(normalize_path("/healthz"), "/healthz");
        assert_eq!(normalize_path("/repos"), "/repos");
        assert_eq!(normalize_path("/push"), "/push");
        assert_eq!(normalize_path("/v2/handshake"), "/v2/handshake");
    }

    #[test]
    fn test_normalize_repo_id_paths() {
        assert_eq!(normalize_path("/repo/my-repo-123"), "/repo/:id");
        assert_eq!(
            normalize_path("/repos/my-repo/branches"),
            "/repos/:id/branches"
        );
        assert_eq!(
            normalize_path("/repos/my-repo/blobs/abc123def456"),
            "/repos/:id/blobs/:id"
        );
    }

    #[test]
    fn test_normalize_hex_paths() {
        assert_eq!(
            normalize_path("/repos/test/blobs/cafebabe1234"),
            "/repos/:id/blobs/:id"
        );
    }

    #[test]
    fn test_record_and_snapshot() {
        let m = HubMetrics::new();
        m.record_request("GET", "/repos", 200);
        m.record_request("GET", "/repos", 200);
        m.record_request("POST", "/push", 200);

        let snapshot = m.snapshot_request_counts();
        assert_eq!(snapshot.len(), 2);

        let get_repos = snapshot
            .iter()
            .find(|(m, p, s, _)| m == "GET" && p == "/repos" && s == "200")
            .unwrap();
        assert_eq!(get_repos.3, 2);

        let post_push = snapshot
            .iter()
            .find(|(m, p, s, _)| m == "POST" && p == "/push" && s == "200")
            .unwrap();
        assert_eq!(post_push.3, 1);
    }

    #[test]
    fn test_metrics_output_format() {
        let m = HubMetrics::new();
        m.record_request("GET", "/repos", 200);

        let snapshot = m.snapshot_request_counts();
        assert!(!snapshot.is_empty());
    }

    #[test]
    fn test_normalize_sso_paths() {
        assert_eq!(
            normalize_path("/sso/providers/google"),
            "/sso/providers/:id"
        );
        assert_eq!(normalize_path("/sso/authorize"), "/sso/authorize");
    }

    #[test]
    fn test_normalize_lfs_paths() {
        assert_eq!(
            normalize_path("/lfs/objects/myrepo/cafebabe1234"),
            "/lfs/objects/:id/:id"
        );
    }
}
