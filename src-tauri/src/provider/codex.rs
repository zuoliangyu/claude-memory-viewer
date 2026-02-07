use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::models::message::{DisplayContentBlock, DisplayMessage, PaginatedMessages};
use crate::models::project::ProjectEntry;
use crate::models::session::SessionIndexEntry;
use crate::models::stats::{DailyTokenEntry, TokenUsageSummary};

/// Maximum size for text content blocks sent to frontend (20KB)
const MAX_TEXT_BLOCK_SIZE: usize = 20_000;
/// Maximum size for tool output blocks sent to frontend (30KB)
const MAX_OUTPUT_BLOCK_SIZE: usize = 30_000;
/// Maximum size for function call arguments (10KB)
const MAX_ARGS_SIZE: usize = 10_000;

// ── Directory scanning ──

fn get_codex_home() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codex"))
}

pub fn get_sessions_dir() -> Option<PathBuf> {
    get_codex_home().map(|h| h.join("sessions"))
}

pub fn scan_all_session_files() -> Vec<PathBuf> {
    let sessions_dir = match get_sessions_dir() {
        Some(d) if d.exists() => d,
        _ => return Vec::new(),
    };

    let mut files: Vec<PathBuf> = Vec::new();

    let year_dirs = match fs::read_dir(&sessions_dir) {
        Ok(d) => d,
        Err(_) => return files,
    };

    for year_entry in year_dirs.flatten() {
        let year_path = year_entry.path();
        if !year_path.is_dir() {
            continue;
        }
        let month_dirs = match fs::read_dir(&year_path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        for month_entry in month_dirs.flatten() {
            let month_path = month_entry.path();
            if !month_path.is_dir() {
                continue;
            }
            let day_dirs = match fs::read_dir(&month_path) {
                Ok(d) => d,
                Err(_) => continue,
            };
            for day_entry in day_dirs.flatten() {
                let day_path = day_entry.path();
                if !day_path.is_dir() {
                    continue;
                }
                let jsonl_files = match fs::read_dir(&day_path) {
                    Ok(d) => d,
                    Err(_) => continue,
                };
                for file_entry in jsonl_files.flatten() {
                    let file_path = file_entry.path();
                    if file_path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                        files.push(file_path);
                    }
                }
            }
        }
    }

    files
}

fn short_name_from_path(path: &str) -> String {
    let path = path.trim_end_matches(['/', '\\']);
    if let Some(pos) = path.rfind(['/', '\\']) {
        path[pos + 1..].to_string()
    } else {
        path.to_string()
    }
}

fn extract_date_from_path(path: &Path) -> Option<String> {
    let components: Vec<&str> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    let len = components.len();
    if len >= 4 {
        let day = components[len - 2];
        let month = components[len - 3];
        let year = components[len - 4];

        if year.len() == 4
            && year.chars().all(|c| c.is_ascii_digit())
            && month.len() <= 2
            && month.chars().all(|c| c.is_ascii_digit())
            && day.len() <= 2
            && day.chars().all(|c| c.is_ascii_digit())
        {
            return Some(format!("{}-{:0>2}-{:0>2}", year, month, day));
        }
    }
    None
}

// ── Session metadata ──

pub struct SessionMeta {
    pub id: String,
    pub cwd: String,
    pub cli_version: Option<String>,
    pub model_provider: Option<String>,
    pub git_branch: Option<String>,
}

pub fn extract_session_meta(path: &Path) -> Option<SessionMeta> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines().take(5) {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let row: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let row_type = row.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if row_type == "session_meta" {
            if let Some(payload) = row.get("payload") {
                let id = payload
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let cwd = payload
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let cli_version = payload
                    .get("cli_version")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let model_provider = payload
                    .get("model_provider")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let git_branch = payload
                    .get("git")
                    .and_then(|g| g.get("branch"))
                    .and_then(|v| v.as_str())
                    .map(String::from);

                return Some(SessionMeta {
                    id,
                    cwd,
                    cli_version,
                    model_provider,
                    git_branch,
                });
            }
        }
    }
    None
}

// ── Projects and sessions ──

fn list_all_sessions() -> Result<Vec<SessionIndexEntry>, String> {
    let files = scan_all_session_files();
    let mut entries: Vec<SessionIndexEntry> = Vec::new();

    for file_path in files {
        let meta = extract_session_meta(&file_path);
        let first_prompt = extract_first_prompt(&file_path);
        let message_count = count_messages(&file_path);

        let (session_id, cwd, model_provider, cli_version, git_branch) = match meta {
            Some(m) => (m.id, m.cwd, m.model_provider, m.cli_version, m.git_branch),
            None => {
                let stem = file_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                (stem, String::new(), None, None, None)
            }
        };

        let short_name = if cwd.is_empty() {
            "unknown".to_string()
        } else {
            short_name_from_path(&cwd)
        };
        let _ = short_name; // used indirectly via cwd

        let file_meta = fs::metadata(&file_path).ok();
        let modified = file_meta.as_ref().and_then(|m| {
            m.modified().ok().map(|t| {
                let d = t
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default();
                chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default()
            })
        });

        let created = file_meta.as_ref().and_then(|m| {
            m.created().ok().map(|t| {
                let d = t
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default();
                chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default()
            })
        });

        entries.push(SessionIndexEntry {
            source: "codex".to_string(),
            session_id,
            file_path: file_path.to_string_lossy().to_string(),
            first_prompt,
            message_count,
            created,
            modified,
            git_branch,
            project_path: None,
            is_sidechain: None,
            cwd: Some(cwd),
            model_provider,
            cli_version,
        });
    }

    entries.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(entries)
}

