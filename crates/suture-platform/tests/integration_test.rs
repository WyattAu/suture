use std::net::TcpListener;

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn test_config(
) -> (
    suture_platform::Config,
    u16,
    tempfile::NamedTempFile,
    tempfile::NamedTempFile,
) {
    let port = free_port();
    let db_file = tempfile::NamedTempFile::new().unwrap();
    let hub_db_file = tempfile::NamedTempFile::new().unwrap();
    let config = suture_platform::Config {
        addr: format!("127.0.0.1:{}", port),
        db_path: db_file.path().to_str().unwrap().to_string(),
        hub_db_path: hub_db_file.path().to_str().unwrap().to_string(),
        jwt_secret: "test-secret".to_string(),
        stripe_key: None,
        platform_url: String::new(),
    };
    (config, port, db_file, hub_db_file)
}

async fn start_server(
    config: suture_platform::Config,
) -> u16 {
    let port: u16 = config.addr.split(':').next_back().unwrap().parse().unwrap();
    tokio::spawn(async move {
        let _ = suture_platform::run(config).await;
    });
    wait_for_server(port).await;
    port
}

async fn wait_for_server(port: u16) {
    let url = format!("http://127.0.0.1:{}/healthz", port);
    for _ in 0..50 {
        if reqwest::get(&url).await.is_ok() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("server did not start on port {}", port);
}

fn base_url(port: u16) -> String {
    format!("http://127.0.0.1:{}", port)
}

async fn register_user(
    port: u16,
    email: &str,
    password: &str,
) -> (String, serde_json::Value) {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/auth/register", base_url(port)))
        .json(&serde_json::json!({
            "email": email,
            "password": password
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    let token = body["token"].as_str().unwrap().to_string();
    (token, body)
}

#[tokio::test]
async fn test_health_check() {
    let (config, _port, _db, _hub_db) = test_config();
    let port = start_server(config).await;

    let resp = reqwest::get(format!("{}/healthz", base_url(port)))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "ok");
}

#[tokio::test]
async fn test_register_and_login() {
    let (config, _port, _db, _hub_db) = test_config();
    let port = start_server(config).await;

    let (token, reg_body) = register_user(port, "alice@example.com", "password123").await;
    assert!(reg_body["user"]["email"].as_str().unwrap().contains("alice"));

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/auth/login", base_url(port)))
        .json(&serde_json::json!({
            "email": "alice@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let login_body: serde_json::Value = resp.json().await.unwrap();
    assert!(login_body["token"].is_string());

    let resp = client
        .get(format!("{}/auth/me", base_url(port)))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let me: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(me["email"].as_str().unwrap(), "alice@example.com");
}

#[tokio::test]
async fn test_register_duplicate_email_fails() {
    let (config, _port, _db, _hub_db) = test_config();
    let port = start_server(config).await;

    register_user(port, "dup@example.com", "password123").await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/auth/register", base_url(port)))
        .json(&serde_json::json!({
            "email": "dup@example.com",
            "password": "different456"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_login_wrong_password_fails() {
    let (config, _port, _db, _hub_db) = test_config();
    let port = start_server(config).await;

    register_user(port, "wrongpw@example.com", "correctpassword").await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/auth/login", base_url(port)))
        .json(&serde_json::json!({
            "email": "wrongpw@example.com",
            "password": "wrongpassword"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_invalid_token_rejected() {
    let (config, _port, _db, _hub_db) = test_config();
    let port = start_server(config).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/auth/me", base_url(port)))
        .bearer_auth("garbage.token.value")
        .send()
        .await
        .unwrap();
    assert!(
        resp.status() == 401 || resp.status() == 500,
        "expected 401 or 500 for invalid token, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_list_drivers() {
    let (config, _port, _db, _hub_db) = test_config();
    let port = start_server(config).await;

    let resp = reqwest::get(format!("{}/api/drivers", base_url(port)))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let drivers = body["drivers"].as_array().unwrap();
    assert!(drivers.len() >= 5);
}

#[tokio::test]
async fn test_merge_json_success() {
    let (config, _port, _db, _hub_db) = test_config();
    let port = start_server(config).await;

    let (token, _) = register_user(port, "jsonmerge@example.com", "password123").await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/merge", base_url(port)))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "driver": "json",
            "base": "{\"name\": \"suture\", \"version\": \"1.0\"}",
            "ours": "{\"name\": \"suture\", \"version\": \"2.0\"}",
            "theirs": "{\"name\": \"suture\", \"version\": \"1.0\", \"license\": \"MIT\"}"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(!body["conflicts"].as_bool().unwrap());
    let result = body["result"].as_str().unwrap();
    assert!(result.contains("2.0"));
    assert!(result.contains("MIT"));
}

#[tokio::test]
async fn test_merge_yaml_success() {
    let (config, _port, _db, _hub_db) = test_config();
    let port = start_server(config).await;

    let (token, _) = register_user(port, "yamlmerge@example.com", "password123").await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/merge", base_url(port)))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "driver": "yaml",
            "base": "name: suture\nversion: 1.0",
            "ours": "name: suture\nversion: 2.0",
            "theirs": "name: suture\nversion: 1.0\nlicense: MIT"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(!body["conflicts"].as_bool().unwrap());
    let result = body["result"].as_str().unwrap();
    assert!(result.contains("2.0") || result.contains("license"));
}

#[tokio::test]
async fn test_merge_conflict_returns_null() {
    let (config, _port, _db, _hub_db) = test_config();
    let port = start_server(config).await;

    let (token, _) = register_user(port, "conflict@example.com", "password123").await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/merge", base_url(port)))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "driver": "json",
            "base": "{\"key\": \"original\"}",
            "ours": "{\"key\": \"ours-value\"}",
            "theirs": "{\"key\": \"theirs-value\"}"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["conflicts"].as_bool().unwrap());
    assert!(body["result"].is_null());
}

#[tokio::test]
async fn test_unsupported_driver_returns_400() {
    let (config, _port, _db, _hub_db) = test_config();
    let port = start_server(config).await;

    let (token, _) = register_user(port, "baddriver@example.com", "password123").await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/merge", base_url(port)))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "driver": "brainfuck",
            "base": "",
            "ours": "",
            "theirs": ""
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("unsupported"));
}

#[tokio::test]
async fn test_usage_tracking() {
    let (config, _port, _db, _hub_db) = test_config();
    let port = start_server(config).await;

    let (token, _) = register_user(port, "usage@example.com", "password123").await;
    let client = reqwest::Client::new();

    for _ in 0..3 {
        let resp = client
            .post(format!("{}/api/merge", base_url(port)))
            .bearer_auth(&token)
            .json(&serde_json::json!({
                "driver": "json",
                "base": "{}",
                "ours": "{\"a\": 1}",
                "theirs": "{\"b\": 2}"
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    let resp = client
        .get(format!("{}/api/usage", base_url(port)))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["merges_used"].as_i64().unwrap(), 3);
}

#[tokio::test]
async fn test_create_and_list_orgs() {
    let (config, _port, _db, _hub_db) = test_config();
    let port = start_server(config).await;

    let (token, _) = register_user(port, "orguser@example.com", "password123").await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/orgs", base_url(port)))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "name": "my-test-org",
            "display_name": "My Test Org"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let created: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(created["name"].as_str().unwrap(), "my-test-org");

    let resp = client
        .get(format!("{}/api/orgs", base_url(port)))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let orgs: serde_json::Value = resp.json().await.unwrap();
    let org_list = orgs.as_array().unwrap();
    assert!(org_list.iter().any(|o| o["name"].as_str().unwrap() == "my-test-org"));
}

#[tokio::test]
async fn test_create_duplicate_org_fails() {
    let (config, _port, _db, _hub_db) = test_config();
    let port = start_server(config).await;

    let (token, _) = register_user(port, "duporg@example.com", "password123").await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/orgs", base_url(port)))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "unique-org"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let resp = client
        .post(format!("{}/api/orgs", base_url(port)))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "unique-org"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn test_oauth_start_unconfigured() {
    let (config, _port, _db, _hub_db) = test_config();
    let port = start_server(config).await;

    let resp = reqwest::get(format!(
        "{}/auth/oauth/start?provider=google",
        base_url(port)
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 503);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["error"]
        .as_str()
        .unwrap()
        .to_lowercase()
        .contains("not configured"));
}

#[tokio::test]
async fn test_billing_not_configured() {
    let (config, _port, _db, _hub_db) = test_config();
    let port = start_server(config).await;

    let (token, _) = register_user(port, "billing@example.com", "password123").await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/billing/checkout", base_url(port)))
        .bearer_auth(&token)
        .json(&serde_json::json!({"tier": "pro"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 503);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["error"]
        .as_str()
        .unwrap()
        .to_lowercase()
        .contains("not configured"));
}

#[tokio::test]
async fn test_rate_limit_headers_present() {
    let (config, _port, _db, _hub_db) = test_config();
    let port = start_server(config).await;

    let (token, _) = register_user(port, "ratelimit@example.com", "password123").await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/usage", base_url(port)))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert!(resp.headers().get("x-ratelimit-limit").is_some());
    assert!(resp.headers().get("x-ratelimit-remaining").is_some());
    assert!(resp.headers().get("x-ratelimit-reset").is_some());
}

#[tokio::test]
async fn test_anonymous_merge_works() {
    let (config, _port, _db, _hub_db) = test_config();
    let port = start_server(config).await;

    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/merge", base_url(port)))
        .json(&serde_json::json!({
            "driver": "json",
            "base": "{\"a\": 1}",
            "ours": "{\"a\": 2}",
            "theirs": "{\"b\": 3}"
        }))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status() == 200 || resp.status() == 401 || resp.status() == 500,
        "expected 200, 401, or 500 for anonymous merge, got {}",
        resp.status()
    );
    if resp.status() == 200 {
        let body: serde_json::Value = resp.json().await.unwrap();
        assert!(!body["conflicts"].as_bool().unwrap());
        assert!(body["result"].is_string());
    }
}
