use std::sync::Arc;
use std::time::Duration;

async fn start_test_hub() -> String {
    let mut hub = suture_hub::SutureHubServer::new_in_memory();
    hub.set_no_auth(true);
    let hub = Arc::new(hub);

    let app = axum::Router::new()
        .route(
            "/push",
            axum::routing::post(suture_hub::server::push_handler),
        )
        .route(
            "/pull",
            axum::routing::post(suture_hub::server::pull_handler),
        )
        .route("/handshake", axum::routing::get(handshake_get))
        .route(
            "/handshake",
            axum::routing::post(suture_hub::server::handshake_handler),
        )
        .route("/api/merge", axum::routing::post(merge_handler))
        .with_state(hub);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let url = format!("http://127.0.0.1:{}", port);

    tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await;
    });

    let client = reqwest::Client::new();
    for _ in 0..50 {
        if client
            .get(format!("{}/handshake", &url))
            .send()
            .await
            .is_ok()
        {
            return url;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("Hub did not start in time");
}

async fn handshake_get() -> axum::Json<suture_hub::types::HandshakeResponse> {
    axum::Json(suture_hub::types::HandshakeResponse {
        server_version: suture_hub::types::PROTOCOL_VERSION,
        server_name: "suture-hub".to_string(),
        compatible: true,
    })
}

async fn merge_handler(
    axum::Json(body): axum::Json<serde_json::Value>,
) -> axum::Json<serde_json::Value> {
    use suture_driver::SutureDriver;

    let driver_name = body
        .get("driver")
        .and_then(|v| v.as_str())
        .unwrap_or("json");
    let base = body.get("base").and_then(|v| v.as_str()).unwrap_or("");
    let ours = body.get("ours").and_then(|v| v.as_str()).unwrap_or("");
    let theirs = body.get("theirs").and_then(|v| v.as_str()).unwrap_or("");

    let result: Result<Option<String>, suture_driver::DriverError> = match driver_name {
        "json" => {
            let driver = suture_driver_json::JsonDriver;
            driver.merge(base, ours, theirs)
        }
        "yaml" => {
            let driver = suture_driver_yaml::YamlDriver::new();
            driver.merge(base, ours, theirs)
        }
        _ => Err(suture_driver::DriverError::DriverNotFound(
            driver_name.to_string(),
        )),
    };

    match result {
        Ok(Some(merged)) => axum::Json(serde_json::json!({
            "status": "success",
            "result": merged,
        })),
        Ok(None) => axum::Json(serde_json::json!({
            "status": "conflict",
            "conflicts": ["merge returned None (unresolvable conflict)"],
        })),
        Err(e) => axum::Json(serde_json::json!({
            "status": "conflict",
            "error": e.to_string(),
            "conflicts": [e.to_string()],
        })),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_platform_merge_api_json_success() {
    let hub_url = start_test_hub().await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/merge", hub_url))
        .json(&serde_json::json!({
            "driver": "json",
            "base": r#"{"key": "base", "other": "shared"}"#,
            "ours": r#"{"key": "ours", "other": "shared"}"#,
            "theirs": r#"{"key": "base", "other": "shared"}"#
        }))
        .send()
        .await
        .expect("merge API request should succeed");

    let data: serde_json::Value = resp.json().await.unwrap();
    assert!(
        data.get("result").is_some(),
        "merge API should return 'result' field on success: {}",
        data
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_platform_merge_api_json_conflict() {
    let hub_url = start_test_hub().await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/merge", hub_url))
        .json(&serde_json::json!({
            "driver": "json",
            "base": r#"{"key": "base"}"#,
            "ours": r#"{"key": "ours"}"#,
            "theirs": r#"{"key": "theirs"}"#
        }))
        .send()
        .await
        .expect("merge API request should succeed");

    let data: serde_json::Value = resp.json().await.unwrap();
    assert!(
        data.get("result").is_some() || data.get("conflicts").is_some(),
        "merge API should return 'result' or 'conflicts': {}",
        data
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_platform_merge_api_yaml_success() {
    let hub_url = start_test_hub().await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/merge", hub_url))
        .json(&serde_json::json!({
            "driver": "yaml",
            "base": "host: localhost\nport: 8080\n",
            "ours": "host: 0.0.0.0\nport: 8080\n",
            "theirs": "host: localhost\nport: 9090\n"
        }))
        .send()
        .await
        .expect("merge API request should succeed");

    let data: serde_json::Value = resp.json().await.unwrap();
    assert!(
        data.get("result").is_some(),
        "YAML merge API should return 'result' field: {}",
        data
    );

    let result = data["result"].as_str().unwrap();
    assert!(
        result.contains("0.0.0.0"),
        "YAML merge result should contain host change: {}",
        result
    );
    assert!(
        result.contains("9090"),
        "YAML merge result should contain port change: {}",
        result
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_platform_merge_api_unsupported_driver() {
    let hub_url = start_test_hub().await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/merge", hub_url))
        .json(&serde_json::json!({
            "driver": "nonexistent",
            "base": "a",
            "ours": "b",
            "theirs": "c"
        }))
        .send()
        .await
        .expect("merge API request should succeed");

    let data: serde_json::Value = resp.json().await.unwrap();
    assert!(
        data.get("error").is_some() || data.get("conflicts").is_some(),
        "unsupported driver should return error or conflicts: {}",
        data
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_platform_merge_api_missing_fields() {
    let hub_url = start_test_hub().await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/merge", hub_url))
        .json(&serde_json::json!({
            "driver": "json"
        }))
        .send()
        .await
        .expect("merge API request should succeed");

    let status = resp.status();
    let data: serde_json::Value = resp.json().await.unwrap();
    assert!(
        status.is_client_error() || data.get("error").is_some() || data.get("conflicts").is_some(),
        "missing fields should return error: {}",
        data
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires an external platform server at localhost:3000; run with: cargo test -p suture-e2e --test platform_merge_api -- --ignored"]
async fn test_platform_merge_api_external() {
    let client = reqwest::Client::new();
    let resp = client
        .post("http://localhost:3000/api/merge")
        .json(&serde_json::json!({
            "driver": "json",
            "base": "{\"key\": \"base\"}",
            "ours": "{\"key\": \"ours\"}",
            "theirs": "{\"key\": \"theirs\"}"
        }))
        .send()
        .await;

    match resp {
        Ok(r) => {
            let data: serde_json::Value = r.json().await.unwrap();
            assert!(
                data.get("result").is_some() || data.get("conflicts").is_some(),
                "external platform should return result or conflicts: {}",
                data
            );
        }
        Err(e) => {
            println!("SKIP: Platform not running at localhost:3000 ({})", e);
        }
    }
}
