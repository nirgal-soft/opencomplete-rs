//! Defines the registry for managing multiple providers

use super::completion_provider::CompletionProvider;
use std::sync::Arc;

// ─────────────────────────────────────────────────────────────────────────────
// Provider Registry
// ─────────────────────────────────────────────────────────────────────────────

/// registry for managing multiple providers
pub struct ProviderRegistry {
    providers: Vec<Arc<dyn CompletionProvider>>,
    default_provider: Option<String>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            default_provider: None,
        }
    }

    pub fn register(&mut self, provider: Arc<dyn CompletionProvider>) {
        if self.default_provider.is_none() {
            self.default_provider = Some(provider.id().to_string());
        }
        self.providers.push(provider);
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn CompletionProvider>> {
        self.providers.iter().find(|p| p.id() == id).cloned()
    }

    pub fn get_default(&self) -> Option<Arc<dyn CompletionProvider>> {
        self.default_provider.as_ref().and_then(|id| self.get(id))
    }

    pub fn set_default(&mut self, id: &str) -> bool {
        if self.providers.iter().any(|p| p.id() == id) {
            self.default_provider = Some(id.to_string());
            true
        } else {
            false
        }
    }

    pub fn list(&self) -> Vec<&dyn CompletionProvider> {
        self.providers.iter().map(|p| p.as_ref()).collect()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
