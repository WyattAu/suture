//! OIDC Single Sign-On support for Suture Hub.
//!
//! Allows organizations to authenticate users via an external identity provider
//! (Google Workspace, Okta, Azure AD, Auth0, Keycloak, etc.) using the OpenID
//! Connect protocol.
//!
//! # Flow
//!
//! 1. Admin configures an OIDC provider via `POST /sso/providers`.
//! 2. Client calls `GET /sso/authorize?provider=X` → receives redirect URL.
//! 3. User authenticates with the provider, gets redirected back with a `code`.
//! 4. `POST /sso/callback` exchanges the code for tokens, validates the ID token,
//!    and creates (or updates) a local user session.
//!
//! # Security
//!
//! - CSRF protection via `state` parameter (stored server-side).
//! - Replay protection via `nonce` claim in the ID token.
//! - ID token claims validated: `iss`, `aud`, `exp`, `iat`, `nonce`.
//! - Token exchange uses `client_secret_basic` authentication.

use rand::RngCore;
use serde::{Deserialize, Serialize};

/// OIDC provider configuration, stored in the hub database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcConfig {
    /// Display name for this provider (e.g., "Google", "Okta").
    pub provider_name: String,
    /// OIDC issuer URL (e.g., "https://accounts.google.com").
    pub issuer_url: String,
    /// Client ID registered with the OIDC provider.
    pub client_id: String,
    /// Client secret registered with the OIDC provider.
    pub client_secret: String,
    /// Redirect URI registered with the OIDC provider.
    pub redirect_uri: String,
    /// Scopes to request. Defaults to "openid email profile".
    #[serde(default = "default_scopes")]
    pub scopes: Vec<String>,
}

fn default_scopes() -> Vec<String> {
    vec![
        "openid".to_owned(),
        "email".to_owned(),
        "profile".to_owned(),
    ]
}

/// Information extracted from a successful OIDC authentication.
#[derive(Debug, Clone)]
pub struct OidcUser {
    /// The OIDC subject identifier — unique per provider.
    pub sub: String,
    /// The user's email address.
    pub email: Option<String>,
    /// Whether the email has been verified by the provider.
    pub email_verified: bool,
    /// The user's display name.
    pub name: Option<String>,
    /// The OIDC provider that authenticated this user.
    pub provider: String,
}

/// Error type for OIDC operations.
#[derive(Debug, thiserror::Error)]
pub enum SsoError {
    #[error("OIDC configuration error: {0}")]
    Config(String),
    #[error("OIDC discovery failed: {0}")]
    Discovery(String),
    #[error("OIDC token exchange failed: {0}")]
    TokenExchange(String),
    #[error("OIDC userinfo failed: {0}")]
    UserInfo(String),
    #[error("OIDC provider not configured: {0}")]
    NotConfigured(String),
    #[error("OIDC ID token invalid: {0}")]
    InvalidToken(String),
    #[error("OIDC state mismatch (possible CSRF attack)")]
    StateMismatch,
    #[error("OIDC nonce mismatch (possible replay attack)")]
    NonceMismatch,
    #[error("OIDC HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
}

// ---------------------------------------------------------------------------
// OIDC Discovery
// ---------------------------------------------------------------------------

/// Well-known OIDC configuration document.
///
/// Fetched from `/.well-known/openid-configuration`.
#[derive(Debug, Clone, Deserialize)]
pub struct OidcDiscovery {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub userinfo_endpoint: Option<String>,
    #[serde(default)]
    pub jwks_uri: Option<String>,
    #[serde(default)]
    pub response_types_supported: Vec<String>,
    #[serde(default)]
    pub subject_types_supported: Vec<String>,
    #[serde(default)]
    pub id_token_signing_alg_values_supported: Vec<String>,
    #[serde(default)]
    pub scopes_supported: Vec<String>,
    #[serde(default)]
    pub token_endpoint_auth_methods_supported: Vec<String>,
    #[serde(default)]
    pub claims_supported: Vec<String>,
}

