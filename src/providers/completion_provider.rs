//! Defines the core traits and types that all LLM providers must implement

use super::CompletionStream;
use super::completion_request::CompletionRequest;
use super::provider_error::ProviderError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Provider Trait
// ─────────────────────────────────────────────────────────────────────────────

/// the core traite that all LLM providers must implement
#[async_trait]
pub trait CompletionProvider: Send + Sync {
    /// a unique identifier for this provider (e.g. "claude", "ollama")
    fn id(&self) -> &'static str;
    /// human-readable name (e.g. "Claude (Anthropic)")
    fn display_name(&self) -> &str;
    /// is this provider ready to use?
    #[allow(dead_code)]
    async fn health_check(&self) -> Result<(), ProviderError>;
    /// get supported model IDs
    #[allow(dead_code)]
    fn models(&self) -> Vec<ModelInfo>;
    /// stream a completion request
    async fn complete(&self, request: CompletionRequest)
    -> Result<CompletionStream, ProviderError>;
}

/// information about a model
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub context_window: u32,
    pub supports_fim: bool,
}