pub fn get_projects() -> Result<Vec<ProjectEntry>, String> {
    let sessions = list_all_sessions()?;

    let mut project_map: HashMap<String, ProjectEntry> = HashMap::new();

    for session in sessions {
        let cwd = session.cwd.as_deref().unwrap_or("").to_string();
        if cwd.is_empty() {
            continue;
        }

        let entry = project_map
            .entry(cwd.clone())
            .or_insert_with(|| ProjectEntry {
                source: "codex".to_string(),
                id: cwd.clone(),
                display_path: cwd.clone(),
                short_name: short_name_from_path(&cwd),
                session_count: 0,
                last_modified: None,
                model_provider: session.model_provider.clone(),
            });

        entry.session_count += 1;

        if let Some(ref modified) = session.modified {
            if entry
                .last_modified
                .as_ref()
                .map(|m| modified > m)
                .unwrap_or(true)
            {
                entry.last_modified = Some(modified.clone());
            }
        }
    }

    let mut projects: Vec<ProjectEntry> = project_map.into_values().collect();
    projects.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));

    Ok(projects)
}

pub fn get_sessions(cwd: &str) -> Result<Vec<SessionIndexEntry>, String> {
    let mut entries = list_all_sessions()?;
    entries.retain(|e| e.cwd.as_deref() == Some(cwd));
    Ok(entries)
}

// ── Message parsing ──

pub fn parse_session_messages(
    path: &Path,
    page: usize,
    page_size: usize,
) -> Result<PaginatedMessages, String> {
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

pub fn parse_all_messages(path: &Path) -> Result<Vec<DisplayMessage>, String> {
    let file = fs::File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
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

        let row: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let row_type = row.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let timestamp = row.get("timestamp").and_then(|v| v.as_str()).map(String::from);
        let payload = match row.get("payload") {
            Some(p) => p,
            None => continue,
        };

        if row_type == "response_item" {
            let payload_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");

            match payload_type {
                "message" => {
                    let role = payload
                        .get("role")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if role == "developer" || role == "system" {
                        continue;
                    }
                    if role == "user" || role == "assistant" {
                        let content_blocks = extract_message_content(payload);
                        if !content_blocks.is_empty() {
                            messages.push(DisplayMessage {
                                uuid: None,
                                role: role.to_string(),
                                timestamp: timestamp.clone(),
                                content: content_blocks,
                            });
                        }
                    }
                }
                "function_call" => {
                    let name = payload
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let arguments = payload
                        .get("arguments")
                        .map(|v| {
                            if let Some(s) = v.as_str() {
                                if let Ok(parsed) = serde_json::from_str::<Value>(s) {
                                    serde_json::to_string_pretty(&parsed)
                                        .unwrap_or_else(|_| s.to_string())
                                } else {
                                    s.to_string()
                                }
                            } else {
                                serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
                            }
                        })
                        .unwrap_or_default();
                    let call_id = payload
                        .get("call_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    messages.push(DisplayMessage {
                        uuid: None,
                        role: "assistant".to_string(),
                        timestamp: timestamp.clone(),
                        content: vec![DisplayContentBlock::FunctionCall {
                            name,
                            arguments: truncate_string(&arguments, MAX_ARGS_SIZE),
                            call_id,
                        }],
                    });
                }
                "function_call_output" => {
                    let call_id = payload
                        .get("call_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let output = payload
                        .get("output")
                        .map(|v| {
                            if let Some(s) = v.as_str() {
                                s.to_string()
                            } else {
                                serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
                            }
                        })
                        .unwrap_or_default();

                    messages.push(DisplayMessage {
                        uuid: None,
                        role: "tool".to_string(),
                        timestamp: timestamp.clone(),
                        content: vec![DisplayContentBlock::FunctionCallOutput {
                            call_id,
                            output: truncate_string(&output, MAX_OUTPUT_BLOCK_SIZE),
                        }],
                    });
                }
                "reasoning" => {
                    let text = payload
                        .get("text")
                        .or_else(|| payload.get("summary").and_then(|s| s.get(0)))
                        .map(|v| {
                            if let Some(s) = v.as_str() {
                                s.to_string()
                            } else if let Some(arr) = v.as_array() {
                                arr.iter()
                                    .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                                    .collect::<Vec<&str>>()
                                    .join("\n")
                            } else {
                                v.to_string()
                            }
                        })
                        .unwrap_or_default();

                    if !text.is_empty() {
                        messages.push(DisplayMessage {
                            uuid: None,
                            role: "assistant".to_string(),
                            timestamp: timestamp.clone(),
                            content: vec![DisplayContentBlock::Reasoning { text }],
                        });
                    }
                }
                _ => {}
            }
        }
    }

    Ok(messages)
}