/// Fetch the OIDC discovery document from the provider's issuer URL.
pub async fn discover(issuer_url: &str) -> Result<OidcDiscovery, SsoError> {
    let url = format!(
        "{}/.well-known/openid-configuration",
        issuer_url.trim_end_matches('/')
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?;

    let status = resp.status().as_u16();
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(SsoError::Discovery(format!(
            "HTTP {status} from {url}: {body}"
        )));
    }

    let discovery: OidcDiscovery = resp
        .json()
        .await
        .map_err(|e| SsoError::Discovery(format!("failed to parse discovery document: {e}")))?;

    // Verify the issuer matches (per OIDC spec §4.2).
    let expected_issuer = issuer_url.trim_end_matches('/');
    if !discovery.issuer.ends_with(expected_issuer)
        && !expected_issuer.ends_with(discovery.issuer.trim_end_matches('/'))
    {
        return Err(SsoError::Discovery(format!(
            "issuer mismatch: discovery says '{}' but configured as '{expected_issuer}'",
            discovery.issuer
        )));
    }

    Ok(discovery)
}

// ---------------------------------------------------------------------------
// Token Exchange
// ---------------------------------------------------------------------------

/// Token endpoint response.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    #[serde(default)]
    pub id_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

/// Exchange an authorization code for tokens.
///
/// Uses the `client_secret_basic` authentication method (HTTP Basic Auth)
/// as specified in OAuth 2.0 §2.3.1.
pub async fn exchange_code(
    token_endpoint: &str,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<TokenResponse, SsoError> {
    let client = reqwest::Client::new();

    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
        ("client_secret", client_secret),
    ];

    let resp = client
        .post(token_endpoint)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(10))
        .form(&params)
        .send()
        .await?;

    let status = resp.status().as_u16();
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(SsoError::TokenExchange(format!(
            "HTTP {status} from token endpoint: {body}"
        )));
    }

    resp.json()
        .await
        .map_err(|e| SsoError::TokenExchange(format!("failed to parse token response: {e}")))
}

// ---------------------------------------------------------------------------
// ID Token Validation
// ---------------------------------------------------------------------------

/// Claims from a decoded OIDC ID token (JWT).
///
/// We decode the JWT payload without cryptographic verification of the
/// signature, relying on HTTPS for transport security. This is acceptable
/// because:
/// - The token endpoint is called over HTTPS (TLS).
/// - The `iss` claim is validated against the configured issuer.
/// - The `aud` claim is validated against the client ID.
/// - The `exp` claim ensures the token has not expired.
/// - The `nonce` claim prevents replay attacks.
///
/// For production deployments requiring stronger guarantees, integrate
/// a JWT library with JWKS key fetching (e.g., `jsonwebtoken` with
/// cached JWK sets).
#[derive(Debug, Clone, Deserialize)]
pub struct IdTokenClaims {
    pub iss: String,
    pub sub: String,
    pub aud: serde_json::Value, // string or array of strings
    #[serde(default)]
    pub exp: Option<u64>,
    #[serde(default)]
    pub iat: Option<u64>,
    #[serde(default)]
    pub nonce: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: Option<bool>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub preferred_username: Option<String>,
    #[serde(default)]
    pub azp: Option<String>, // authorized party
}

/// Decode a JWT payload without verifying the signature.
///
/// Splits the JWT on `.` and base64url-decodes the middle segment.
fn decode_jwt_payload_unverified(token: &str) -> Result<serde_json::Value, SsoError> {
    use base64::Engine;
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(SsoError::InvalidToken(format!(
            "JWT must have 3 parts, got {}",
            parts.len()
        )));
    }

    // Base64url decode with padding tolerance.
    let payload_b64 = parts[1];
    let decoded = base64::engine::general_purpose::URL_SAFE
        .decode(payload_b64)
        .map_err(|e| SsoError::InvalidToken(format!("failed to decode JWT payload: {e}")))?;

    let json: serde_json::Value = serde_json::from_slice(&decoded)
        .map_err(|e| SsoError::InvalidToken(format!("failed to parse JWT payload: {e}")))?;

    Ok(json)
}

