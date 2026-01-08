//! Configuration management
//!
//! Loads and manages configuration from:
//! - Config file (~/.config/opencomplete/config.toml)
//! - Environment variables (ANTHROPIC_API_KEY)

use crate::providers::ProviderConfig;
use crate::providers::provider_configuration::{ClaudeConfig, OllamaConfig};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// Configuration Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Server settings
    #[serde(default)]
    pub server: ServerConfig,

    /// Default provider to use
    #[serde(default)]
    pub default_provider: Option<String>,

    /// Provider configurations
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Port to listen on
    #[serde(default = "default_port")]
    pub port: u16,

    /// Host to bind to
    #[serde(default = "default_host")]
    pub host: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            host: default_host(),
        }
    }
}

fn default_port() -> u16 {
    8642
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            default_provider: Some("claude".to_string()),
            providers: vec![
                ProviderConfig::Claude(ClaudeConfig {
                    model: "claude-sonnet-4-20250514".to_string(),
                    api_key: None, // Uses ANTHROPIC_API_KEY env var
                }),
                ProviderConfig::Ollama(OllamaConfig {
                    model: "qwen2.5-coder:latest".to_string(),
                    base_url: "http://localhost:11434".to_string(),
                }),
            ],
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Config Loading
// ─────────────────────────────────────────────────────────────────────────────

impl Config {
    /// Load config from the default location
    pub fn load() -> anyhow::Result<Self> {
        let config_path = Self::default_path()?;

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            // Create default config
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Save config to the default location
    pub fn save(&self) -> anyhow::Result<()> {
        let config_path = Self::default_path()?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;

        Ok(())
    }

    /// Get the default config path
    fn default_path() -> anyhow::Result<PathBuf> {
        let home = std::env::var("HOME")
            .map_err(|_| anyhow::anyhow!("Could not determine home directory"))?;

        Ok(PathBuf::from(home).join(".config/opencomplete/config.toml"))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Example Config
// ─────────────────────────────────────────────────────────────────────────────

/// Generate an example config file
pub fn example_config() -> String {
    r#"# opencomplete configuration

[server]
port = 8642
host = "127.0.0.1"

# Default provider to use
default_provider = "ollama"

# Ollama provider (local models - free!)
[[providers]]
type = "ollama"
model = "qwen3-coder:latest"
base_url = "http://localhost:11434"

# Claude API provider (requires API key)
# Uncomment to enable:
# [[providers]]
# type = "claude"
# model = "claude-sonnet-4-20250514"
# api_key = "sk-ant-..."  # Or use ANTHROPIC_API_KEY env var
"#
    .to_string()
}
