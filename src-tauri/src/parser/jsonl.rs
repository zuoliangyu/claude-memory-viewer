use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::models::message::{
    ContentBlock, ContentValue, DisplayContentBlock, DisplayMessage, PaginatedMessages, RawRecord,
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
    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let reader = BufReader::new(file);

    let mut all_messages: Vec<DisplayMessage> = Vec::new();

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

        let record: RawRecord = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Only process user/assistant messages
        if record.record_type != "user" && record.record_type != "assistant" {
            continue;
        }

        if let Some(msg) = record.message {
            let display_blocks = convert_content(&msg.content);

            // Skip messages with no meaningful content
            if display_blocks.is_empty() {
                continue;
            }

            all_messages.push(DisplayMessage {
                uuid: record.uuid,
                role: msg.role,
                timestamp: record.timestamp,
                model: msg.model,
                content: display_blocks,
            });
        }
    }

    let total = all_messages.len();

    if from_end {
        // page=0 means last page, page=1 means second-to-last, etc.
        let end = if total > page * page_size {
            total - page * page_size
        } else {
            0
        };
        let start = if end > page_size { end - page_size } else { 0 };
        let has_more = start > 0;

        let page_messages = if end > 0 {
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
    } else {
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
}

/// Parse all messages from a JSONL file (no pagination, for search)
pub fn parse_all_messages(path: &Path) -> Result<Vec<DisplayMessage>, String> {
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

        if record.record_type != "user" && record.record_type != "assistant" {
            continue;
        }

        if let Some(msg) = record.message {
            let display_blocks = convert_content(&msg.content);
            if display_blocks.is_empty() {
                continue;
            }
            messages.push(DisplayMessage {
                uuid: record.uuid,
                role: msg.role,
                timestamp: record.timestamp,
                model: msg.model,
                content: display_blocks,
            });
        }
    }

    Ok(messages)
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
                vec![DisplayContentBlock::Text {
                    text: s.clone(),
                }]
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
                                        if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
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

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{}...", truncated)
    }
}
