use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEntry {
    /// "claude" or "codex"
    pub source: String,
    /// Claude: encoded_name, Codex: cwd
    pub id: String,
    /// Full display path
    pub display_path: String,
    /// Last path segment
    pub short_name: String,
    /// Number of session files
    pub session_count: usize,
    /// Last modified time (ISO 8601)
    pub last_modified: Option<String>,
    /// Codex: model provider (e.g. "openai")
    pub model_provider: Option<String>,
}
