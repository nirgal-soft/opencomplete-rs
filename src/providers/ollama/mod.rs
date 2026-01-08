//! Ollama provider implementation (local LLMs)
//!
//! Supports local models via Ollama for offline/private code completion.
//! Uses FIM (Fill-in-the-Middle) format for proper code completion.
//! Ideal for: codellama, deepseek-coder, starcoder, qwen2.5-coder, etc.

use crate::providers::{
    CompletionChunk, CompletionRequest, CompletionStream, ProviderError,
    completion_chunk::FinishReason, completion_provider::CompletionProvider,
    completion_provider::ModelInfo, provider_configuration::OllamaConfig,
};
use async_stream::try_stream;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Ollama API Types (Generate endpoint for FIM)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    raw: bool,
    options: GenerateOptions,
    /// Keep model loaded in memory (e.g., "30m" for 30 minutes)
    keep_alive: String,
}

#[derive(Debug, Serialize)]
struct GenerateOptions {
    num_predict: u32,
    stop: Vec<String>,
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct GenerateResponse {
    response: String,
    #[allow(dead_code)]
    done: bool,
    #[serde(default)]
    done_reason: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// FIM Token Formats for different models
// ─────────────────────────────────────────────────────────────────────────────

struct FimTokens {
    prefix: &'static str,
    suffix: &'static str,
    middle: &'static str,
}

fn get_fim_tokens(model: &str) -> FimTokens {
    let model_lower = model.to_lowercase();

    // Qwen-based models (including Strand which is fine-tuned from Qwen2.5-Coder)
    if model_lower.contains("qwen") || model_lower.contains("strand") {
        FimTokens {
            prefix: "<|fim_prefix|>",
            suffix: "<|fim_suffix|>",
            middle: "<|fim_middle|>",
        }
    } else if model_lower.contains("deepseek") {
        FimTokens {
            prefix: "<｜fim▁begin｜>",
            suffix: "<｜fim▁hole｜>",
            middle: "<｜fim▁end｜>",
        }
    } else if model_lower.contains("starcoder") || model_lower.contains("starcode") {
        FimTokens {
            prefix: "<fim_prefix>",
            suffix: "<fim_suffix>",
            middle: "<fim_middle>",
        }
    } else {
        // CodeLlama and default format
        FimTokens {
            prefix: "<PRE> ",
            suffix: " <SUF>",
            middle: " <MID>",
        }
    }
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

    /// Build a FIM prompt for code completion
    fn build_fim_prompt(model: &str, request: &CompletionRequest) -> String {
        let tokens = get_fim_tokens(model);

        // Build style hint comment if provided
        let style_comment =
            Self::build_style_comment(&request.language, request.style_hints.as_deref());

        // Simple FIM format: <prefix>code before cursor<suffix>code after cursor<middle>
        // The model sees the full context and knows how to continue
        format!(
            "{}{}{}{}{}{}",
            tokens.prefix,
            style_comment,
            request.prefix,
            tokens.suffix,
            request.suffix,
            tokens.middle
        )
    }

    /// Build a comment containing style hints for FIM models
    /// Uses the appropriate comment syntax for the language
    /// Only adds a comment if style_hints are provided
    fn build_style_comment(language: &str, style_hints: Option<&str>) -> String {
        match style_hints {
            Some(hints) if !hints.trim().is_empty() => {
                let comment_syntax = Self::get_comment_syntax(language);
                format!("{} Style: {}\n", comment_syntax, hints)
            }
            _ => String::new(),
        }
    }

    /// Get the single-line comment syntax for a language
    fn get_comment_syntax(language: &str) -> &'static str {
        match language.to_lowercase().as_str() {
            "python" | "ruby" | "perl" | "r" | "shell" | "bash" | "sh" | "zsh" | "yaml"
            | "toml" => "#",
            "lua" | "sql" => "--",
            "html" | "xml" | "svg" => "<!--",
            "css" | "scss" | "less" => "/*",
            "lisp" | "clojure" | "scheme" | "elisp" => ";;",
            "vim" | "vimscript" => "\"",
            "bat" | "batch" | "cmd" => "REM",
            "powershell" | "ps1" => "#",
            "fortran" => "!",
            "matlab" | "octave" => "%",
            // Default to C-style comments (covers: rust, go, js, ts, c, cpp, java, kotlin, swift, etc.)
            _ => "//",
        }
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
        vec![
            ModelInfo {
                id: "qwen3-coder:latest".to_string(),
                display_name: "Qwen 3 Coder".to_string(),
                context_window: 32768,
                supports_fim: true,
            },
            ModelInfo {
                id: "qwen2.5-coder:14b".to_string(),
                display_name: "Qwen 2.5 Coder 14B".to_string(),
                context_window: 32768,
                supports_fim: true,
            },
            ModelInfo {
                id: "qwen2.5-coder:7b".to_string(),
                display_name: "Qwen 2.5 Coder 7B".to_string(),
                context_window: 32768,
                supports_fim: true,
            },
            ModelInfo {
                id: "deepseek-coder:6.7b".to_string(),
                display_name: "DeepSeek Coder 6.7B".to_string(),
                context_window: 16384,
                supports_fim: true,
            },
            ModelInfo {
                id: "codellama:7b-code".to_string(),
                display_name: "CodeLlama 7B".to_string(),
                context_window: 16384,
                supports_fim: true,
            },
            ModelInfo {
                id: "starcoder2:3b".to_string(),
                display_name: "StarCoder2 3B".to_string(),
                context_window: 16384,
                supports_fim: true,
            },
        ]
    }

    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionStream, ProviderError> {
        let prompt = Self::build_fim_prompt(&self.config.model, &request);

        tracing::debug!("FIM prompt: {}", prompt);

        let mut stop_sequences = request.stop;
        let fim_tokens = get_fim_tokens(&self.config.model);
        stop_sequences.push(fim_tokens.prefix.to_string());
        stop_sequences.push(fim_tokens.suffix.to_string());
        stop_sequences.push("<|endoftext|>".to_string());
        stop_sequences.push("<|end|>".to_string());
        stop_sequences.push("<|file_sep|>".to_string());
        stop_sequences.push("<|im_end|>".to_string());
        stop_sequences.push("<|eot_id|>".to_string());
        stop_sequences.push("\n\n".to_string());
        stop_sequences.push("\nfn ".to_string());
        stop_sequences.push("\npub ".to_string());
        stop_sequences.push("\nimpl ".to_string());
        stop_sequences.push("\nstruct ".to_string());
        stop_sequences.push("\nenum ".to_string());
        stop_sequences.push("\nuse ".to_string());
        stop_sequences.push("\nmod ".to_string());
        stop_sequences.push("\n#[".to_string());

        let api_request = GenerateRequest {
            model: self.config.model.clone(),
            prompt,
            stream: false,
            raw: true,
            options: GenerateOptions {
                num_predict: request.max_tokens,
                stop: stop_sequences,
                temperature: 0.2,
            },
            keep_alive: "30m".to_string(), // Keep model loaded for 30 minutes
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

        let gen_response: GenerateResponse = response.json().await?;

        // The model sees the full FIM context including indentation, so it outputs
        // properly formatted code. Return as-is.
        let text = gen_response.response;

        let finish_reason = match gen_response.done_reason.as_deref() {
            Some("stop") => FinishReason::Stop,
            Some("length") => FinishReason::Length,
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
