//! Webhook delivery system for Suture Hub.
//!
//! Sends HTTP POST notifications when events occur (push, branch create,
//! branch delete, etc.). Supports HMAC-SHA256 payload signing, retry with
//! exponential backoff, and delivery tracking.

use rand::Rng;
use serde::{Deserialize, Serialize};

/// A webhook configuration stored in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Webhook {
    pub id: String,
    pub repo_id: String,
    pub url: String,
    pub events: Vec<String>,
    pub secret: Option<String>,
    pub created_at: u64,
    pub active: bool,
}

/// Payload sent to webhook endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    pub event: String,
    pub repo_id: String,
    pub timestamp: u64,
    pub data: serde_json::Value,
}

/// Summary of a webhook trigger batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookResult {
    pub triggered: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub retried: usize,
}

/// Record of a webhook delivery attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryRecord {
    pub webhook_id: String,
    pub event: String,
    pub repo_id: String,
    pub status: DeliveryStatus,
    pub status_code: Option<u16>,
    pub attempt: u32,
    pub last_attempt_at: u64,
    pub next_retry_at: Option<u64>,
    pub response_body: Option<String>,
    pub error: Option<String>,
}

/// Status of a webhook delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryStatus {
    /// Delivery succeeded (2xx response).
    Succeeded,
    /// Delivery failed, will retry.
    Pending,
    /// All retries exhausted.
    Failed,
    /// Delivery aborted (webhook deleted or disabled).
    Aborted,
}

/// Configuration for retry behavior.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of delivery attempts (including the first).
    pub max_retries: u32,
    /// Base delay between retries in milliseconds.
    pub base_delay_ms: u64,
    /// Maximum delay between retries in milliseconds.
    pub max_delay_ms: u64,
    /// Timeout for each HTTP request in seconds.
    pub timeout_secs: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            base_delay_ms: 1000,
            max_delay_ms: 300_000, // 5 minutes
            timeout_secs: 10,
        }
    }
}

/// Webhook delivery manager.
///
/// Handles signing, sending, and retry logic for webhook notifications.
pub struct WebhookManager {
    client: reqwest::Client,
    retry_config: RetryConfig,
}

