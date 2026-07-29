use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::models::message::{DisplayContentBlock, DisplayMessage, PaginatedMessages, RangeMessages};
use crate::models::project::ProjectEntry;
use crate::models::session::{SessionIndexEntry, SessionStatus};

const UPDATES_FILE: &str = "updates.jsonl";

pub fn get_sessions_dir() -> Option<PathBuf> {
    std::env::var_os("GROK_HOME").map(PathBuf::from).or_else(|| dirs::home_dir().map(|home| home.join(".grok"))).map(|home| home.join("sessions"))
}

fn session_dirs() -> Vec<PathBuf> {
    let Some(root) = get_sessions_dir() else { return Vec::new() };
    let Ok(projects) = fs::read_dir(root) else { return Vec::new() };
    projects.flatten().filter_map(|project| fs::read_dir(project.path()).ok()).flat_map(|sessions| sessions.flatten().map(|session| session.path())).filter(|path| path.join("summary.json").is_file() && path.join(UPDATES_FILE).is_file()).collect()
}

fn session_entry(dir: &Path) -> Option<SessionIndexEntry> {
    let summary: Value = serde_json::from_str(&fs::read_to_string(dir.join("summary.json")).ok()?).ok()?;
    let info = summary.get("info")?;
    let session_id = info.get("id").and_then(Value::as_str)?.to_string();
    let cwd = info.get("cwd").and_then(Value::as_str).map(ToString::to_string);
    let file_path = dir.join(UPDATES_FILE);
    let message_count = count_messages(&file_path);
    Some(SessionIndexEntry { source: "grok".to_string(), session_id, file_path: file_path.to_string_lossy().into_owned(), first_prompt: first_message(&file_path, "user_message_chunk"), thread_name: summary.get("generated_title").and_then(Value::as_str).map(ToString::to_string), message_count, created: summary.get("created_at").and_then(Value::as_str).map(ToString::to_string), modified: summary.get("updated_at").and_then(Value::as_str).map(ToString::to_string), git_branch: None, project_path: cwd.clone(), is_sidechain: None, cwd, model_provider: summary.get("current_model_id").and_then(Value::as_str).map(ToString::to_string), cli_version: None, alias: None, tags: None, status: if message_count == 0 { SessionStatus::Empty } else { SessionStatus::Valid } })
}

pub fn get_projects() -> Result<Vec<ProjectEntry>, String> {
    let mut grouped: BTreeMap<String, Vec<SessionIndexEntry>> = BTreeMap::new();
    for dir in session_dirs() { if let Some(entry) = session_entry(&dir) { grouped.entry(entry.cwd.clone().unwrap_or_else(|| "<grok-unrooted>".to_string())).or_default().push(entry); } }
    Ok(grouped.into_iter().map(|(id, sessions)| {
        let short_name = Path::new(&id).file_name().and_then(|name| name.to_str()).filter(|name| !name.is_empty()).unwrap_or(&id).to_string();
        ProjectEntry { source: "grok".to_string(), id: id.clone(), display_path: id.clone(), short_name, session_count: sessions.len(), last_modified: sessions.iter().filter_map(|session| session.modified.clone()).max(), model_provider: None, alias: None, path_exists: Path::new(&id).exists(), is_virtual: id == "<grok-unrooted>" }
    }).collect())
}
pub fn refresh_projects_cache() -> Result<Vec<ProjectEntry>, String> { get_projects() }
pub fn rebuild_projects_cache() -> Result<Vec<ProjectEntry>, String> { get_projects() }
pub fn invalidate_sessions_cache() {}
pub fn get_sessions(project_id: &str) -> Result<Vec<SessionIndexEntry>, String> { let mut sessions: Vec<_> = session_dirs().iter().filter_map(|dir| session_entry(dir)).filter(|entry| entry.cwd.as_deref().unwrap_or("<grok-unrooted>") == project_id).collect(); sessions.sort_by(|left, right| right.modified.cmp(&left.modified)); Ok(sessions) }
pub fn refresh_sessions_cache(project_id: &str) -> Result<Vec<SessionIndexEntry>, String> { get_sessions(project_id) }
pub fn get_invalid_sessions(_project_id: &str) -> Result<Vec<SessionIndexEntry>, String> { Ok(Vec::new()) }

fn update_parts(row: &Value) -> Option<(&str, &str)> { let update = row.get("params")?.get("update")?; Some((update.get("sessionUpdate")?.as_str()?, update.get("content")?.get("text")?.as_str()?)) }
fn first_message(path: &Path, wanted: &str) -> Option<String> { let file = fs::File::open(path).ok()?; BufReader::new(file).lines().map_while(Result::ok).filter_map(|line| serde_json::from_str::<Value>(&line).ok()).find_map(|row| { let (kind, text) = update_parts(&row)?; (kind == wanted && !text.trim().is_empty()).then(|| text.chars().take(200).collect()) }) }
pub fn count_messages(path: &Path) -> u32 { let Ok(file) = fs::File::open(path) else { return 0 }; BufReader::new(file).lines().map_while(Result::ok).filter_map(|line| serde_json::from_str::<Value>(&line).ok()).filter_map(|row| update_parts(&row).map(|(kind, _)| kind)).filter(|kind| matches!(*kind, "user_message_chunk" | "agent_message_chunk")).count() as u32 }
pub fn parse_all_messages(path: &Path) -> Result<Vec<DisplayMessage>, String> {
    let file = fs::File::open(path).map_err(|error| format!("Failed to open Grok session: {error}"))?;
    let mut messages = Vec::new();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(row) = serde_json::from_str::<Value>(&line) else { continue }; let Some((kind, text)) = update_parts(&row) else { continue }; if text.is_empty() { continue; }
        let timestamp = row.get("timestamp").and_then(Value::as_str).map(ToString::to_string);
        let (role, content) = match kind { "user_message_chunk" => ("user", DisplayContentBlock::Text { text: text.to_string() }), "agent_message_chunk" => ("assistant", DisplayContentBlock::Text { text: text.to_string() }), "agent_thought_chunk" => ("assistant", DisplayContentBlock::Reasoning { text: text.to_string() }), _ => continue };
        messages.push(DisplayMessage { uuid: None, parent_uuid: None, role: role.to_string(), timestamp, model: None, content: vec![content] });
    }
    Ok(messages)
}
pub fn parse_session_messages(path: &Path, page: usize, page_size: usize, from_end: bool) -> Result<PaginatedMessages, String> { let all = parse_all_messages(path)?; let total = all.len(); let start = if from_end { total.saturating_sub((page + 1).saturating_mul(page_size)) } else { page.saturating_mul(page_size).min(total) }; let end = if from_end { total.saturating_sub(page.saturating_mul(page_size)) } else { start.saturating_add(page_size).min(total) }; Ok(PaginatedMessages { messages: all[start..end].to_vec(), total, page, page_size, has_more: if from_end { start > 0 } else { end < total } }) }
pub fn parse_messages_range(path: &Path, start: usize, end: usize) -> Result<RangeMessages, String> { let all = parse_all_messages(path)?; let total = all.len(); let start = start.min(total); let end = end.min(total).max(start); Ok(RangeMessages { messages: all[start..end].to_vec(), total, start, end }) }
