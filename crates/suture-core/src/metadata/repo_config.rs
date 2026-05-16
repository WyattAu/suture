//! Per-repo configuration loaded from `.suture/config` (TOML).
//!
//! Config lookup priority:
//! 1. Environment variables (`SUTURE_<KEY>`)
//! 2. `.suture/config` (repo-level, committed or local)
//! 3. SQLite config table (set via `suture config key=value`)
//! 4. `~/.config/suture/config.toml` (global)
//! 5. Built-in defaults

use serde::Deserialize;
use std::path::Path;

/// Same structure as GlobalConfig for consistent TOML parsing.
/// A repo config file can override any of these settings.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct RepoConfig {
    #[serde(default)]
    pub user: UserSection,
    #[serde(default)]
    pub signing: SigningSection,
    #[serde(default)]
    pub core: CoreSection,
    #[serde(default)]
    pub push: PushSection,
    #[serde(default)]
    pub pull: PullSection,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UserSection {
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SigningSection {
    pub key: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CoreSection {
    pub compression: Option<bool>,
    pub compression_level: Option<i32>,
    pub editor: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PushSection {
    pub auto: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PullSection {
    pub rebase: Option<bool>,
}

impl RepoConfig {
    /// Parse configuration from a TOML string.
    // Public API; reserved for future config parsing
    #[allow(dead_code)]
    pub fn from_str(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    /// Load repo config from `.suture/config` in the given repo root.
    pub fn load(repo_root: &Path) -> Self {
        let path = repo_root.join(".suture").join("config");
        if !path.exists() {
            return Self::default();
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match toml::from_str(&content) {
            Ok(config) => config,
            Err(e) => {
                tracing::warn!(
                    "warning: failed to parse repo config at {}: {}",
                    path.display(),
                    e
                );
                Self::default()
            }
        }
    }

    /// Get a config value by dotted key (e.g., "user.name", "core.editor").
    #[allow(clippy::single_match, clippy::collapsible_match)]
    pub fn get(&self, key: &str) -> Option<String> {
        let parts: Vec<&str> = key.splitn(2, '.').collect();
        if parts.len() == 2 {
            match parts[0] {
                "user" => match parts[1] {
                    "name" => return self.user.name.clone(),
                    "email" => return self.user.email.clone(),
                    _ => {}
                },
                "signing" => match parts[1] {
                    "key" => return self.signing.key.clone(),
                    _ => {}
                },
                "core" => match parts[1] {
                    "compression" => return self.core.compression.map(|v| v.to_string()),
                    "compression_level" => {
                        return self.core.compression_level.map(|v| v.to_string());
                    }
                    "editor" => return self.core.editor.clone(),
                    _ => {}
                },
                "push" => match parts[1] {
                    "auto" => return self.push.auto.map(|v| v.to_string()),
                    _ => {}
                },
                "pull" => match parts[1] {
                    "rebase" => return self.pull.rebase.map(|v| v.to_string()),
                    _ => {}
                },
                _ => {}
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_toml() {
        let toml_str = r#"
[user]
name = "RepoUser"
email = "repo@example.com"

[core]
editor = "vim"
compression = true

[pull]
rebase = true
"#;
        let config: RepoConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.get("user.name"), Some("RepoUser".to_string()));
        assert_eq!(config.get("core.editor"), Some("vim".to_string()));
        assert_eq!(config.get("pull.rebase"), Some("true".to_string()));
        assert!(config.get("signing.key").is_none());
    }

    #[test]
    fn test_parse_empty_returns_defaults() {
        let config: RepoConfig = toml::from_str("").unwrap();
        assert!(config.get("user.name").is_none());
        assert!(config.get("core.editor").is_none());
    }

    #[test]
    fn test_get_dotted_keys() {
        let toml_str = r#"
[core]
editor = "nvim"
compression_level = 9
"#;
        let config: RepoConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.get("core.editor"), Some("nvim".to_string()));
        assert_eq!(config.get("core.compression_level"), Some("9".to_string()));
        assert!(config.get("core.nonexistent").is_none());
    }
}