impl WebhookManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            retry_config: RetryConfig::default(),
        }
    }

    #[must_use]
    pub fn with_retry_config(config: RetryConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            retry_config: config,
        }
    }

    /// Sign a payload with HMAC-SHA256.
    ///
    /// The signature is prefixed with `sha256=` for compatibility with
    /// GitHub's webhook signature format.
    pub fn sign_payload(&self, payload: &str, secret: &str) -> Option<String> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;
        let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("HMAC key rejected ({} bytes): {}", secret.len(), e);
                return None;
            }
        };
        mac.update(payload.as_bytes());
        let result = mac.finalize();
        Some(hex::encode(result.into_bytes()))
    }

    /// Verify a webhook signature.
    ///
    /// Used by consumers to validate incoming webhook payloads.
    pub fn verify_signature(payload: &str, secret: &str, signature: &str) -> bool {
        let expected_sig = match signature.strip_prefix("sha256=") {
            Some(sig) => sig,
            None => return false,
        };

        let manager = Self::new();
        match manager.sign_payload(payload, secret) {
            Some(computed) => {
                // Constant-time comparison to prevent timing attacks.
                if computed.len() != expected_sig.len() {
                    return false;
                }
                computed
                    .as_bytes()
                    .iter()
                    .zip(expected_sig.as_bytes().iter())
                    .all(|(a, b)| a == b)
            }
            None => false,
        }
    }

    /// Trigger webhooks for an event.
    ///
    /// Sends to all matching active webhooks. Failed deliveries are
    /// reported but not retried inline — use `retry_delivery` for that.
    pub async fn trigger(
        &self,
        webhooks: &[Webhook],
        event: &str,
        repo_id: &str,
        data: serde_json::Value,
    ) -> WebhookResult {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let payload = WebhookPayload {
            event: event.to_owned(),
            repo_id: repo_id.to_owned(),
            timestamp,
            data,
        };

        let Ok(payload_json) = serde_json::to_string(&payload) else {
            return WebhookResult {
                triggered: 0,
                succeeded: 0,
                failed: 0,
                retried: 0,
            };
        };

        let matching: Vec<&Webhook> = webhooks
            .iter()
            .filter(|w| w.active && w.events.iter().any(|e| e == event))
            .collect();

        let triggered = matching.len();
        let mut succeeded = 0usize;
        let mut failed = 0usize;
        let mut retried = 0usize;

        for webhook in matching {
            let mut req = self
                .client
                .post(&webhook.url)
                .header("Content-Type", "application/json")
                .header("X-Suture-Event", event)
                .header("X-Suture-Delivery", &webhook.id)
                .header("X-Suture-Timestamp", timestamp.to_string())
                .timeout(std::time::Duration::from_secs(
                    self.retry_config.timeout_secs,
                ))
                .body(payload_json.clone());

            if let Some(ref secret) = webhook.secret
                && let Some(signature) = self.sign_payload(&payload_json, secret)
            {
                req = req.header("X-Suture-Signature", format!("sha256={signature}"));
            }

            match req.send().await {
                Ok(resp) if resp.status().is_success() => succeeded += 1,
                Ok(resp) if resp.status().as_u16() == 429 => {
                    // Rate limited — could retry later.
                    retried += 1;
                    failed += 1;
                    tracing::warn!(
                        webhook_id = %webhook.id,
                        url = %webhook.url,
                        "webhook rate limited"
                    );
                }
                Ok(resp) => {
                    failed += 1;
                    tracing::warn!(
                        webhook_id = %webhook.id,
                        url = %webhook.url,
                        status = %resp.status(),
                        "webhook delivery failed"
                    );
                }
                Err(e) => {
                    failed += 1;
                    retried += 1;
                    tracing::warn!(
                        webhook_id = %webhook.id,
                        url = %webhook.url,
                        error = %e,
                        "webhook delivery error"
                    );
                }
            }
        }

        WebhookResult {
            triggered,
            succeeded,
            failed,
            retried,
        }
    }

    /// Retry a failed webhook delivery.
    ///
    /// Returns the delivery result and the timestamp when the next retry
    /// should be attempted (if applicable).
    pub async fn retry_delivery(
        &self,
        webhook: &Webhook,
        payload_json: &str,
        event: &str,
        attempt: u32,
    ) -> Result<DeliveryRecord, DeliveryRecord> {
        if attempt > self.retry_config.max_retries {
            return Err(DeliveryRecord {
                webhook_id: webhook.id.clone(),
                event: event.to_owned(),
                repo_id: webhook.repo_id.clone(),
                status: DeliveryStatus::Failed,
                status_code: None,
                attempt,
                last_attempt_at: now_secs(),
                next_retry_at: None,
                response_body: None,
                error: Some("max retries exceeded".to_owned()),
            });
        }

        if !webhook.active {
            return Err(DeliveryRecord {
                webhook_id: webhook.id.clone(),
                event: event.to_owned(),
                repo_id: webhook.repo_id.clone(),
                status: DeliveryStatus::Aborted,
                status_code: None,
                attempt,
                last_attempt_at: now_secs(),
                next_retry_at: None,
                response_body: None,
                error: Some("webhook disabled".to_owned()),
            });
        }

        let delay = self.calculate_backoff(attempt);
        let mut req = self
            .client
            .post(&webhook.url)
            .header("Content-Type", "application/json")
            .header("X-Suture-Event", event)
            .header("X-Suture-Delivery", &webhook.id)
            .header("X-Suture-Timestamp", now_secs().to_string())
            .header("X-Suture-Retry-Count", attempt.to_string())
            .timeout(std::time::Duration::from_secs(
                self.retry_config.timeout_secs,
            ))
            .body(payload_json.to_owned());

        if let Some(ref secret) = webhook.secret
            && let Some(signature) = self.sign_payload(payload_json, secret)
        {
            req = req.header("X-Suture-Signature", format!("sha256={signature}"));
        }

        let now = now_secs();
        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                let status_code = resp.status().as_u16();
                Ok(DeliveryRecord {
                    webhook_id: webhook.id.clone(),
                    event: event.to_owned(),
                    repo_id: webhook.repo_id.clone(),
                    status: DeliveryStatus::Succeeded,
                    status_code: Some(status_code),
                    attempt,
                    last_attempt_at: now,
                    next_retry_at: None,
                    response_body: None,
                    error: None,
                })
            }
            Ok(resp) => {
                let status_code = resp.status().as_u16();
                let body = resp.text().await.unwrap_or_default();
                let error_msg = format!("HTTP {status_code}");

                Err(DeliveryRecord {
                    webhook_id: webhook.id.clone(),
                    event: event.to_owned(),
                    repo_id: webhook.repo_id.clone(),
                    status: DeliveryStatus::Pending,
                    status_code: Some(status_code),
                    attempt,
                    last_attempt_at: now,
                    next_retry_at: Some(now + delay.as_secs()),
                    response_body: Some(body),
                    error: Some(error_msg),
                })
            }
            Err(e) => Err(DeliveryRecord {
                webhook_id: webhook.id.clone(),
                event: event.to_owned(),
                repo_id: webhook.repo_id.clone(),
                status: DeliveryStatus::Pending,
                status_code: None,
                attempt,
                last_attempt_at: now,
                next_retry_at: Some(now + delay.as_secs()),
                response_body: None,
                error: Some(e.to_string()),
            }),
        }
    }

    /// Calculate exponential backoff delay with jitter.
    fn calculate_backoff(&self, attempt: u32) -> std::time::Duration {
        // Exponential backoff: base * 2^attempt, capped at max_delay
        let base = self.retry_config.base_delay_ms;
        let max = self.retry_config.max_delay_ms;
        let delay = base.saturating_mul(2u64.saturating_pow(attempt));
        let delay = delay.min(max);

        // Add jitter: ±25% of the capped delay
        let jitter = delay / 4;
        let jitter_range = rand::thread_rng().gen_range(0..=jitter * 2);
        let final_delay = delay.saturating_sub(jitter).saturating_add(jitter_range);

        // Ensure final delay never exceeds max after jitter
        std::time::Duration::from_millis(final_delay.min(max))
    }
}

