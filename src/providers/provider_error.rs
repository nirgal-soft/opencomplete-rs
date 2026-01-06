//! Defines the error types for providers

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Authentication required")]
    AuthRequired,
    #[error("Authentication failed: {0}")]
    AuthFailed(String),
    #[error("Token expired")]
    #[allow(dead_code)]
    TokenExpired,
    #[error("Rate limited, retry after {retry_after_ms:?}ms")]
    RateLimited { retry_after_ms: Option<u64> },
    #[error("Network error: {0}")]
    Network(String),
    #[error("Provider returned error: {0}")]
    ProviderResponse(String),
    #[error("Invalid response format: {0}")]
    #[allow(dead_code)]
    InvalidResponse(String),
    #[error("Provider not configured: {0}")]
    NotConfigured(String),
}

impl From<reqwest::Error> for ProviderError {
    fn from(err: reqwest::Error) -> Self {
        ProviderError::Network(err.to_string())
    }
}
