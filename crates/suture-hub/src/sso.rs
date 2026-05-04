//! OIDC Single Sign-On support for Suture Hub.
//!
//! Allows organizations to authenticate users via an external identity provider
//! (Google Workspace, Okta, Azure AD, Auth0, Keycloak, etc.) using the OpenID
//! Connect protocol.

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
    vec!["openid".to_owned(), "email".to_owned(), "profile".to_owned()]
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
}

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
pub fn generate_state() -> String {
    use rand::RngCore;
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 32];
    rng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Generate a nonce parameter for replay protection.
pub fn generate_nonce() -> String {
    generate_state()
}

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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_default_scopes() {
        let scopes = default_scopes();
        assert_eq!(scopes, vec!["openid", "email", "profile"]);
    }
}
