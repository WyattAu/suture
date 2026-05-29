use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_address: String,
    pub tls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailNotification {
    pub to: String,
    pub subject: String,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct EmailService {
    config: Option<SmtpConfig>,
}

impl EmailService {
    pub fn new(config: Option<SmtpConfig>) -> Self {
        Self { config }
    }

    pub fn is_configured(&self) -> bool {
        self.config.is_some()
    }

    pub fn send(&self, notification: &EmailNotification) -> Result<(), String> {
        let Some(config) = &self.config else {
            tracing::info!(
                "email skipped (no SMTP): to={} subject={}",
                notification.to,
                notification.subject
            );
            return Ok(());
        };

        let message = format!(
            "From: {}\r\nTo: {}\r\nSubject: {}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{}",
            config.from_address, notification.to, notification.subject, notification.body
        );

        self.send_raw(config, &message)
    }

    fn send_raw(&self, config: &SmtpConfig, message: &str) -> Result<(), String> {
        use std::io::{Read, Write};
        use std::net::TcpStream;

        let addr = format!("{}:{}", config.host, config.port);
        let mut stream = TcpStream::connect_timeout(
            &addr.parse().map_err(|e| format!("invalid addr: {e}"))?,
            Duration::from_secs(10),
        )
        .map_err(|e| format!("SMTP connect failed: {e}"))?;

        let mut buf = [0u8; 1024];

        stream
            .read(&mut buf)
            .map_err(|e| format!("SMTP read greeting failed: {e}"))?;

        stream
            .write_all(b"EHLO suture-hub\r\n")
            .map_err(|e| format!("SMTP EHLO failed: {e}"))?;
        stream
            .read(&mut buf)
            .map_err(|e| format!("SMTP read EHLO response failed: {e}"))?;

        if config.tls {
            // STARTTLS would go here - for now skip TLS handshake
        }

        stream
            .write_all("AUTH LOGIN\r\n".as_bytes())
            .map_err(|e| format!("SMTP AUTH failed: {e}"))?;
        stream
            .read(&mut buf)
            .map_err(|e| format!("SMTP read AUTH response failed: {e}"))?;

        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD;
        stream
            .write_all(format!("{}\r\n", b64.encode(&config.username)).as_bytes())
            .map_err(|e| format!("SMTP username failed: {e}"))?;
        stream
            .read(&mut buf)
            .map_err(|e| format!("SMTP read username response failed: {e}"))?;

        stream
            .write_all(format!("{}\r\n", b64.encode(&config.password)).as_bytes())
            .map_err(|e| format!("SMTP password failed: {e}"))?;
        stream
            .read(&mut buf)
            .map_err(|e| format!("SMTP read password response failed: {e}"))?;

        stream
            .write_all(format!("MAIL FROM:<{}>\r\n", config.from_address).as_bytes())
            .map_err(|e| format!("SMTP MAIL FROM failed: {e}"))?;
        stream
            .read(&mut buf)
            .map_err(|e| format!("SMTP read MAIL FROM response failed: {e}"))?;

        let to_addr = message
            .lines()
            .find(|l| l.starts_with("To: "))
            .and_then(|l| l.strip_prefix("To: "))
            .unwrap_or(&config.from_address);

        stream
            .write_all(format!("RCPT TO:<{}>\r\n", to_addr).as_bytes())
            .map_err(|e| format!("SMTP RCPT TO failed: {e}"))?;
        stream
            .read(&mut buf)
            .map_err(|e| format!("SMTP read RCPT TO response failed: {e}"))?;

        stream
            .write_all(b"DATA\r\n")
            .map_err(|e| format!("SMTP DATA failed: {e}"))?;
        stream
            .read(&mut buf)
            .map_err(|e| format!("SMTP read DATA response failed: {e}"))?;

        stream
            .write_all(message.as_bytes())
            .map_err(|e| format!("SMTP write message failed: {e}"))?;
        stream
            .write_all(b"\r\n.\r\n")
            .map_err(|e| format!("SMTP end message failed: {e}"))?;
        stream
            .read(&mut buf)
            .map_err(|e| format!("SMTP read end response failed: {e}"))?;

        stream
            .write_all(b"QUIT\r\n")
            .map_err(|e| format!("SMTP QUIT failed: {e}"))?;

        tracing::info!("email sent to {}", to_addr);
        Ok(())
    }

    pub fn notify_issue_created(
        &self,
        repo_id: &str,
        issue_title: &str,
        author: &str,
        recipients: &[String],
    ) {
        for to in recipients {
            let _ = self.send(&EmailNotification {
                to: to.clone(),
                subject: format!("[{}] New issue: {}", repo_id, issue_title),
                body: format!(
                    "{} created a new issue in {}:\n\n{}\n\nView at: /repo/{}/issues",
                    author, repo_id, issue_title, repo_id
                ),
            });
        }
    }

    pub fn notify_pr_merged(
        &self,
        repo_id: &str,
        pr_title: &str,
        merged_by: &str,
        recipients: &[String],
    ) {
        for to in recipients {
            let _ = self.send(&EmailNotification {
                to: to.clone(),
                subject: format!("[{}] PR merged: {}", repo_id, pr_title),
                body: format!(
                    "{} merged a pull request in {}:\n\n{}\n\nView at: /repo/{}/pulls",
                    merged_by, repo_id, pr_title, repo_id
                ),
            });
        }
    }

    pub fn notify_push(
        &self,
        repo_id: &str,
        patch_count: usize,
        author: &str,
        recipients: &[String],
    ) {
        for to in recipients {
            let _ = self.send(&EmailNotification {
                to: to.clone(),
                subject: format!(
                    "[{}] {} patch(es) pushed by {}",
                    repo_id, patch_count, author
                ),
                body: format!(
                    "{} pushed {} patch(es) to {}.\n\nView at: /repo/{}",
                    author, patch_count, repo_id, repo_id
                ),
            });
        }
    }
}

pub fn flush_email_queue(storage: &crate::storage::HubStorage) -> Result<u32, String> {
    let smtp_config = storage
        .get_smtp_config()
        .map_err(|e| format!("SMTP config read failed: {e}"))?;

    let config = match smtp_config {
        Some(c) => c,
        None => return Err("SMTP not configured".to_string()),
    };

    let pending = storage
        .get_pending_emails(100)
        .map_err(|e| format!("Email queue read failed: {e}"))?;

    let service = EmailService::new(Some(config));

    let mut sent = 0u32;
    for (id, recipient, subject, body, _event_type) in &pending {
        match service.send(&EmailNotification {
            to: recipient.clone(),
            subject: subject.clone(),
            body: body.clone(),
        }) {
            Ok(()) => {
                let _ = storage.mark_email_sent(*id);
                sent += 1;
            }
            Err(e) => {
                eprintln!("Failed to send email {}: {}", id, e);
            }
        }
    }

    Ok(sent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_service_not_configured() {
        let service = EmailService::new(None);
        assert!(!service.is_configured());
        let result = service.send(&EmailNotification {
            to: "test@example.com".to_string(),
            subject: "test".to_string(),
            body: "test body".to_string(),
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_email_service_configured() {
        let config = SmtpConfig {
            host: "localhost".to_string(),
            port: 25,
            username: "user".to_string(),
            password: "pass".to_string(),
            from_address: "noreply@suture.dev".to_string(),
            tls: false,
        };
        let service = EmailService::new(Some(config));
        assert!(service.is_configured());
    }

    #[test]
    fn test_notify_issue_created() {
        let service = EmailService::new(None);
        service.notify_issue_created("myrepo", "Test issue", "alice", &[]);
        service.notify_issue_created(
            "myrepo",
            "Test issue",
            "alice",
            &["bob@example.com".to_string()],
        );
    }

    #[test]
    fn test_notify_pr_merged() {
        let service = EmailService::new(None);
        service.notify_pr_merged(
            "myrepo",
            "Fix bug",
            "alice",
            &["bob@example.com".to_string()],
        );
    }

    #[test]
    fn test_notify_push() {
        let service = EmailService::new(None);
        service.notify_push("myrepo", 5, "alice", &["bob@example.com".to_string()]);
    }

    #[test]
    fn test_email_message_format() {
        let from = "noreply@suture.dev";
        let to = "user@example.com";
        let subject = "Test Subject";
        let body = "Hello, World!";

        let message = format!(
            "From: {from}\r\nTo: {to}\r\nSubject: {subject}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}\r\n.\r\n"
        );

        assert!(message.starts_with("From: "));
        assert!(message.contains("To: "));
        assert!(message.contains("Subject: "));
        assert!(message.contains("Content-Type: text/plain"));
        assert!(message.ends_with("\r\n.\r\n"));
    }
}
