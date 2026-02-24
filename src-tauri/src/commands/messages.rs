use std::path::Path;

use session_core::models::message::PaginatedMessages;
use session_core::provider::{claude, codex};

#[tauri::command]
pub fn get_messages(
    source: String,
    file_path: String,
    page: usize,
    page_size: usize,
    from_end: Option<bool>,
) -> Result<PaginatedMessages, String> {
    let path = Path::new(&file_path);
    if !path.exists() {
        return Err(format!("Session file not found: {}", file_path));
    }

    match source.as_str() {
        "claude" => claude::parse_session_messages(path, page, page_size, from_end.unwrap_or(false)),
        "codex" => codex::parse_session_messages(path, page, page_size, from_end.unwrap_or(false)),
        _ => Err(format!("Unknown source: {}", source)),
    }
}
