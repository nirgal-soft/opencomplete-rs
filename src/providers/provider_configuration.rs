//! Defines the configuration for instantiating providers

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Provider Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// configuration for instantiating providers
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderConfig {
    Claude(ClaudeConfig),
    Ollama(OllamaConfig),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClaudeConfig {
    /// which model to use
    pub model: String,
    /// API key (can also be set via ANTHROPIC_API_KEY env var)
    #[serde(default)]
    pub api_key: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OllamaConfig {
    /// which model to use
    pub model: String,
    /// ollama server URL
    #[serde(default = "default_ollama_url")]
    pub base_url: String,
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}
