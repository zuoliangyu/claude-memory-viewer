use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::models::message::{
    DisplayContentBlock, DisplayMessage, PaginatedMessages, RangeMessages,
};
use crate::models::project::ProjectEntry;
use crate::models::session::{SessionIndexEntry, SessionStatus};

const CHAT_HISTORY_FILE: &str = "chat_history.jsonl";
const UNROOTED_PROJECT: &str = "<grok-unrooted>";

pub struct SessionMeta {
    pub id: String,
    pub cwd: Option<String>,
}

pub fn get_sessions_dir() -> Option<PathBuf> {
    std::env::var_os("GROK_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".grok")))
        .map(|home| home.join("sessions"))
}

fn session_dirs() -> Vec<PathBuf> {
    let Some(root) = get_sessions_dir() else {
        return Vec::new();
    };
    let Ok(projects) = fs::read_dir(root) else {
        return Vec::new();
    };

    projects
        .flatten()
        .filter_map(|project| fs::read_dir(project.path()).ok())
        .flat_map(|sessions| sessions.flatten().map(|session| session.path()))
        .filter(|path| {
            path.join("summary.json").is_file() && path.join(CHAT_HISTORY_FILE).is_file()
        })
        .collect()
}

pub fn extract_session_meta(chat_history_path: &Path) -> Option<SessionMeta> {
    let summary_path = chat_history_path.parent()?.join("summary.json");
    let summary: Value = serde_json::from_str(&fs::read_to_string(summary_path).ok()?).ok()?;
    let info = summary.get("info")?;
    Some(SessionMeta {
        id: info.get("id")?.as_str()?.to_string(),
        cwd: info
            .get("cwd")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    })
}

fn text_content(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return (!text.trim().is_empty()).then(|| text.to_string());
    }

    let text = value
        .as_array()?
        .iter()
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then_some(text)
}

fn display_message_from_row(row: &Value) -> Option<DisplayMessage> {
    let row_type = row.get("type")?.as_str()?;
    let (role, model, content) = match row_type {
        "user"
            if row
                .get("synthetic_reason")
                .and_then(Value::as_str)
                .is_none() =>
        {
            (
                "user",
                None,
                DisplayContentBlock::Text {
                    text: text_content(row.get("content")?)?,
                },
            )
        }
        "assistant" => (
            "assistant",
            row.get("model_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            DisplayContentBlock::Text {
                text: text_content(row.get("content")?)?,
            },
        ),
        "reasoning" => (
            "assistant",
            None,
            DisplayContentBlock::Reasoning {
                text: text_content(row.get("summary")?)?,
            },
        ),
        _ => return None,
    };

    Some(DisplayMessage {
        uuid: None,
        parent_uuid: None,
        role: role.to_string(),
        timestamp: None,
        model,
        content: vec![content],
    })
}

pub fn parse_all_messages(path: &Path) -> Result<Vec<DisplayMessage>, String> {
    let file =
        fs::File::open(path).map_err(|error| format!("Failed to open Grok session: {error}"))?;

    Ok(BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
        .filter_map(|row| display_message_from_row(&row))
        .collect())
}

fn text_message_count(messages: &[DisplayMessage]) -> u32 {
    messages
        .iter()
        .filter(|message| {
            message
                .content
                .iter()
                .any(|block| matches!(block, DisplayContentBlock::Text { .. }))
        })
        .count() as u32
}

pub fn count_messages(path: &Path) -> u32 {
    parse_all_messages(path)
        .map(|messages| text_message_count(&messages))
        .unwrap_or(0)
}

fn session_entry(dir: &Path) -> Option<SessionIndexEntry> {
    let summary: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("summary.json")).ok()?).ok()?;
    let meta = extract_session_meta(&dir.join(CHAT_HISTORY_FILE))?;
    let file_path = dir.join(CHAT_HISTORY_FILE);

    // ponytail: Grok histories are parsed once per listing; add the shared LRU only if large
    // histories make project navigation measurably slow.
    let messages = parse_all_messages(&file_path).unwrap_or_default();
    let message_count = text_message_count(&messages);
    let first_prompt = messages.iter().find_map(|message| {
        if message.role != "user" {
            return None;
        }
        message.content.iter().find_map(|block| match block {
            DisplayContentBlock::Text { text } => Some(text.chars().take(200).collect()),
            _ => None,
        })
    });

    Some(SessionIndexEntry {
        source: "grok".to_string(),
        session_id: meta.id,
        file_path: file_path.to_string_lossy().into_owned(),
        first_prompt,
        thread_name: summary
            .get("session_summary")
            .and_then(Value::as_str)
            .filter(|title| !title.trim().is_empty())
            .map(ToString::to_string),
        message_count,
        created: summary
            .get("created_at")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        modified: summary
            .get("updated_at")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        git_branch: None,
        project_path: meta.cwd.clone(),
        is_sidechain: None,
        cwd: meta.cwd,
        model_provider: summary
            .get("current_model_id")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        cli_version: None,
        alias: None,
        tags: None,
        status: if message_count == 0 {
            SessionStatus::Empty
        } else {
            SessionStatus::Valid
        },
    })
}

