use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use crate::models::message::{
    ContentBlock, ContentValue, DisplayContentBlock, DisplayMessage, PaginatedMessages, RawRecord,
};
use crate::models::session::{SessionsIndex, SessionsIndexFileEntry};
use crate::state::{
    get_cached_full_messages, get_cached_page, paginate_from_range, store_full_messages,
    store_partial_messages, tail_window_len,
};

/// Types of records to skip during parsing (large/irrelevant)
const SKIP_TYPES: &[&str] = &["file-history-snapshot", "progress"];

/// Parse a JSONL session file and return paginated display messages.
/// Uses line-level pre-filtering to skip irrelevant record types.
pub fn parse_session_messages(
    path: &Path,
    page: usize,
    page_size: usize,
    from_end: bool,
) -> Result<PaginatedMessages, String> {
    if let Ok(Some(cached)) = get_cached_page(path, page, page_size, from_end) {
        return Ok(cached);
    }

    if from_end {
        return parse_tail_messages(path, page, page_size);
    }

    let all_messages = parse_all_messages(path)?;
    let total = all_messages.len();
    let start = page * page_size;
    let end = (start + page_size).min(total);
    let has_more = end < total;

    let page_messages = if start < total {
        all_messages[start..end].to_vec()
    } else {
        Vec::new()
    };

    Ok(PaginatedMessages {
        messages: page_messages,
        total,
        page,
        page_size,
        has_more,
    })
}

fn parse_tail_messages(
    path: &Path,
    page: usize,
    page_size: usize,
) -> Result<PaginatedMessages, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let reader = BufReader::new(file);
    let window_len = tail_window_len(page, page_size);
    // Keep only the tail window as *raw* records. We defer the expensive
    // `convert_content` (to_string_pretty of tool inputs, big tool-result
    // string joins) to the very end, so all but the last `window_len`
    // records skip that work entirely — the common case when opening a long
    // session and only needing the most recent messages.
    let mut tail_records: VecDeque<RawRecord> = VecDeque::with_capacity(window_len);
    let mut total = 0usize;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Line-level pre-filter: skip known large/irrelevant record types
        if SKIP_TYPES
            .iter()
            .any(|t| trimmed.contains(&format!("\"type\":\"{}\"", t)))
        {
            continue;
        }

        // Cheap gate: only user/assistant records can ever become a displayed
        // message, so skip the (relatively costly) `from_str` for everything
        // else. Mirrors the substring fast-path used in `scan_session_file_once`.
        if !trimmed.contains("\"type\":\"user\"") && !trimmed.contains("\"type\":\"assistant\"") {
            continue;
        }

        let record: RawRecord = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // `record_is_displayable` exactly mirrors the Some/None decision of
        // `display_message_from_record` (without building any strings), so
        // `total` stays identical to a full parse.
        if !record_is_displayable(&record) {
            continue;
        }

        total += 1;
        if tail_records.len() == window_len {
            tail_records.pop_front();
        }
        tail_records.push_back(record);
    }

    // Convert only the retained tail window.
    let messages: Vec<DisplayMessage> = tail_records
        .into_iter()
        .filter_map(display_message_from_record)
        .collect();
    let range_start = total.saturating_sub(messages.len());
    let _ = store_partial_messages(path, total, range_start, &messages);

    paginate_from_range(&messages, total, page, page_size, true, range_start)
        .ok_or_else(|| "Failed to paginate tail messages".to_string())
}

/// Whether a raw record would yield a displayed message — i.e. exactly when
/// `display_message_from_record` returns `Some`. Kept in sync with that
/// function and `convert_content`, but does **no** string allocation, so it's
/// cheap enough to run over every line while counting `total`.
fn record_is_displayable(record: &RawRecord) -> bool {
    if record.record_type != "user" && record.record_type != "assistant" {
        return false;
    }
    match &record.message {
        Some(msg) => content_produces_block(&msg.content),
        None => false,
    }
}

/// Mirror of `convert_content`'s emptiness rule: returns true iff conversion
/// would produce at least one display block.
fn content_produces_block(content: &ContentValue) -> bool {
    match content {
        ContentValue::Text(s) => !s.trim().is_empty(),
        ContentValue::Blocks(blocks) => blocks.iter().any(|block| match block {
            ContentBlock::Text { text } => !text.trim().is_empty(),
            ContentBlock::Thinking { thinking } => !thinking.trim().is_empty(),
            ContentBlock::ToolUse { .. } => true,
            ContentBlock::ToolResult { .. } => true,
            ContentBlock::Unknown => false,
        }),
    }
}

