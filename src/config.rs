//! Configuration management
//!
//! Loads and manages configuration from:
//! - Config file (~/.config/nvim-supercomplete/config.toml)
//! - Environment variables (ANTHROPIC_API_KEY)

use crate::providers::{ClaudeConfig, ProviderConfig};
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
            providers: vec![ProviderConfig::Claude(ClaudeConfig {
                model: "claude-sonnet-4-20250514".to_string(),
                api_key: None, // Will use ANTHROPIC_API_KEY env var
            })],
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
        let dirs =
            directories::ProjectDirs::from("com", "nvim-supercomplete", "nvim-supercomplete")
                .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;

        Ok(dirs.config_dir().join("config.toml"))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Example Config
// ─────────────────────────────────────────────────────────────────────────────

/// Generate an example config file
pub fn example_config() -> String {
    r#"# nvim-supercomplete configuration

[server]
port = 8642
host = "127.0.0.1"

# Default provider to use
default_provider = "claude"

# Claude provider (API key)
# Set ANTHROPIC_API_KEY env var, or uncomment api_key below
[[providers]]
type = "claude"
model = "claude-sonnet-4-20250514"
# api_key = "sk-ant-..."  # Or use ANTHROPIC_API_KEY env var

# Ollama provider (local models - free!)
# Uncomment to enable:
# [[providers]]
# type = "ollama"
# model = "qwen2.5-coder:7b"
# base_url = "http://localhost:11434"
"#
    .to_string()
}