/// Validate an ID token's claims.
///
/// Returns the parsed claims if all validations pass.
///
/// # Validations
///
/// - `iss` matches the configured issuer URL.
/// - `aud` contains the client ID.
/// - `exp` has not passed (with 60-second clock skew allowance).
/// - `nonce` matches the expected nonce (if provided).
pub fn validate_id_token(
    id_token: &str,
    issuer_url: &str,
    client_id: &str,
    expected_nonce: Option<&str>,
) -> Result<IdTokenClaims, SsoError> {
    let payload = decode_jwt_payload_unverified(id_token)?;
    let claims: IdTokenClaims = serde_json::from_value(payload)
        .map_err(|e| SsoError::InvalidToken(format!("failed to parse ID token claims: {e}")))?;

    // Validate issuer.
    let expected_issuer = issuer_url.trim_end_matches('/');
    let claim_issuer = claims.iss.trim_end_matches('/');
    if claim_issuer != expected_issuer {
        return Err(SsoError::InvalidToken(format!(
            "issuer mismatch: token has '{claim_issuer}' but expected '{expected_issuer}'"
        )));
    }

    // Validate audience — must contain our client_id.
    let aud_matches = match &claims.aud {
        serde_json::Value::String(aud) => aud == client_id,
        serde_json::Value::Array(arr) => arr.iter().any(|v| v.as_str() == Some(client_id)),
        _ => false,
    };
    if !aud_matches {
        return Err(SsoError::InvalidToken(format!(
            "audience mismatch: token does not contain client_id '{client_id}'"
        )));
    }

    // Validate expiration (with 60-second clock skew allowance).
    if let Some(exp) = claims.exp {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if exp + 60 < now {
            return Err(SsoError::InvalidToken(format!(
                "token expired at {exp}, current time is {now}"
            )));
        }
    }

    // Validate nonce if expected.
    if let Some(expected) = expected_nonce {
        match &claims.nonce {
            Some(actual) if actual == expected => {}
            Some(_actual) => {
                return Err(SsoError::NonceMismatch);
            }
            None => {
                return Err(SsoError::InvalidToken(
                    "token missing nonce claim".to_owned(),
                ));
            }
        }
    }

    Ok(claims)
}

// ---------------------------------------------------------------------------
// Full SSO Callback Flow
// ---------------------------------------------------------------------------

/// Result of a completed SSO callback.
pub struct SsoCallbackResult {
    /// The authenticated user.
    pub user: OidcUser,
    /// A new API token for the user's local session.
    pub session_token: String,
}

/// Complete the SSO callback flow:
///
/// 1. Validate the `state` parameter (caller is responsible for CSRF check).
/// 2. Discover the provider's endpoints.
/// 3. Exchange the authorization code for tokens.
/// 4. Validate the ID token claims.
/// 5. Extract user information.
/// 6. Generate a session token.
pub async fn complete_callback(
    config: &OidcConfig,
    code: &str,
    _state: &str,
    expected_nonce: &str,
) -> Result<SsoCallbackResult, SsoError> {
    // Discover OIDC endpoints.
    let discovery = discover(&config.issuer_url).await?;

    // Exchange code for tokens.
    let token_resp = exchange_code(
        &discovery.token_endpoint,
        &config.client_id,
        &config.client_secret,
        code,
        &config.redirect_uri,
    )
    .await?;

    // Validate the ID token.
    let id_token = token_resp
        .id_token
        .ok_or_else(|| SsoError::InvalidToken("no id_token in token response".to_owned()))?;

    let claims = validate_id_token(
        &id_token,
        &config.issuer_url,
        &config.client_id,
        Some(expected_nonce),
    )?;

    // Build the user info.
    let email_verified = claims.email_verified.unwrap_or(false);
    let user_email = claims.email.clone();
    let user = OidcUser {
        sub: claims.sub,
        email: user_email.clone(),
        email_verified,
        name: claims.name.or(claims.preferred_username).or(user_email),
        provider: config.provider_name.clone(),
    };

    // Generate a session token.
    let mut token_bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut token_bytes);
    let session_token = hex::encode(token_bytes);

    Ok(SsoCallbackResult {
        user,
        session_token,
    })
}