/// Load a half-open `[start, end)` slice of messages. Used for the
/// progressive / windowed view in `MessagesPage` so the frontend can grow
/// its loaded window in either direction without going through the
/// page/from_end gymnastics. Falls back to a full parse when the range
/// can't be served from the existing partial cache; result is then
/// memoized via `store_full_messages`.
pub fn parse_messages_range(
    path: &Path,
    start: usize,
    end: usize,
) -> Result<crate::models::message::RangeMessages, String> {
    if let Ok(Some((slice, total))) = crate::state::get_cached_range(path, start, end) {
        let actual_end = (start + slice.len()).min(total);
        return Ok(crate::models::message::RangeMessages {
            messages: slice,
            total,
            start,
            end: actual_end,
        });
    }

    let all_messages = parse_all_messages(path)?;
    let total = all_messages.len();
    let _ = store_full_messages(path, &all_messages);

    let clamped_start = start.min(total);
    let clamped_end = end.min(total);
    let slice = if clamped_end > clamped_start {
        all_messages[clamped_start..clamped_end].to_vec()
    } else {
        Vec::new()
    };

    Ok(crate::models::message::RangeMessages {
        messages: slice,
        total,
        start: clamped_start,
        end: clamped_end,
    })
}

/// Parse all messages from a JSONL file (no pagination, for search)
pub fn parse_all_messages(path: &Path) -> Result<Vec<DisplayMessage>, String> {
    if let Ok(Some(cached)) = get_cached_full_messages(path) {
        return Ok(cached);
    }

    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let reader = BufReader::new(file);
    let mut messages: Vec<DisplayMessage> = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if SKIP_TYPES
            .iter()
            .any(|t| trimmed.contains(&format!("\"type\":\"{}\"", t)))
        {
            continue;
        }

        let record: RawRecord = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(_) => continue,
        };

        if let Some(message) = display_message_from_record(record) {
            messages.push(message);
        }
    }

    let _ = store_full_messages(path, &messages);
    Ok(messages)
}

fn display_message_from_record(record: RawRecord) -> Option<DisplayMessage> {
    if record.record_type != "user" && record.record_type != "assistant" {
        return None;
    }

    let msg = record.message?;
    let display_blocks = convert_content(&msg.content);

    if display_blocks.is_empty() {
        return None;
    }

    let role = if msg.role == "user"
        && display_blocks
            .iter()
            .all(|block| matches!(block, DisplayContentBlock::ToolResult { .. }))
    {
        "tool".to_string()
    } else {
        msg.role
    };

    Some(DisplayMessage {
        uuid: record.uuid,
        parent_uuid: record.parent_uuid,
        role,
        timestamp: record.timestamp,
        model: msg.model,
        content: display_blocks,
    })
}

/// Extract the custom title set by CC `/rename` from a JSONL file.
/// Returns the LAST non-empty `customTitle` found.
/// If the last `custom-title` record has an empty `customTitle`, returns None
/// (this represents a "clear alias" intent written by the app).
pub fn extract_custom_title(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut last_title: Option<String> = None; // None means "no record yet" or "cleared"

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.contains("\"type\":\"custom-title\"") {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if value.get("type").and_then(|v| v.as_str()) == Some("custom-title") {
                let title = value
                    .get("customTitle")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if title.trim().is_empty() {
                    // Empty = "clear" intent; reset to None
                    last_title = None;
                } else {
                    last_title = Some(title.trim().to_string());
                }
            }
        }
    }

    last_title
}

/// Append a `custom-title` record to a JSONL file.
/// Matches the format written by CC `/rename`.
///
/// - `title = Some("name")` → appends `{"type":"custom-title","customTitle":"name","sessionId":"..."}`
/// - `title = None` or `title = Some("")` → appends `{"type":"custom-title","customTitle":"","sessionId":"..."}`
///   (empty string acts as "clear" signal; `extract_custom_title` will return None next read)
pub fn append_custom_title(
    path: &Path,
    session_id: &str,
    title: Option<&str>,
) -> Result<(), String> {
    use std::io::Write as IoWrite;

    let title_str = title.unwrap_or("").trim();

    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(|e| format!("Failed to open JSONL for append: {}", e))?;

    let record = serde_json::json!({
        "type": "custom-title",
        "customTitle": title_str,
        "sessionId": session_id,
    });
    writeln!(file, "{}", record)
        .map_err(|e| format!("Failed to write custom-title record: {}", e))?;
    Ok(())
}

