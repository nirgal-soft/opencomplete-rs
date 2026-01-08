//! Provider abstraction layer for LLM backends
//!
//! This module defines the core traits and types that all LLM providers must implement

pub mod claude;
pub mod completion_chunk;
pub mod completion_provider;
pub mod completion_request;
pub mod context_file;
pub mod ollama;
pub mod provider_configuration;
pub mod provider_error;
pub mod provider_registry;

pub use completion_chunk::CompletionChunk;
pub use completion_request::CompletionRequest;
pub use context_file::ContextFile;
use futures::Stream;
pub use provider_configuration::ProviderConfig;
pub use provider_error::ProviderError;
pub use provider_registry::ProviderRegistry;
use std::pin::Pin;

/// streaming response type
pub type CompletionStream =
    Pin<Box<dyn Stream<Item = Result<CompletionChunk, ProviderError>> + Send>>;