pub fn get_projects() -> Result<Vec<ProjectEntry>, String> {
    let mut grouped: BTreeMap<String, Vec<SessionIndexEntry>> = BTreeMap::new();
    for dir in session_dirs() {
        if let Some(entry) = session_entry(&dir) {
            grouped
                .entry(
                    entry
                        .cwd
                        .clone()
                        .unwrap_or_else(|| UNROOTED_PROJECT.to_string()),
                )
                .or_default()
                .push(entry);
        }
    }

    Ok(grouped
        .into_iter()
        .map(|(id, sessions)| {
            let short_name = Path::new(&id)
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .unwrap_or(&id)
                .to_string();
            ProjectEntry {
                source: "grok".to_string(),
                id: id.clone(),
                display_path: id.clone(),
                short_name,
                session_count: sessions.len(),
                last_modified: sessions
                    .iter()
                    .filter_map(|session| session.modified.clone())
                    .max(),
                model_provider: None,
                alias: None,
                path_exists: Path::new(&id).exists(),
                is_virtual: id == UNROOTED_PROJECT,
            }
        })
        .collect())
}

pub fn refresh_projects_cache() -> Result<Vec<ProjectEntry>, String> {
    get_projects()
}

pub fn rebuild_projects_cache() -> Result<Vec<ProjectEntry>, String> {
    get_projects()
}

// Grok currently has no provider-local cache. Keep the common lifecycle hook so
// callers do not need a Grok-only branch when a cache is introduced later.
pub fn invalidate_sessions_cache() {}

pub fn get_sessions(project_id: &str) -> Result<Vec<SessionIndexEntry>, String> {
    let mut sessions: Vec<_> = session_dirs()
        .iter()
        .filter_map(|dir| session_entry(dir))
        .filter(|entry| entry.cwd.as_deref().unwrap_or(UNROOTED_PROJECT) == project_id)
        .collect();
    sessions.sort_by(|left, right| right.modified.cmp(&left.modified));
    Ok(sessions)
}

pub fn refresh_sessions_cache(project_id: &str) -> Result<Vec<SessionIndexEntry>, String> {
    get_sessions(project_id)
}

pub fn get_invalid_sessions(project_id: &str) -> Result<Vec<SessionIndexEntry>, String> {
    Ok(get_sessions(project_id)?
        .into_iter()
        .filter(|session| session.status != SessionStatus::Valid)
        .collect())
}

pub fn delete_project(project_id: &str) -> Result<super::claude::DeleteResult, String> {
    if project_id.is_empty() {
        return Err("Invalid project id".to_string());
    }

    let sessions = get_sessions(project_id)?;
    let project_name = Path::new(project_id)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(project_id)
        .to_string();
    let mut sessions_deleted = 0;

    for session in &sessions {
        let path = Path::new(&session.file_path);
        if !path.exists() {
            continue;
        }
        let Some(session_dir) = path.parent() else {
            continue;
        };
        if crate::recyclebin::move_to_recyclebin(
            session_dir,
            "project",
            "ManualDelete",
            "grok",
            project_id,
            None,
            Some(project_name.clone()),
        )
        .is_ok()
        {
            sessions_deleted += 1;
            let _ = crate::metadata::remove_session_meta("grok", project_id, &session.session_id);
        }
    }

    Ok(super::claude::DeleteResult {
        sessions_deleted,
        config_cleaned: false,
        bookmarks_removed: 0,
    })
}

pub fn parse_session_messages(
    path: &Path,
    page: usize,
    page_size: usize,
    from_end: bool,
) -> Result<PaginatedMessages, String> {
    let all = parse_all_messages(path)?;
    let total = all.len();
    let (start, end) = if from_end {
        (
            total.saturating_sub((page + 1).saturating_mul(page_size)),
            total.saturating_sub(page.saturating_mul(page_size)),
        )
    } else {
        let start = page.saturating_mul(page_size).min(total);
        (start, start.saturating_add(page_size).min(total))
    };

    Ok(PaginatedMessages {
        messages: all[start..end].to_vec(),
        total,
        page,
        page_size,
        has_more: if from_end { start > 0 } else { end < total },
    })
}

pub fn parse_messages_range(
    path: &Path,
    start: usize,
    end: usize,
) -> Result<RangeMessages, String> {
    let all = parse_all_messages(path)?;
    let total = all.len();
    let start = start.min(total);
    let end = end.min(total).max(start);
    Ok(RangeMessages {
        messages: all[start..end].to_vec(),
        total,
        start,
        end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_visible_history_and_paginates_from_end() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ai-session-viewer-grok-{}-{unique}.jsonl",
            std::process::id()
        ));
        let rows = [
            serde_json::json!({"type":"system","content":"hidden"}),
            serde_json::json!({"type":"user","content":[{"type":"text","text":"hello"}]}),
            serde_json::json!({"type":"user","content":[{"type":"text","text":"hidden"}],"synthetic_reason":"system_reminder"}),
            serde_json::json!({"type":"reasoning","summary":[{"type":"summary_text","text":"thinking"}]}),
            serde_json::json!({"type":"assistant","content":"world","model_id":"grok-test"}),
        ];
        fs::write(
            &path,
            rows.iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();

        assert_eq!(count_messages(&path), 2);
        let page = parse_session_messages(&path, 0, 2, true).unwrap();
        assert_eq!(page.total, 3);
        assert_eq!(page.messages.len(), 2);
        assert!(page.has_more);

        fs::remove_file(path).unwrap();
    }
}