/// Extract the first user prompt from a JSONL file
pub fn extract_first_prompt(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.contains("\"type\":\"user\"") {
            continue;
        }

        let record: RawRecord = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(_) => continue,
        };

        if record.record_type == "user" {
            if let Some(msg) = &record.message {
                if msg.role == "user" {
                    match &msg.content {
                        ContentValue::Text(s) => {
                            if !s.is_empty() {
                                return Some(truncate_string(s, 200));
                            }
                        }
                        ContentValue::Blocks(blocks) => {
                            for block in blocks {
                                if let ContentBlock::Text { text } = block {
                                    if !text.is_empty() {
                                        return Some(truncate_string(text, 200));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Extract session metadata (session_id, git_branch, etc.) from the first few lines
pub fn extract_session_metadata(path: &Path) -> Option<(String, Option<String>, Option<String>)> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines().take(10) {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let record: RawRecord = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(_) => continue,
        };

        if let Some(session_id) = record.session_id {
            return Some((session_id, record.git_branch, record.cwd));
        }
    }
    None
}

fn convert_content(content: &ContentValue) -> Vec<DisplayContentBlock> {
    match content {
        ContentValue::Text(s) => {
            if s.trim().is_empty() {
                Vec::new()
            } else {
                vec![DisplayContentBlock::Text { text: s.clone() }]
            }
        }
        ContentValue::Blocks(blocks) => {
            let mut result = Vec::new();
            for block in blocks {
                match block {
                    ContentBlock::Text { text } => {
                        if !text.trim().is_empty() {
                            result.push(DisplayContentBlock::Text { text: text.clone() });
                        }
                    }
                    ContentBlock::Thinking { thinking } => {
                        if !thinking.trim().is_empty() {
                            result.push(DisplayContentBlock::Thinking {
                                thinking: thinking.clone(),
                            });
                        }
                    }
                    ContentBlock::ToolUse { id, name, input } => {
                        let input_str = serde_json::to_string_pretty(input)
                            .unwrap_or_else(|_| input.to_string());
                        result.push(DisplayContentBlock::ToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input: input_str,
                        });
                    }
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        let content_str = match content {
                            Some(v) => match v {
                                serde_json::Value::String(s) => s.clone(),
                                serde_json::Value::Array(arr) => {
                                    // tool_result content can be an array of content blocks
                                    let mut parts = Vec::new();
                                    for item in arr {
                                        if let Some(text) =
                                            item.get("text").and_then(|t| t.as_str())
                                        {
                                            parts.push(text.to_string());
                                        }
                                    }
                                    parts.join("\n")
                                }
                                _ => serde_json::to_string_pretty(v)
                                    .unwrap_or_else(|_| v.to_string()),
                            },
                            None => String::new(),
                        };
                        result.push(DisplayContentBlock::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            content: content_str,
                            is_error: is_error.unwrap_or(false),
                        });
                    }
                    ContentBlock::Unknown => {}
                }
            }
            result
        }
    }
}

// ── Single-pass session file scanner ──

/// Result of a single-pass scan of a JSONL session file.
#[derive(Debug)]
pub struct SessionFileScan {
    pub first_prompt: Option<String>,
    pub custom_title: Option<String>,
    pub git_branch: Option<String>,
    pub project_path: Option<String>,
    pub message_count: u32,
    pub has_messages: bool,
    /// `true` only when a **non-last** line fails JSON parsing.
    /// A truncated final line (common after SIGKILL) is silently ignored.
    pub is_corrupt: bool,
}

/// Scan a JSONL session file in a single pass, extracting all metadata needed
/// for session list display.
///
/// Returns `None` only if the file cannot be opened.
pub fn scan_session_file_once(path: &Path) -> Option<SessionFileScan> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut scan = SessionFileScan {
        first_prompt: None,
        custom_title: None,
        git_branch: None,
        project_path: None,
        message_count: 0,
        has_messages: false,
        is_corrupt: false,
    };

    let mut metadata_done = false;

    // Sliding-window approach: hold `prev` so we always know if the line
    // being processed is the last one (to avoid marking truncated tails as Corrupt).
    let mut prev: Option<String> = None;

    let process =
        |trimmed: &str, is_last: bool, scan: &mut SessionFileScan, metadata_done: &mut bool| {
            if trimmed.is_empty() {
                return;
            }

            let want_meta = !*metadata_done;
            let want_user = trimmed.contains("\"type\":\"user\"");
            let want_assistant = trimmed.contains("\"type\":\"assistant\"");
            let want_title = trimmed.contains("\"type\":\"custom-title\"");

            if !want_meta && !want_user && !want_assistant && !want_title {
                return;
            }

            let value: serde_json::Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(_) => {
                    if !is_last {
                        scan.is_corrupt = true;
                    }
                    return;
                }
            };

            let record_type = value.get("type").and_then(|t| t.as_str()).unwrap_or("");

            if want_meta {
                if value.get("sessionId").and_then(|v| v.as_str()).is_some() {
                    *metadata_done = true;
                }
                if scan.git_branch.is_none() {
                    scan.git_branch = value
                        .get("gitBranch")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string());
                }
                if scan.project_path.is_none() {
                    scan.project_path = value
                        .get("cwd")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string());
                }
            }

            match record_type {
                "user" => {
                    scan.message_count += 1;
                    scan.has_messages = true;
                    if scan.first_prompt.is_none() {
                        if let Some(msg) = value.get("message") {
                            if msg.get("role").and_then(|r| r.as_str()) == Some("user") {
                                if let Some(text) =
                                    extract_first_text_from_json_content(msg.get("content"))
                                {
                                    scan.first_prompt = Some(truncate_string(&text, 200));
                                }
                            }
                        }
                    }
                }
                "assistant" => {
                    scan.message_count += 1;
                    scan.has_messages = true;
                }
                "custom-title" => {
                    let title = value
                        .get("customTitle")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if title.is_empty() {
                        scan.custom_title = None;
                    } else {
                        scan.custom_title = Some(title);
                    }
                }
                _ => {}
            }
        };

    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim().to_string();

        // Flush previous line as "not last"
        if let Some(p) = prev.take() {
            process(&p, false, &mut scan, &mut metadata_done);
        }

        if !trimmed.is_empty() {
            prev = Some(trimmed);
        }
    }

    // Flush the final line as "last" (truncated JSON is not corruption)
    if let Some(p) = prev {
        process(&p, true, &mut scan, &mut metadata_done);
    }

    Some(scan)
}

