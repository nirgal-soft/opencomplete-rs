//! Defines the completion chunk

use serde::{Deserialize, Serialize};

/// a chunk of streaming completion
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CompletionChunk {
    /// the text content of this chunk
    pub text: String,
    /// is this the final chunk?
    pub is_final: bool,
    /// optional: finish reason if done
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    StopSequence,
}