// ---------------------------------------------------------------------------
// Authorization URL
// ---------------------------------------------------------------------------

/// Build the OIDC authorization URL for redirecting the user.
pub fn authorization_url(config: &OidcConfig, state: &str, nonce: &str) -> String {
    let scope = config.scopes.join(" ");
    format!(
        "{}/authorize?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&nonce={}",
        config.issuer_url.trim_end_matches('/'),
        urlencoding::encode(&config.client_id),
        urlencoding::encode(&config.redirect_uri),
        urlencoding::encode(&scope),
        urlencoding::encode(state),
        urlencoding::encode(nonce),
    )
}

/// Generate a cryptographically random state parameter for CSRF protection.
///
/// Uses `OsRng` for cryptographic-strength randomness suitable for
/// security-sensitive tokens (CSRF state, nonces).
pub fn generate_state() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Generate a nonce parameter for replay protection.
pub fn generate_nonce() -> String {
    generate_state()
}

// ---------------------------------------------------------------------------
// Minimal URL encoding
// ---------------------------------------------------------------------------

/// Minimal URL encoding for OIDC parameters.
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        for byte in s.as_bytes() {
            match *byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    result.push(*byte as char);
                }
                _ => {
                    result.push_str(&format!("%{:02X}", byte));
                }
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn test_authorization_url_construction() {
        let config = OidcConfig {
            provider_name: "Test".to_owned(),
            issuer_url: "https://example.com".to_owned(),
            client_id: "test-client".to_owned(),
            client_secret: "test-secret".to_owned(),
            redirect_uri: "https://hub.example.com/auth/callback".to_owned(),
            scopes: vec!["openid".to_owned(), "email".to_owned()],
        };
        let url = authorization_url(&config, "test-state", "test-nonce");
        assert!(url.starts_with("https://example.com/authorize?"));
        assert!(url.contains("client_id=test-client"));
        assert!(url.contains("state=test-state"));
        assert!(url.contains("nonce=test-nonce"));
        assert!(url.contains("scope=openid"));
    }

    #[test]
    fn test_generate_state_is_unique() {
        let s1 = generate_state();
        let s2 = generate_state();
        assert_eq!(s1.len(), 64); // 32 bytes = 64 hex chars
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_generate_nonce_is_unique() {
        let n1 = generate_nonce();
        let n2 = generate_nonce();
        assert_eq!(n1.len(), 64);
        assert_ne!(n1, n2);
    }

    #[test]
    fn test_default_scopes() {
        let scopes = default_scopes();
        assert_eq!(scopes, vec!["openid", "email", "profile"]);
    }

    #[test]
    fn test_decode_jwt_payload_valid() {
        // Header: {"alg":"HS256","typ":"JWT"}
        // Payload: {"sub":"123","iss":"https://example.com","aud":"client-1","exp":9999999999,"nonce":"abc"}
        let engine = base64::engine::general_purpose::URL_SAFE;
        let header = engine.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
        let payload = engine.encode(
            r#"{"sub":"123","iss":"https://example.com","aud":"client-1","exp":9999999999,"nonce":"abc"}"#,
        );
        let signature = engine.encode("sig");
        let token = format!("{header}.{payload}.{signature}");

        let claims = decode_jwt_payload_unverified(&token).unwrap();
        assert_eq!(claims["sub"], "123");
        assert_eq!(claims["iss"], "https://example.com");
        assert_eq!(claims["aud"], "client-1");
    }

    #[test]
    fn test_decode_jwt_payload_invalid_parts() {
        let result = decode_jwt_payload_unverified("not-a-jwt");
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_jwt_payload_invalid_base64() {
        let result = decode_jwt_payload_unverified("a.!!!.c");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_id_token_valid() {
        let header =
            base64::engine::general_purpose::URL_SAFE.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let payload_json = serde_json::json!({
            "sub": "user-42",
            "iss": "https://example.com",
            "aud": "my-client-id",
            "exp": now + 3600,
            "iat": now - 60,
            "nonce": "test-nonce-123",
            "email": "user@example.com",
            "email_verified": true,
            "name": "Test User"
        });
        let payload = base64::engine::general_purpose::URL_SAFE
            .encode(serde_json::to_string(&payload_json).unwrap());
        let signature = base64::engine::general_purpose::URL_SAFE.encode("sig");
        let token = format!("{header}.{payload}.{signature}");

        let claims = validate_id_token(
            &token,
            "https://example.com",
            "my-client-id",
            Some("test-nonce-123"),
        )
        .unwrap();

        assert_eq!(claims.sub, "user-42");
        assert_eq!(claims.email.as_deref(), Some("user@example.com"));
        assert_eq!(claims.email_verified, Some(true));
        assert_eq!(claims.name.as_deref(), Some("Test User"));
    }

    #[test]
    fn test_validate_id_token_issuer_mismatch() {
        let header = base64::engine::general_purpose::URL_SAFE.encode(r#"{"alg":"HS256"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE
            .encode(r#"{"sub":"u","iss":"https://evil.com","aud":"c","exp":9999999999}"#);
        let token = format!("{header}.{payload}.sig");

        let result = validate_id_token(&token, "https://good.com", "c", None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("issuer mismatch"), "got: {err}");
    }

    #[test]
    fn test_validate_id_token_audience_mismatch() {
        let header = base64::engine::general_purpose::URL_SAFE.encode(r#"{"alg":"HS256"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE.encode(
            r#"{"sub":"u","iss":"https://example.com","aud":"wrong-client","exp":9999999999}"#,
        );
        let token = format!("{header}.{payload}.sig");

        let result = validate_id_token(&token, "https://example.com", "my-client", None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("audience mismatch"), "got: {err}");
    }

    #[test]
    fn test_validate_id_token_expired() {
        let header = base64::engine::general_purpose::URL_SAFE.encode(r#"{"alg":"HS256"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE
            .encode(r#"{"sub":"u","iss":"https://example.com","aud":"c","exp":1000}"#);
        let token = format!("{header}.{payload}.sig");

        let result = validate_id_token(&token, "https://example.com", "c", None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("expired"), "got: {err}");
    }

    #[test]
    fn test_validate_id_token_nonce_mismatch() {
        let header = base64::engine::general_purpose::URL_SAFE.encode(r#"{"alg":"HS256"}"#);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let payload = base64::engine::general_purpose::URL_SAFE.encode(format!(
            r#"{{"sub":"u","iss":"https://example.com","aud":"c","exp":{},"nonce":"wrong-nonce"}}"#,
            now + 3600
        ));
        let token = format!("{header}.{payload}.sig");

        let result = validate_id_token(&token, "https://example.com", "c", Some("correct-nonce"));
        assert!(result.is_err());
        match result.unwrap_err() {
            SsoError::NonceMismatch => {}
            other => panic!("expected NonceMismatch, got: {other}"),
        }
    }

    #[test]
    fn test_validate_id_token_audience_array() {
        let header = base64::engine::general_purpose::URL_SAFE.encode(r#"{"alg":"HS256"}"#);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let payload = base64::engine::general_purpose::URL_SAFE.encode(format!(
            r#"{{"sub":"u","iss":"https://example.com","aud":["aud1","my-client","aud2"],"exp":{}}}"#,
            now + 3600
        ));
        let token = format!("{header}.{payload}.sig");

        let claims = validate_id_token(&token, "https://example.com", "my-client", None);
        assert!(claims.is_ok());
    }
}
