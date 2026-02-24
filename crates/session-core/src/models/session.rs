use serde::{Deserialize, Serialize};

/// The sessions-index.json file structure (Claude only)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionsIndex {
    pub version: Option<u32>,
    pub entries: Vec<SessionsIndexFileEntry>,
    pub original_path: Option<String>,
}

/// Raw entry from sessions-index.json (Claude internal format)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionsIndexFileEntry {
    pub session_id: String,
    pub full_path: Option<String>,
    pub file_mtime: Option<u64>,
    pub first_prompt: Option<String>,
    pub message_count: Option<u32>,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub git_branch: Option<String>,
    pub project_path: Option<String>,
    pub is_sidechain: Option<bool>,
}

/// Unified session entry returned to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionIndexEntry {
    /// "claude" or "codex"
    pub source: String,
    pub session_id: String,
    /// Full file path (both sources need this)
    pub file_path: String,
    pub first_prompt: Option<String>,
    pub message_count: u32,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub git_branch: Option<String>,
    pub project_path: Option<String>,
    // Claude-specific
    pub is_sidechain: Option<bool>,
    // Codex-specific
    pub cwd: Option<String>,
    pub model_provider: Option<String>,
    pub cli_version: Option<String>,
}
