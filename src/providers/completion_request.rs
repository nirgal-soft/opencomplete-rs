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
    /// formatting settings from the editor
    #[serde(default)]
    pub formatting: Option<FormattingOptions>,
    /// free-form style hints describing code conventions
    /// e.g. "tabwidth=2, dangling braces, imports alphabetical, no semicolons"
    #[serde(default)]
    pub style_hints: Option<String>,
}

/// Formatting options from the editor
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FormattingOptions {
    /// Whether to use tabs (true) or spaces (false)
    #[serde(default)]
    pub use_tabs: bool,
    /// Number of spaces per indent level (or tab width for display)
    #[serde(default = "default_indent_size")]
    pub indent_size: u32,
}

fn default_max_tokens() -> u32 {
    256
}

fn default_indent_size() -> u32 {
    2
}