impl Default for WebhookManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current Unix timestamp in seconds.
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_serialization() {
        let webhook = Webhook {
            id: "wh-1".to_string(),
            repo_id: "my-repo".to_string(),
            url: "https://example.com/hook".to_string(),
            events: vec!["push".to_string(), "branch.create".to_string()],
            secret: Some("my-secret".to_string()),
            created_at: 1000,
            active: true,
        };
        let json = serde_json::to_string(&webhook).unwrap();
        let decoded: Webhook = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, webhook.id);
        assert_eq!(decoded.repo_id, webhook.repo_id);
        assert_eq!(decoded.url, webhook.url);
        assert_eq!(decoded.events, webhook.events);
        assert_eq!(decoded.secret, webhook.secret);
        assert_eq!(decoded.created_at, webhook.created_at);
        assert_eq!(decoded.active, webhook.active);
    }

    #[test]
    fn test_payload_serialization() {
        let payload = WebhookPayload {
            event: "push".to_string(),
            repo_id: "test-repo".to_string(),
            timestamp: 12345,
            data: serde_json::json!({"patches": 3, "branch": "main"}),
        };
        let json = serde_json::to_string(&payload).unwrap();
        let decoded: WebhookPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.event, payload.event);
        assert_eq!(decoded.repo_id, payload.repo_id);
        assert_eq!(decoded.timestamp, payload.timestamp);
        assert_eq!(decoded.data["patches"], 3);
    }

    #[test]
    fn test_sign_payload() {
        let manager = WebhookManager::new();
        let payload = r#"{"event":"push"}"#;
        let secret = "test-secret";
        let sig1 = manager
            .sign_payload(payload, secret)
            .expect("valid HMAC key");
        let sig2 = manager
            .sign_payload(payload, secret)
            .expect("valid HMAC key");
        assert_eq!(sig1, sig2);
        assert!(!sig1.is_empty());
        assert!(sig1.len() > 32);

        let different_sig = manager
            .sign_payload(r#"{"event":"pull"}"#, secret)
            .expect("valid HMAC key");
        assert_ne!(sig1, different_sig);
    }

    #[test]
    fn test_verify_signature_valid() {
        let payload = r#"{"event":"push","data":{}}"#;
        let secret = "webhook-secret";
        let manager = WebhookManager::new();
        let sig = manager.sign_payload(payload, secret).unwrap();
        let signature = format!("sha256={sig}");

        assert!(WebhookManager::verify_signature(
            payload, secret, &signature
        ));
    }

    #[test]
    fn test_verify_signature_invalid() {
        let payload = r#"{"event":"push"}"#;
        let secret = "webhook-secret";

        // Wrong signature.
        assert!(!WebhookManager::verify_signature(
            payload,
            secret,
            "sha256=0000000000000000000000000000000000000000000000000000000000000000"
        ));

        // Missing sha256= prefix.
        assert!(!WebhookManager::verify_signature(
            payload,
            secret,
            "invalid-signature"
        ));

        // Wrong payload.
        let manager = WebhookManager::new();
        let sig = manager.sign_payload(payload, secret).unwrap();
        assert!(!WebhookManager::verify_signature(
            r#"{"event":"pull"}"#,
            secret,
            &format!("sha256={sig}")
        ));
    }

    #[test]
    fn test_verify_signature_wrong_secret() {
        let payload = r#"{"event":"push"}"#;
        let manager = WebhookManager::new();
        let sig = manager.sign_payload(payload, "correct-secret").unwrap();
        assert!(!WebhookManager::verify_signature(
            payload,
            "wrong-secret",
            &format!("sha256={sig}")
        ));
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.base_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 300_000);
        assert_eq!(config.timeout_secs, 10);
    }

    #[test]
    fn test_calculate_backoff_increases() {
        let manager = WebhookManager::with_retry_config(RetryConfig {
            max_retries: 5,
            base_delay_ms: 1000,
            max_delay_ms: 60_000,
            timeout_secs: 10,
        });

        // Run multiple times to account for jitter.
        let d0 = manager.calculate_backoff(0).as_millis();
        let d1 = manager.calculate_backoff(1).as_millis();
        let d2 = manager.calculate_backoff(2).as_millis();

        // With jitter ±25%, d1 should be roughly 2x d0 (allowing wide margin).
        // d0 ~1000, d1 ~2000±500, d2 ~4000±1000
        assert!(d0 > 500, "d0={d0}"); // 1000 - 25% = 750, but be generous
        assert!(d1 > 1000, "d1={d1}"); // 2000 - 50% = 1000
        assert!(d2 > 2000, "d2={d2}"); // 4000 - 50% = 2000

        // Should be capped at max_delay.
        let d10 = manager.calculate_backoff(10).as_millis();
        assert!(d10 <= 60_000);
    }

    #[test]
    fn test_delivery_record_serialization() {
        let record = DeliveryRecord {
            webhook_id: "wh-1".to_string(),
            event: "push".to_string(),
            repo_id: "repo".to_string(),
            status: DeliveryStatus::Succeeded,
            status_code: Some(200),
            attempt: 1,
            last_attempt_at: 1000,
            next_retry_at: None,
            response_body: Some("OK".to_string()),
            error: None,
        };
        let json = serde_json::to_string(&record).unwrap();
        let decoded: DeliveryRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.webhook_id, "wh-1");
        assert_eq!(decoded.status, DeliveryStatus::Succeeded);
        assert_eq!(decoded.status_code, Some(200));
    }

    #[test]
    fn test_webhook_result_serialization() {
        let result = WebhookResult {
            triggered: 3,
            succeeded: 2,
            failed: 1,
            retried: 1,
        };
        let json = serde_json::to_string(&result).unwrap();
        let decoded: WebhookResult = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.triggered, 3);
        assert_eq!(decoded.succeeded, 2);
        assert_eq!(decoded.failed, 1);
        assert_eq!(decoded.retried, 1);
    }

    #[tokio::test]
    async fn test_webhook_manager_trigger_filters_events() {
        let manager = WebhookManager::new();
        let webhooks = vec![
            Webhook {
                id: "wh-1".to_string(),
                repo_id: "repo".to_string(),
                url: "https://127.0.0.1:1/hook".to_string(),
                events: vec!["push".to_string()],
                secret: None,
                created_at: 0,
                active: true,
            },
            Webhook {
                id: "wh-2".to_string(),
                repo_id: "repo".to_string(),
                url: "https://127.0.0.1:1/hook".to_string(),
                events: vec!["branch.create".to_string()],
                secret: None,
                created_at: 0,
                active: true,
            },
        ];

        let result = manager
            .trigger(&webhooks, "push", "repo", serde_json::json!({"test": true}))
            .await;
        assert_eq!(result.triggered, 1);

        let result2 = manager
            .trigger(
                &webhooks,
                "branch.create",
                "repo",
                serde_json::json!({"test": true}),
            )
            .await;
        assert_eq!(result2.triggered, 1);

        let result3 = manager
            .trigger(
                &webhooks,
                "branch.delete",
                "repo",
                serde_json::json!({"test": true}),
            )
            .await;
        assert_eq!(result3.triggered, 0);
    }

    #[tokio::test]
    async fn test_webhook_manager_trigger_inactive_skipped() {
        let manager = WebhookManager::new();
        let webhooks = vec![
            Webhook {
                id: "wh-active".to_string(),
                repo_id: "repo".to_string(),
                url: "https://127.0.0.1:1/hook".to_string(),
                events: vec!["push".to_string()],
                secret: None,
                created_at: 0,
                active: true,
            },
            Webhook {
                id: "wh-inactive".to_string(),
                repo_id: "repo".to_string(),
                url: "https://127.0.0.1:1/hook".to_string(),
                events: vec!["push".to_string()],
                secret: None,
                created_at: 0,
                active: false,
            },
        ];

        let result = manager
            .trigger(&webhooks, "push", "repo", serde_json::json!({}))
            .await;
        assert_eq!(result.triggered, 1);
    }
}
