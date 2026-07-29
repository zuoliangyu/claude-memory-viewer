use std::path::Path;

use session_core::models::message::{PaginatedMessages, RangeMessages};
use session_core::provider::{claude, codex, grok};

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
        "grok" => grok::parse_session_messages(path, page, page_size, from_end.unwrap_or(false)),
        _ => Err(format!("Unknown source: {}", source)),
    }
}

/// Load `[start, end)` of messages. Used by the progressive (windowed)
/// view to grow the loaded range in either direction without going through
/// the page/from_end gymnastics.
#[tauri::command]
pub fn get_messages_range(
    source: String,
    file_path: String,
    start: usize,
    end: usize,
) -> Result<RangeMessages, String> {
    let path = Path::new(&file_path);
    if !path.exists() {
        return Err(format!("Session file not found: {}", file_path));
    }

    match source.as_str() {
        "claude" => claude::parse_messages_range(path, start, end),
        "codex" => codex::parse_messages_range(path, start, end),
        "grok" => grok::parse_messages_range(path, start, end),
        _ => Err(format!("Unknown source: {}", source)),
    }
}
