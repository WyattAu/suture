use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    pub event: String,
    pub repo_id: String,
    pub timestamp: u64,
    pub data: serde_json::Value,
}

#[derive(Debug)]
pub struct WebhookResult {
    pub triggered: usize,
    pub succeeded: usize,
    pub failed: usize,
}

pub struct WebhookManager {
    client: reqwest::Client,
}

impl WebhookManager {
    #[must_use] 
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

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
                };
            };

        let matching: Vec<&Webhook> = webhooks
            .iter()
            .filter(|w| w.active && w.events.iter().any(|e| e == event))
            .collect();

        let triggered = matching.len();
        let mut succeeded = 0usize;
        let mut failed = 0usize;

        for webhook in matching {
            let mut req = self
                .client
                .post(&webhook.url)
                .header("Content-Type", "application/json")
                .header("X-Suture-Event", event)
                .header("X-Suture-Delivery", &webhook.id)
                .body(payload_json.clone());

            if let Some(ref secret) = webhook.secret
                && let Some(signature) = self.sign_payload(&payload_json, secret)
            {
                req = req.header("X-Suture-Signature", format!("sha256={signature}"));
            }

            match req.send().await {
                Ok(resp) if resp.status().is_success() => succeeded += 1,
                _ => failed += 1,
            }
        }

        WebhookResult {
            triggered,
            succeeded,
            failed,
        }
    }
}

impl Default for WebhookManager {
    fn default() -> Self {
        Self::new()
    }
}

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
        let sig1 = manager.sign_payload(payload, secret).expect("valid HMAC key");
        let sig2 = manager.sign_payload(payload, secret).expect("valid HMAC key");
        assert_eq!(sig1, sig2);
        assert!(!sig1.is_empty());
        assert!(sig1.len() > 32);

        let different_sig = manager.sign_payload(r#"{"event":"pull"}"#, secret).expect("valid HMAC key");
        assert_ne!(sig1, different_sig);
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
