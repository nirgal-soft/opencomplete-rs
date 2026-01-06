//! Defines the request structure for a completion request

use super::ContextFile;
use serde::{Deserialize, Serialize};

/// A completion request from the client text editor
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CompletionRequest {
    /// code before the cursor
    pub prefix: String,
    /// code after the cursor
    pub suffix: String,
    /// language identifier
    pub language: String,
    /// optional file path for additional context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// max tokens to generate
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// stop sequences
    #[serde(default)]
    pub stop: Vec<String>,
    /// optional additional context (other open files, imports, etc)
    #[serde(default)]
    pub context: Vec<ContextFile>,
}

fn default_max_tokens() -> u32 {
    256
}