/// Extract the first non-empty text string from a JSON content field.
/// Handles both plain string content and content block arrays.
fn extract_first_text_from_json_content(content: Option<&serde_json::Value>) -> Option<String> {
    let content = content?;
    match content {
        serde_json::Value::String(s) if !s.trim().is_empty() => Some(s.clone()),
        serde_json::Value::Array(arr) => {
            for item in arr {
                if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        if !text.trim().is_empty() {
                            return Some(text.to_string());
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{}...", truncated)
    }
}

// ── Fork session logic ──

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkResult {
    pub new_session_id: String,
    pub new_file_path: String,
    pub message_count: u32,
    pub first_prompt: Option<String>,
}

/// Fork a session from a specific user message.
/// Copies all JSONL lines up to (and including) the target user message and its
/// assistant reply, replacing sessionId with a new UUID. Registers the new session
/// in sessions-index.json.
pub fn fork_session_from_message(
    original_file_path: &Path,
    user_msg_uuid: &str,
    project_path: Option<&str>,
) -> Result<ForkResult, String> {
    let new_session_id = uuid::Uuid::new_v4().to_string();

    let file = File::open(original_file_path).map_err(|e| format!("Failed to open file: {}", e))?;
    let reader = BufReader::new(file);

    // Phase 1: collect lines up to and including the target user message
    let mut collected_lines: Vec<String> = Vec::new();
    let mut found_target = false;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            collected_lines.push(line);
            continue;
        }

        collected_lines.push(line.clone());

        // Check if this line contains the target uuid
        if !found_target {
            if let Ok(record) = serde_json::from_str::<serde_json::Value>(trimmed) {
                if record.get("uuid").and_then(|v| v.as_str()) == Some(user_msg_uuid)
                    && record.get("type").and_then(|v| v.as_str()) == Some("user")
                {
                    found_target = true;
                }
            }
        } else {
            // Phase 2: after finding the user message, look for the assistant reply
            if let Ok(record) = serde_json::from_str::<serde_json::Value>(trimmed) {
                let is_assistant = record.get("type").and_then(|v| v.as_str()) == Some("assistant");
                let parent_matches =
                    record.get("parentUuid").and_then(|v| v.as_str()) == Some(user_msg_uuid);
                if is_assistant && parent_matches {
                    // Found the assistant reply — stop collecting
                    break;
                }
                // If we hit another user message, the target had no assistant reply
                let is_user = record.get("type").and_then(|v| v.as_str()) == Some("user");
                if is_user {
                    // Remove this line (it's the next user message, not part of the fork)
                    collected_lines.pop();
                    break;
                }
            }
        }
    }

    if !found_target {
        return Err(format!(
            "User message with uuid '{}' not found",
            user_msg_uuid
        ));
    }

    // Replace sessionId in every line
    let mut output_lines: Vec<String> = Vec::new();
    let mut message_count: u32 = 0;
    let mut first_prompt: Option<String> = None;

    for line in &collected_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            output_lines.push(line.clone());
            continue;
        }

        // Try to parse and replace sessionId
        if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if value.get("sessionId").is_some() {
                value["sessionId"] = serde_json::Value::String(new_session_id.clone());
            }

            // Count user/assistant messages
            if let Some(t) = value.get("type").and_then(|v| v.as_str()) {
                if t == "user" || t == "assistant" {
                    message_count += 1;
                }
                // Extract first user prompt
                if t == "user" && first_prompt.is_none() {
                    if let Some(msg) = value.get("message") {
                        if let Some(content) = msg.get("content") {
                            if let Some(s) = content.as_str() {
                                if !s.is_empty() {
                                    first_prompt = Some(truncate_string(s, 200));
                                }
                            } else if let Some(blocks) = content.as_array() {
                                for block in blocks {
                                    if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                                        if let Some(text) =
                                            block.get("text").and_then(|v| v.as_str())
                                        {
                                            if !text.is_empty() {
                                                first_prompt = Some(truncate_string(text, 200));
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            output_lines.push(serde_json::to_string(&value).unwrap_or_else(|_| line.clone()));
        } else {
            output_lines.push(line.clone());
        }
    }

    // Write new JSONL file in the same directory
    let parent_dir = original_file_path
        .parent()
        .ok_or("Cannot determine parent directory")?;
    let new_file_name = format!("{}.jsonl", new_session_id);
    let new_file_path = parent_dir.join(&new_file_name);

    let mut out_file =
        File::create(&new_file_path).map_err(|e| format!("Failed to create file: {}", e))?;
    for line in &output_lines {
        writeln!(out_file, "{}", line).map_err(|e| format!("Failed to write line: {}", e))?;
    }

    // Update sessions-index.json
    let index_path = parent_dir.join("sessions-index.json");
    let mut index: SessionsIndex = if index_path.exists() {
        match fs::read_to_string(&index_path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
        {
            Some(idx) => idx,
            None => SessionsIndex {
                version: Some(1),
                entries: Vec::new(),
                original_path: project_path.map(|s| s.to_string()),
            },
        }
    } else {
        SessionsIndex {
            version: Some(1),
            entries: Vec::new(),
            original_path: project_path.map(|s| s.to_string()),
        }
    };

    let now = chrono::Utc::now().to_rfc3339();
    let new_file_path_str = new_file_path.to_string_lossy().to_string();

    index.entries.push(SessionsIndexFileEntry {
        session_id: new_session_id.clone(),
        full_path: Some(new_file_path_str.clone()),
        file_mtime: None,
        first_prompt: first_prompt.clone(),
        message_count: Some(message_count),
        created: Some(now.clone()),
        modified: Some(now),
        git_branch: None,
        project_path: project_path.map(|s| s.to_string()),
        is_sidechain: Some(false),
    });

    if let Ok(json) = serde_json::to_string_pretty(&index) {
        let tmp_path = index_path.with_extension("json.tmp");
        if fs::write(&tmp_path, &json).is_ok() {
            let _ = fs::rename(&tmp_path, &index_path);
        }
    }

    Ok(ForkResult {
        new_session_id,
        new_file_path: new_file_path_str,
        message_count,
        first_prompt,
    })
}
