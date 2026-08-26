use std::path::Path;
use std::time::Instant;

use serde_json::json;
use session_core::models::message::{PaginatedMessages, RangeMessages};
use session_core::provider::{claude, codex, grok};

use super::perf;

#[tauri::command]
pub async fn get_messages(
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

    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown")
        .to_string();
    let source_for_parse = source.clone();
    let from_end = from_end.unwrap_or(false);
    let started = Instant::now();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let path = Path::new(&file_path);
        match source_for_parse.as_str() {
            "claude" => claude::parse_session_messages(path, page, page_size, from_end),
            "codex" => codex::parse_session_messages(path, page, page_size, from_end),
            "grok" => grok::parse_session_messages(path, page, page_size, from_end),
            _ => Err(format!("Unknown source: {}", source_for_parse)),
        }
    })
    .await
    .map_err(|error| format!("消息解析任务失败: {error}"))?;

    if perf::enabled() {
        let duration_ms = started.elapsed().as_secs_f64() * 1_000.0;
        match &result {
            Ok(messages) => perf::emit_backend(
                "messages.backend_parse",
                duration_ms,
                json!({
                    "source": source,
                    "fileName": file_name,
                    "page": page,
                    "pageSize": page_size,
                    "fromEnd": from_end,
                    "messages": messages.messages.len(),
                    "total": messages.total,
                    "hasMore": messages.has_more,
                }),
            ),
            Err(error) => perf::emit_backend(
                "messages.backend_error",
                duration_ms,
                json!({
                    "source": source,
                    "fileName": file_name,
                    "page": page,
                    "pageSize": page_size,
                    "fromEnd": from_end,
                    "errorType": if error.is_empty() { "empty" } else { "parse" },
                }),
            ),
        }
    }

    result
}

/// Load `[start, end)` of messages. Used by the progressive (windowed)
/// view to grow the loaded range in either direction without going through
/// the page/from_end gymnastics.
#[tauri::command]
pub async fn get_messages_range(
    source: String,
    file_path: String,
    start: usize,
    end: usize,
) -> Result<RangeMessages, String> {
    let path = Path::new(&file_path);
    if !path.exists() {
        return Err(format!("Session file not found: {}", file_path));
    }

    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown")
        .to_string();
    let source_for_parse = source.clone();
    let started = Instant::now();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let path = Path::new(&file_path);
        match source_for_parse.as_str() {
            "claude" => claude::parse_messages_range(path, start, end),
            "codex" => codex::parse_messages_range(path, start, end),
            "grok" => grok::parse_messages_range(path, start, end),
            _ => Err(format!("Unknown source: {}", source_for_parse)),
        }
    })
    .await
    .map_err(|error| format!("消息区间解析任务失败: {error}"))?;

    if perf::enabled() {
        let duration_ms = started.elapsed().as_secs_f64() * 1_000.0;
        match &result {
            Ok(messages) => perf::emit_backend(
                "messages.range_backend_parse",
                duration_ms,
                json!({
                    "source": source,
                    "fileName": file_name,
                    "requestedStart": start,
                    "requestedEnd": end,
                    "start": messages.start,
                    "end": messages.end,
                    "messages": messages.messages.len(),
                    "total": messages.total,
                }),
            ),
            Err(error) => perf::emit_backend(
                "messages.range_backend_error",
                duration_ms,
                json!({
                    "source": source,
                    "fileName": file_name,
                    "requestedStart": start,
                    "requestedEnd": end,
                    "errorType": if error.is_empty() { "empty" } else { "parse" },
                }),
            ),
        }
    }

    result
}
