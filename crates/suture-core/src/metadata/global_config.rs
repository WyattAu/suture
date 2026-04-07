use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GlobalConfig {
    #[serde(default)]
    pub user: UserConfig,
    #[serde(default)]
    pub signing: SigningConfig,
    #[serde(default)]
    pub core: CoreConfig,
    #[serde(default)]
    pub push: PushConfig,
    #[serde(default)]
    pub pull: PullConfig,
    #[serde(default)]
    pub extra: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UserConfig {
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SigningConfig {
    pub key: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CoreConfig {
    pub compression: Option<bool>,
    pub compression_level: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PushConfig {
    pub auto: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PullConfig {
    pub rebase: Option<bool>,
}

impl GlobalConfig {
    /// Parse configuration from a TOML string.
    #[allow(dead_code, clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if !path.exists() {
            return Self::default();
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        match toml::from_str(&content) {
            Ok(config) => config,
            Err(e) => {
                tracing::warn!(
                    "warning: failed to parse global config at {}: {}",
                    path.display(),
                    e
                );
                Self::default()
            }
        }
    }

    pub fn config_path() -> PathBuf {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg).join("suture").join("config.toml")
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home)
                .join(".config")
                .join("suture")
                .join("config.toml")
        } else {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("suture")
                .join("config.toml")
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let env_key = format!("SUTURE_{}", key.to_uppercase().replace('.', "_"));
        if let Ok(val) = std::env::var(&env_key) {
            return Some(val);
        }

        let parts: Vec<&str> = key.splitn(2, '.').collect();
        if parts.len() == 2 {
            match parts[0] {
                "user" => match parts[1] {
                    "name" => return self.user.name.clone(),
                    "email" => return self.user.email.clone(),
                    _ => {}
                },
                "signing" => {
                    if parts[1] == "key" {
                        return self.signing.key.clone();
                    }
                }
                "core" => match parts[1] {
                    "compression" => return self.core.compression.map(|v| v.to_string()),
                    "compression_level" => {
                        return self.core.compression_level.map(|v| v.to_string());
                    }
                    _ => {}
                },
                "push" => {
                    if parts[1] == "auto" {
                        return self.push.auto.map(|v| v.to_string());
                    }
                }
                "pull" => {
                    if parts[1] == "rebase" {
                        return self.pull.rebase.map(|v| v.to_string());
                    }
                }
                _ => {}
            }
        }

        self.extra.get(key).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_nonexistent_file_returns_defaults() {
        // Ensure no env vars interfere
        unsafe {
            std::env::remove_var("SUTURE_USER_NAME");
            std::env::remove_var("SUTURE_USER_EMAIL");
        }
        let config = GlobalConfig::load();
        assert!(config.user.name.is_none());
        assert!(config.user.email.is_none());
        assert!(config.get("user.name").is_none());
    }

    #[test]
    fn test_parse_valid_toml() {
        let toml_str = r#"
[user]
name = "Alice"
email = "alice@example.com"

[signing]
key = "default"

[core]
compression = true
compression_level = 3

[push]
auto = false

[pull]
rebase = false
"#;
        let config: GlobalConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.user.name.as_deref(), Some("Alice"));
        assert_eq!(config.user.email.as_deref(), Some("alice@example.com"));
        assert_eq!(config.signing.key.as_deref(), Some("default"));
        assert_eq!(config.core.compression, Some(true));
        assert_eq!(config.core.compression_level, Some(3));
        assert_eq!(config.push.auto, Some(false));
        assert_eq!(config.pull.rebase, Some(false));
    }

    #[test]
    fn test_get_dotted_keys() {
        let toml_str = r#"
[user]
name = "Bob"
email = "bob@example.com"

[core]
compression = true
compression_level = 6
"#;
        let config: GlobalConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.get("user.name"), Some("Bob".to_string()));
        assert_eq!(
            config.get("user.email"),
            Some("bob@example.com".to_string())
        );
        assert_eq!(config.get("core.compression"), Some("true".to_string()));
        assert_eq!(config.get("core.compression_level"), Some("6".to_string()));
        assert!(config.get("core.nonexistent").is_none());
        assert!(config.get("nonexistent.key").is_none());
    }

    #[test]
    fn test_env_var_override() {
        let config = GlobalConfig::default();
        unsafe {
            std::env::set_var("SUTURE_USER_NAME", "EnvUser");
        }
        assert_eq!(config.get("user.name"), Some("EnvUser".to_string()));
        unsafe {
            std::env::remove_var("SUTURE_USER_NAME");
        }
    }

    #[test]
    fn test_config_path() {
        let path = GlobalConfig::config_path();
        assert!(path.to_string_lossy().contains("suture"));
        assert!(path.to_string_lossy().ends_with("config.toml"));
    }

    #[test]
    fn test_parse_partial_toml() {
        let toml_str = r#"
[user]
name = "Charlie"
"#;
        let config: GlobalConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.get("user.name"), Some("Charlie".to_string()));
        assert!(config.get("user.email").is_none());
        assert!(config.get("signing.key").is_none());
    }
}
