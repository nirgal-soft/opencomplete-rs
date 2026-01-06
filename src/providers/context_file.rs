//! Defines context files clients can optionally provide

use serde::{Deserialize, Serialize};

/// additional context from other files
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ContextFile {
    pub path: String,
    pub content: String,
    pub relevance: f32,
}