fn extract_message_content(payload: &Value) -> Vec<DisplayContentBlock> {
    let mut blocks = Vec::new();

    if let Some(content) = payload.get("content") {
        if let Some(arr) = content.as_array() {
            for item in arr {
                let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match item_type {
                    "input_text" | "output_text" | "text" => {
                        let text = item
                            .get("text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if !text.trim().is_empty() {
                            blocks.push(DisplayContentBlock::Text {
                                text: truncate_string(text, MAX_TEXT_BLOCK_SIZE),
                            });
                        }
                    }
                    "reasoning" => {
                        let text = item
                            .get("text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if !text.trim().is_empty() {
                            blocks.push(DisplayContentBlock::Reasoning {
                                text: truncate_string(text, MAX_TEXT_BLOCK_SIZE),
                            });
                        }
                    }
                    _ => {}
                }
            }
        } else if let Some(s) = content.as_str() {
            if !s.trim().is_empty() {
                blocks.push(DisplayContentBlock::Text {
                    text: truncate_string(s, MAX_TEXT_BLOCK_SIZE),
                });
            }
        }
    }

    blocks
}

pub fn extract_first_prompt(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !trimmed.contains("\"role\"") || !trimmed.contains("\"user\"") {
            continue;
        }

        let row: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let row_type = row.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if row_type != "response_item" {
            continue;
        }

        if let Some(payload) = row.get("payload") {
            let payload_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if payload_type != "message" {
                continue;
            }
            let role = payload.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role != "user" {
                continue;
            }

            if let Some(content) = payload.get("content").and_then(|c| c.as_array()) {
                for item in content {
                    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if item_type == "input_text" || item_type == "text" {
                        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                            if !text.is_empty() {
                                return Some(truncate_string(text, 200));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn count_messages(path: &Path) -> u32 {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return 0,
    };
    let reader = BufReader::new(file);
    let mut count: u32 = 0;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();

        // More precise matching: check type field value, not arbitrary string position
        if (trimmed.contains("\"type\":\"response_item\"")
            || trimmed.contains("\"type\": \"response_item\""))
            && (trimmed.contains("\"type\":\"message\"")
                || trimmed.contains("\"type\": \"message\""))
            && !trimmed.contains("\"developer\"")
            && !trimmed.contains("\"system\"")
        {
            count += 1;
        }
    }
    count
}

// ── Token info ──

pub struct TokenInfo {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

pub fn extract_token_info(path: &Path) -> Option<TokenInfo> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut last_token_info: Option<TokenInfo> = None;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.contains("\"token_count\"") {
            continue;
        }

        let row: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let row_type = row.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if row_type != "event_msg" {
            continue;
        }

        if let Some(payload) = row.get("payload") {
            let payload_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if payload_type != "token_count" {
                continue;
            }

            if let Some(info) = payload.get("info").and_then(|i| i.get("total_token_usage")) {
                let input = info.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                let output = info.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                let total = info.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(input + output);

                last_token_info = Some(TokenInfo {
                    input_tokens: input,
                    output_tokens: output,
                    total_tokens: total,
                });
            }
        }
    }

    last_token_info
}

// ── Stats ──

pub fn get_stats() -> Result<TokenUsageSummary, String> {
    let files = scan_all_session_files();

    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;
    let mut total_tokens: u64 = 0;
    let mut tokens_by_model: HashMap<String, u64> = HashMap::new();
    let mut daily_map: HashMap<String, (u64, u64, u64)> = HashMap::new();
    let mut session_count: u64 = 0;
    let mut message_count: u64 = 0;

    for file_path in &files {
        session_count += 1;
        message_count += count_messages(file_path) as u64;

        let model_provider = extract_session_meta(file_path)
            .and_then(|m| m.model_provider)
            .unwrap_or_else(|| "unknown".to_string());

        if let Some(token_info) = extract_token_info(file_path) {
            total_input_tokens += token_info.input_tokens;
            total_output_tokens += token_info.output_tokens;
            total_tokens += token_info.total_tokens;

            *tokens_by_model.entry(model_provider).or_insert(0) += token_info.total_tokens;

            if let Some(date) = extract_date_from_path(file_path) {
                let entry = daily_map.entry(date).or_insert((0, 0, 0));
                entry.0 += token_info.input_tokens;
                entry.1 += token_info.output_tokens;
                entry.2 += token_info.total_tokens;
            }
        }
    }

    let mut daily_tokens: Vec<DailyTokenEntry> = daily_map
        .into_iter()
        .map(|(date, (input, output, total))| DailyTokenEntry {
            date,
            input_tokens: input,
            output_tokens: output,
            total_tokens: total,
        })
        .collect();
    daily_tokens.sort_by(|a, b| a.date.cmp(&b.date));

    Ok(TokenUsageSummary {
        total_input_tokens,
        total_output_tokens,
        total_tokens,
        tokens_by_model,
        daily_tokens,
        session_count,
        message_count,
    })
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{}...", truncated)
    }
}
