//! Ollama provider implementation (local LLMs)
//!
//! Supports local models via Ollama for offline/private code completion.
//! Ideal for: codellama, deepseek-coder, starcoder, etc.

use crate::providers::{
    CompletionChunk, CompletionRequest, CompletionStream, ProviderError,
    completion_chunk::FinishReason, completion_provider::CompletionProvider,
    completion_provider::ModelInfo, provider_configuration::OllamaConfig,
};
use async_stream::try_stream;
use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Ollama API Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    suffix: Option<String>,
    options: GenerateOptions,
}

#[derive(Debug, Serialize)]
struct GenerateOptions {
    num_predict: u32,
    stop: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GenerateResponse {
    response: String,
    done: bool,
    #[serde(default)]
    done_reason: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Provider Implementation
// ─────────────────────────────────────────────────────────────────────────────

pub struct OllamaProvider {
    config: OllamaConfig,
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(config: OllamaConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Build a FIM-style prompt
    /// Many Ollama models support Fill-in-Middle natively
    fn build_prompt(request: &CompletionRequest) -> (String, Option<String>) {
        // For models that support FIM (codellama, deepseek-coder):
        // Use <PRE>, <SUF>, <MID> tokens
        //
        // For now, we'll use a simple prompt format that works with most models
        let prompt = format!(
            "<|fim_prefix|>{}<|fim_suffix|>{}<|fim_middle|>",
            request.prefix, request.suffix
        );

        (prompt, None)
    }
}

#[async_trait]
impl CompletionProvider for OllamaProvider {
    fn id(&self) -> &'static str {
        "ollama"
    }

    fn display_name(&self) -> &str {
        "Ollama (Local)"
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        let response = self
            .client
            .get(format!("{}/api/version", self.config.base_url))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(ProviderError::Network(
                "Ollama server not reachable".to_string(),
            ))
        }
    }

    fn models(&self) -> Vec<ModelInfo> {
        // In a real implementation, we'd query /api/tags to get available models
        vec![
            ModelInfo {
                id: "codellama:7b-code".to_string(),
                display_name: "CodeLlama 7B".to_string(),
                context_window: 16384,
                supports_fim: true,
            },
            ModelInfo {
                id: "deepseek-coder:6.7b".to_string(),
                display_name: "DeepSeek Coder 6.7B".to_string(),
                context_window: 16384,
                supports_fim: true,
            },
            ModelInfo {
                id: "qwen2.5-coder:7b".to_string(),
                display_name: "Qwen 2.5 Coder 7B".to_string(),
                context_window: 32768,
                supports_fim: true,
            },
        ]
    }

    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionStream, ProviderError> {
        let (prompt, suffix) = Self::build_prompt(&request);

        let api_request = GenerateRequest {
            model: self.config.model.clone(),
            prompt,
            stream: true,
            suffix,
            options: GenerateOptions {
                num_predict: request.max_tokens,
                stop: request.stop,
            },
        };

        let response = self
            .client
            .post(format!("{}/api/generate", self.config.base_url))
            .json(&api_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ProviderError::ProviderResponse(error_text));
        }

        let byte_stream = response.bytes_stream();

        let stream = try_stream! {
          futures::pin_mut!(byte_stream);

          while let Some(chunk_result) = byte_stream.next().await {
            let chunk = chunk_result?;
            let text = String::from_utf8_lossy(&chunk);

            // Ollama sends newline-delimited JSON
            for line in text.lines() {
              if line.is_empty() {
                continue;
              }

              if let Ok(response) = serde_json::from_str::<GenerateResponse>(line) {
                yield CompletionChunk {
                  text: response.response,
                  is_final: response.done,
                  finish_reason: if response.done {
                    Some(match response.done_reason.as_deref() {
                      Some("stop") => FinishReason::Stop,
                      Some("length") => FinishReason::Length,
                      _ => FinishReason::Stop,
                    })
                  } else {
                    None
                  },
                };
              }
            }
          }
        };

        Ok(Box::pin(stream))
    }
}
