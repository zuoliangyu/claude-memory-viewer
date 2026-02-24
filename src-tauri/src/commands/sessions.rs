use std::fs;

use session_core::models::session::SessionIndexEntry;
use session_core::provider::{claude, codex};

#[tauri::command]
pub fn get_sessions(source: String, project_id: String) -> Result<Vec<SessionIndexEntry>, String> {
    match source.as_str() {
        "claude" => claude::get_sessions(&project_id),
        "codex" => codex::get_sessions(&project_id),
        _ => Err(format!("Unknown source: {}", source)),
    }
}

#[tauri::command]
pub fn delete_session(file_path: String) -> Result<(), String> {
    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err(format!("File not found: {}", file_path));
    }
    fs::remove_file(path).map_err(|e| format!("Failed to delete session: {}", e))
}
