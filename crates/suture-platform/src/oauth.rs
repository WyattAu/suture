use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::auth::{self, UserInfo, AuthResponse};
use crate::db::PlatformDb;
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: String,
    pub state: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OAuthURLResponse {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct OAuthStartQuery {
    pub provider: String,
}

const GOOGLE_CLIENT_ID: &str = "";
const GOOGLE_CLIENT_SECRET: &str = "";
const GOOGLE_REDIRECT_URI: &str = "http://localhost:8080/auth/google/callback";

const GITHUB_CLIENT_ID: &str = "";
const GITHUB_CLIENT_SECRET: &str = "";
const GITHUB_REDIRECT_URI: &str = "http://localhost:8080/auth/github/callback";

#[derive(Debug, Serialize, Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
    token_type: String,
    expires_in: Option<i64>,
    id_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GoogleUserInfo {
    sub: String,
    email: String,
    name: Option<String>,
    picture: Option<String>,
    email_verified: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GitHubTokenResponse {
    access_token: String,
    token_type: String,
    scope: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GitHubUserInfo {
    id: i64,
    login: String,
    email: Option<String>,
    name: Option<String>,
    avatar_url: Option<String>,
}

pub async fn start_oauth(
    State(state): State<AppState>,
    Query(query): Query<OAuthStartQuery>,
) -> Result<Json<OAuthURLResponse>, (StatusCode, Json<serde_json::Value>)> {
    let _ = &state;
    let url = match query.provider.to_lowercase().as_str() {
        "google" => {
            let client_id = std::env::var("GOOGLE_CLIENT_ID").unwrap_or_else(|_| GOOGLE_CLIENT_ID.to_string());
            if client_id.is_empty() {
                return Err((
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({"error": "Google OAuth not configured", "hint": "set GOOGLE_CLIENT_ID env var"})),
                ));
            }
            let redirect = std::env::var("GOOGLE_REDIRECT_URI").unwrap_or_else(|_| GOOGLE_REDIRECT_URI.to_string());
            format!(
                "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope=openid%20email%20profile&access_type=offline",
                client_id, redirect
            )
        }
        "github" => {
            let client_id = std::env::var("GITHUB_CLIENT_ID").unwrap_or_else(|_| GITHUB_CLIENT_ID.to_string());
            if client_id.is_empty() {
                return Err((
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({"error": "GitHub OAuth not configured", "hint": "set GITHUB_CLIENT_ID env var"})),
                ));
            }
            let redirect = std::env::var("GITHUB_REDIRECT_URI").unwrap_or_else(|_| GITHUB_REDIRECT_URI.to_string());
            format!(
                "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=user:email",
                client_id, redirect
            )
        }
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "unsupported provider", "supported": ["google", "github"]})),
            ));
        }
    };

    Ok(Json(OAuthURLResponse { url }))
}

pub async fn google_callback(
    State(state): State<AppState>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<serde_json::Value>)> {
    let client_id = std::env::var("GOOGLE_CLIENT_ID").unwrap_or_else(|_| GOOGLE_CLIENT_ID.to_string());
    let client_secret = std::env::var("GOOGLE_CLIENT_SECRET").unwrap_or_else(|_| GOOGLE_CLIENT_SECRET.to_string());
    let redirect_uri = std::env::var("GOOGLE_REDIRECT_URI").unwrap_or_else(|_| GOOGLE_REDIRECT_URI.to_string());

    if client_id.is_empty() || client_secret.is_empty() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Google OAuth not configured"})),
        ));
    }

    let client = reqwest::Client::new();

    let grant_type = "authorization_code".to_string();
    let token_resp: GoogleTokenResponse = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", &query.code),
            ("client_id", &client_id),
            ("client_secret", &client_secret),
            ("redirect_uri", &redirect_uri),
            ("grant_type", &grant_type),
        ])
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))))?
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))))?;

    let google_user: GoogleUserInfo = client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(&token_resp.access_token)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))))?
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))))?;

    let user = find_or_create_oauth_user(
        &state.db,
        &format!("google:{}", google_user.sub),
        &google_user.email,
        google_user.name.as_deref(),
    )?;

    let token = auth::create_jwt(
        &user.user_id,
        &user.email,
        &user.tier,
        None,
        "user",
        &state.config.jwt_secret,
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    Ok(Json(AuthResponse { token, user }))
}

pub async fn github_callback(
    State(state): State<AppState>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<serde_json::Value>)> {
    let client_id = std::env::var("GITHUB_CLIENT_ID").unwrap_or_else(|_| GITHUB_CLIENT_ID.to_string());
    let client_secret = std::env::var("GITHUB_CLIENT_SECRET").unwrap_or_else(|_| GITHUB_CLIENT_SECRET.to_string());
    let redirect_uri = std::env::var("GITHUB_REDIRECT_URI").unwrap_or_else(|_| GITHUB_REDIRECT_URI.to_string());

    if client_id.is_empty() || client_secret.is_empty() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "GitHub OAuth not configured"})),
        ));
    }

    let client = reqwest::Client::new();

    let token_resp: GitHubTokenResponse = client
        .post("https://github.com/login/oauth/access_token")
        .form(&[
            ("code", &query.code),
            ("client_id", &client_id),
            ("client_secret", &client_secret),
            ("redirect_uri", &redirect_uri),
        ])
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))))?
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))))?;

    let github_user: GitHubUserInfo = client
        .get("https://api.github.com/user")
        .bearer_auth(&token_resp.access_token)
        .header("User-Agent", "suture-platform")
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))))?
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))))?;

    let email = match github_user.email {
        Some(e) if !e.is_empty() => e,
        _ => format!("{}@github.placeholder", github_user.login),
    };

    let user = find_or_create_oauth_user(
        &state.db,
        &format!("github:{}", github_user.id),
        &email,
        github_user.name.as_deref(),
    )?;

    let token = auth::create_jwt(
        &user.user_id,
        &user.email,
        &user.tier,
        None,
        "user",
        &state.config.jwt_secret,
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    Ok(Json(AuthResponse { token, user }))
}

fn find_or_create_oauth_user(
    db: &PlatformDb,
    provider_id: &str,
    email: &str,
    display_name: Option<&str>,
) -> Result<UserInfo, (StatusCode, Json<serde_json::Value>)> {
    let conn = db.conn()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e}))))?;

    let oauth_marker = format!("oauth:{}", provider_id);

    let existing: Result<(String, String, String, String, String), _> = conn.query_row(
        "SELECT user_id, email, display_name, tier, created_at FROM accounts WHERE password_hash = ?1",
        rusqlite::params![oauth_marker],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    );

    match existing {
        Ok((user_id, email, display_name, tier, created_at)) => {
            Ok(UserInfo { user_id, email, display_name, tier, created_at })
        }
        Err(_) => {
            let user_id = uuid::Uuid::new_v4().to_string();
            let display_name = display_name.unwrap_or("").to_string();

            let _ = conn.execute(
                "INSERT INTO accounts (user_id, email, password_hash, display_name) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![user_id, email, oauth_marker, display_name],
            );

            let month = chrono::Utc::now().format("%Y-%m").to_string();
            let _ = conn.execute(
                "INSERT OR IGNORE INTO usage (account_id, month) VALUES (?1, ?2)",
                rusqlite::params![user_id, month],
            );

            Ok(UserInfo {
                user_id: user_id.clone(),
                email: email.to_string(),
                display_name,
                tier: "free".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
            })
        }
    }
}
