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
use futures::StreamExt;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const API_BASE: &str = "https://api.anthropic.com/v1";

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

/// streaming event from Anthropic API
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StreamEvent {
    MessageStart {
        #[allow(dead_code)]
        message: MessageStart,
    },
    ContentBlockStart {
        #[allow(dead_code)]
        index: u32,
        #[allow(dead_code)]
        content_block: ContentBlock,
    },
    ContentBlockDelta {
        #[allow(dead_code)]
        index: u32,
        delta: ContentDelta,
    },
    ContentBlockStop {
        #[allow(dead_code)]
        index: u32,
    },
    MessageDelta {
        delta: MessageDeltaContent,
    },
    MessageStop,
    Ping,
    Error {
        error: ApiError,
    },
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct MessageStart {
    id: String,
    model: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ContentBlock {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ContentDelta {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    delta_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MessageDeltaContent {
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    #[serde(rename = "type")]
    _error_type: String,
    message: String,
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
      "You are an expert code completion assistant. Complete the code at <CURSOR>.

Language: {}
{}

Code context:
```
{}
<CURSOR>
{}
```

Provide ONLY the code that should replace <CURSOR>. Do not include markdown, explanations, or the surrounding code. Output raw code only.",
      request.language,
      request.file_path.as_deref().map(|p| format!("File: {}", p)).unwrap_or_default(),
      request.prefix,
      request.suffix
    )
    }

    /// parse sse stream
    fn parse_sse_line(line: &str) -> Option<StreamEvent> {
        if let Some(data) = line.strip_prefix("data: ") {
            if data == "[DONE]" {
                return None;
            }
            serde_json::from_str(data).ok()
        } else {
            None
        }
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

        let api_request = MessagesRequest {
      model: self.config.model.clone(),
      max_tokens: request.max_tokens,
      stream: true,
      messages: vec![Message {
        role: "user".to_string(),
        content: prompt,
      }],
      system: Some(
        "You are a code completion assistant. Output only raw code, no markdown or explanations."
          .to_string(),
      ),
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

        let byte_stream = response.bytes_stream();

        let stream = try_stream! {
          let mut buffer = String::new();

          futures::pin_mut!(byte_stream);

          while let Some(chunk_result) = byte_stream.next().await {
            let chunk = chunk_result?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            while let Some(newline_pos) = buffer.find('\n') {
              let line = buffer[..newline_pos].trim().to_string();
              buffer = buffer[newline_pos + 1..].to_string();

              if line.is_empty() {
                continue;
              }

              if let Some(event) = Self::parse_sse_line(&line) {
                match event {
                  StreamEvent::ContentBlockDelta { delta, .. } => {
                    if let Some(text) = delta.text {
                      yield CompletionChunk {
                        text,
                        is_final: false,
                        finish_reason: None,
                      };
                    }
                  }
                  StreamEvent::MessageDelta { delta } => {
                    if let Some(reason) = delta.stop_reason {
                      yield CompletionChunk {
                        text: String::new(),
                        is_final: true,
                        finish_reason: Some(match reason.as_str() {
                          "end_turn" => FinishReason::Stop,
                          "max_tokens" => FinishReason::Length,
                          "stop_sequence" => FinishReason::StopSequence,
                          _ => FinishReason::Stop,
                        }),
                      };
                    }
                  }
                  StreamEvent::MessageStop => {
                    yield CompletionChunk {
                      text: String::new(),
                      is_final: true,
                      finish_reason: Some(FinishReason::Stop),
                    };
                  }
                  StreamEvent::Error { error } => {
                    Err(ProviderError::ProviderResponse(error.message))?;
                  }
                  _ => {}
                }
              }
            }
          }
        };

        Ok(Box::pin(stream))
    }
}
