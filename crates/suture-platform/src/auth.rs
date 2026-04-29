use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::db::PlatformDb;

const SESSION_DURATION_HOURS: i64 = 24 * 7;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub tier: String,
    pub org_id: Option<String>,
    pub role: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub user_id: String,
    pub email: String,
    pub display_name: String,
    pub tier: String,
    pub created_at: String,
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("password hashing failed: {e}"))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> anyhow::Result<bool> {
    let parsed = PasswordHash::new(hash).map_err(|e| anyhow::anyhow!("invalid password hash: {e}"))?;
    Ok(Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
}

pub fn create_jwt(
    user_id: &str,
    email: &str,
    tier: &str,
    org_id: Option<&str>,
    role: &str,
    secret: &str,
) -> anyhow::Result<String> {
    let now = Utc::now();
    let exp = now.timestamp() as usize + (SESSION_DURATION_HOURS * 3600) as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        tier: tier.to_string(),
        org_id: org_id.map(|s| s.to_string()),
        role: role.to_string(),
        exp,
        iat: now.timestamp() as usize,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    Ok(token)
}

pub fn verify_jwt(token: &str, secret: &str) -> anyhow::Result<Claims> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}

pub fn register_user(db: &PlatformDb, req: &RegisterRequest) -> anyhow::Result<(String, UserInfo)> {
    if !req.email.contains('@') || req.email.len() > 254 {
        anyhow::bail!("invalid email address");
    }
    if req.password.len() < 8 {
        anyhow::bail!("password must be at least 8 characters");
    }

    let user_id = uuid::Uuid::new_v4().to_string();
    let password_hash = hash_password(&req.password)?;
    let display_name = req.display_name.clone().unwrap_or_default();

    let conn = db.conn().map_err(|e| anyhow::anyhow!("{e}"))?;
    conn.execute(
        "INSERT INTO accounts (user_id, email, password_hash, display_name) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![user_id, req.email, password_hash, display_name],
    )?;

    let month = Utc::now().format("%Y-%m").to_string();
    conn.execute(
        "INSERT OR IGNORE INTO usage (account_id, month) VALUES (?1, ?2)",
        rusqlite::params![user_id, month],
    )?;

    Ok((
        user_id.clone(),
        UserInfo {
            user_id,
            email: req.email.clone(),
            display_name,
            tier: "free".to_string(),
            created_at: Utc::now().to_rfc3339(),
        },
    ))
}

pub fn login_user(db: &PlatformDb, req: &LoginRequest) -> anyhow::Result<(String, UserInfo)> {
    let conn = db.conn().map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut stmt = conn.prepare(
        "SELECT user_id, email, password_hash, display_name, tier, created_at FROM accounts WHERE email = ?1",
    )?;
    let mut rows = stmt.query(rusqlite::params![req.email])?;

    let row = rows.next()?.ok_or_else(|| anyhow::anyhow!("invalid email or password"))?;
    let user_id: String = row.get(0)?;
    let email: String = row.get(1)?;
    let password_hash: String = row.get(2)?;
    let display_name: String = row.get(3)?;
    let tier: String = row.get(4)?;
    let created_at: String = row.get(5)?;

    if !verify_password(&req.password, &password_hash)? {
        anyhow::bail!("invalid email or password");
    }

    Ok((
        user_id.clone(),
        UserInfo {
            user_id,
            email,
            display_name,
            tier,
            created_at,
        },
    ))
}

pub fn get_user_by_id(db: &PlatformDb, user_id: &str) -> anyhow::Result<UserInfo> {
    let conn = db.conn().map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut stmt = conn.prepare(
        "SELECT user_id, email, display_name, tier, created_at FROM accounts WHERE user_id = ?1",
    )?;
    let mut rows = stmt.query(rusqlite::params![user_id])?;

    let row = rows.next()?.ok_or_else(|| anyhow::anyhow!("user not found"))?;
    Ok(UserInfo {
        user_id: row.get(0)?,
        email: row.get(1)?,
        display_name: row.get(2)?,
        tier: row.get(3)?,
        created_at: row.get(4)?,
    })
}

use axum::{extract::State, http::StatusCode, Extension, Json};
use crate::server::AppState;

pub async fn register_handler(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<AuthResponse>), (StatusCode, Json<serde_json::Value>)> {
    match register_user(&state.db, &req) {
        Ok((user_id, user)) => {
            let token = create_jwt(
                &user_id,
                &user.email,
                &user.tier,
                None,
                "user",
                &state.config.jwt_secret,
            )
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
            })?;
            Ok((StatusCode::CREATED, Json(AuthResponse { token, user })))
        }
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}

pub async fn login_handler(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<serde_json::Value>)> {
    match login_user(&state.db, &req) {
        Ok((user_id, user)) => {
            let token = create_jwt(
                &user_id,
                &user.email,
                &user.tier,
                None,
                "user",
                &state.config.jwt_secret,
            )
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
            })?;
            Ok(Json(AuthResponse { token, user }))
        }
        Err(e) => Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}

pub async fn me_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<UserInfo>, (StatusCode, Json<serde_json::Value>)> {
    match get_user_by_id(&state.db, &claims.sub) {
        Ok(user) => Ok(Json(user)),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}

pub async fn list_users_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<UserInfo>>, (StatusCode, Json<serde_json::Value>)> {
    if claims.role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "admin only"})),
        ));
    }
    let conn = state.db.conn().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    let mut stmt = conn
        .prepare(
            "SELECT user_id, email, display_name, tier, created_at FROM accounts ORDER BY created_at DESC",
        )
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
    let users = stmt
        .query_map([], |row| {
            Ok(UserInfo {
                user_id: row.get(0)?,
                email: row.get(1)?,
                display_name: row.get(2)?,
                tier: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
    Ok(Json(users))
}
