//! Claude API provider implementation
//!
//! Implements the CompletionProvider trait for the Claude models.
//! Uses API key authentication

use crate::providers::{
    CompletionChunk, CompletionRequest, CompletionStream, ProviderError,
    completion_chunk::FinishReason, completion_provider::CompletionProvider,
    completion_provider::ModelInfo, provider_configuration::ClaudeConfig,
};
use async_stream::try_stream;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const API_BASE: &str = "https://api.anthropic.com/v1";

const BASE_SYSTEM_PROMPT: &str = r#"You are a code completion assistant. Complete the code at the cursor position marked by <CURSOR>.

If the cursor follows a comment describing code, start your completion with a newline.

CRITICAL: Output ONLY raw code. Do NOT wrap in markdown code fences (```). No explanations. Just the code to insert."#;

/// Build system prompt, optionally including style hints
fn build_system_prompt(style_hints: Option<&str>) -> String {
    match style_hints {
        Some(hints) if !hints.trim().is_empty() => {
            format!(
                "{}\n\nCode style conventions to follow:\n{}",
                BASE_SYSTEM_PROMPT, hints
            )
        }
        _ => BASE_SYSTEM_PROMPT.to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Request/Response Types (Anthropic API format)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u32,
    stream: bool,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop_sequences: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

/// Non-streaming response from Anthropic API
#[derive(Debug, Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    content_type: String,
    text: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Provider Implementation
// ─────────────────────────────────────────────────────────────────────────────

pub struct ClaudeProvider {
    config: ClaudeConfig,
    client: reqwest::Client,
    api_key: String,
}

impl ClaudeProvider {
    /// create a new Claude provider
    pub fn new(config: ClaudeConfig) -> Result<Self, ProviderError> {
        let client = reqwest::Client::new();
        let api_key = config
            .api_key
            .clone()
            .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
            .ok_or_else(|| {
                ProviderError::NotConfigured(
                    "No API key provided. Set api_key in config or ANTHROPIC_API_KEY env var"
                        .to_string(),
                )
            })?;

        Ok(Self {
            config,
            client,
            api_key,
        })
    }

    /// Build the prompt for code completion
    fn build_prompt(request: &CompletionRequest) -> String {
        format!(
            "Complete the following {} code at <CURSOR>:\n\n{}<CURSOR>{}",
            request.language, request.prefix, request.suffix
        )
    }

    /// Strip markdown code fences from model output
    /// Models sometimes wrap code in ```language ... ``` despite instructions not to
    fn strip_markdown_fences(text: &str) -> String {
        let trimmed = text.trim();

        // Check if wrapped in code fences
        if !trimmed.starts_with("```") {
            return text.to_string();
        }

        // Find the end of the opening fence (first newline after ```)
        let after_opening = if let Some(newline_pos) = trimmed.find('\n') {
            &trimmed[newline_pos + 1..]
        } else {
            // No newline found, return as-is
            return text.to_string();
        };

        // Check for and strip closing fence
        let content = if after_opening.trim_end().ends_with("```") {
            let end_pos = after_opening.rfind("```").unwrap();
            &after_opening[..end_pos]
        } else {
            after_opening
        };

        content.to_string()
    }
}

#[async_trait]
impl CompletionProvider for ClaudeProvider {
    fn id(&self) -> &'static str {
        "claude"
    }

    fn display_name(&self) -> &str {
        "Claude (Anthropic)"
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        if self.api_key.is_empty() {
            return Err(ProviderError::AuthRequired);
        }
        Ok(())
    }

    fn models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "claude-sonnet-4-20250514".to_string(),
                display_name: "Claude Sonnet 4".to_string(),
                context_window: 200_000,
                supports_fim: false,
            },
            ModelInfo {
                id: "claude-3-5-haiku-20241022".to_string(),
                display_name: "Claude 3.5 Haiku".to_string(),
                context_window: 200_000,
                supports_fim: false,
            },
            ModelInfo {
                id: "claude-opus-4-20250514".to_string(),
                display_name: "Claude Opus 4".to_string(),
                context_window: 200_000,
                supports_fim: false,
            },
        ]
    }

    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionStream, ProviderError> {
        let prompt = Self::build_prompt(&request);
        let system_prompt = build_system_prompt(request.style_hints.as_deref());

        let api_request = MessagesRequest {
            model: self.config.model.clone(),
            max_tokens: request.max_tokens,
            stream: false,
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt,
            }],
            system: Some(system_prompt),
            stop_sequences: request.stop,
        };

        let response = self
            .client
            .post(format!("{}/messages", API_BASE))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&api_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            return Err(match status.as_u16() {
                401 => ProviderError::AuthFailed(error_text),
                429 => ProviderError::RateLimited {
                    retry_after_ms: None,
                },
                _ => ProviderError::ProviderResponse(format!("{}: {}", status, error_text)),
            });
        }

        let messages_response: MessagesResponse = response.json().await?;

        // Extract text from content blocks
        let raw_text = messages_response
            .content
            .into_iter()
            .filter_map(|block| block.text)
            .collect::<Vec<_>>()
            .join("");

        // Strip markdown fences if the model added them despite instructions
        let text = Self::strip_markdown_fences(&raw_text);

        let finish_reason = match messages_response.stop_reason.as_deref() {
            Some("end_turn") => FinishReason::Stop,
            Some("max_tokens") => FinishReason::Length,
            Some("stop_sequence") => FinishReason::StopSequence,
            _ => FinishReason::Stop,
        };

        let stream = try_stream! {
            yield CompletionChunk {
                text,
                is_final: true,
                finish_reason: Some(finish_reason),
            };
        };

        Ok(Box::pin(stream))
    }
}
