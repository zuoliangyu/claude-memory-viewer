use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde_json::Value;

use crate::models::message::{
    DisplayContentBlock, DisplayMessage, PaginatedMessages, RangeMessages,
};
use crate::models::project::ProjectEntry;
use crate::models::session::{SessionIndexEntry, SessionStatus};
use crate::state::{
    file_modified_key, get_cached_page, get_cached_range, paginate_from_range, store_full_messages,
};

const DISK_CACHE_VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct OmpDiskCache {
    version: u32,
    sessions_by_file: HashMap<String, CachedOmpSession>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CachedOmpSession {
    modified_key: u64,
    size: u64,
    entry: SessionIndexEntry,
}

#[derive(Debug, Clone)]
pub struct SessionMeta {
    pub id: String,
    pub cwd: String,
}

#[derive(Debug, Clone)]
struct SessionHeader {
    id: String,
    cwd: String,
    title: Option<String>,
    timestamp: Option<String>,
}

fn sessions_cache() -> &'static Mutex<Option<OmpDiskCache>> {
    static CACHE: OnceLock<Mutex<Option<OmpDiskCache>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn disk_cache_path() -> Option<PathBuf> {
    let dir = dirs::config_dir()?.join("ai-session-viewer");
    let _ = fs::create_dir_all(&dir);
    Some(dir.join("omp-list-cache.json"))
}

fn read_disk_cache() -> OmpDiskCache {
    let Some(path) = disk_cache_path() else {
        return OmpDiskCache::default();
    };
    let Ok(content) = fs::read_to_string(path) else {
        return OmpDiskCache::default();
    };
    let Ok(cache) = serde_json::from_str::<OmpDiskCache>(&content) else {
        return OmpDiskCache::default();
    };
    if cache.version == DISK_CACHE_VERSION {
        cache
    } else {
        OmpDiskCache::default()
    }
}

fn save_disk_cache(cache: &OmpDiskCache) {
    let Some(path) = disk_cache_path() else {
        return;
    };
    let Ok(content) = serde_json::to_string(cache) else {
        return;
    };
    let temporary = path.with_extension("json.tmp");
    if fs::write(&temporary, content).is_err() {
        return;
    }
    if fs::rename(&temporary, &path).is_err() {
        let _ = fs::copy(&temporary, &path);
        let _ = fs::remove_file(&temporary);
    }
}

fn active_profile() -> Option<String> {
    let profile = std::env::var("OMP_PROFILE")
        .ok()
        .or_else(|| std::env::var("PI_PROFILE").ok())?;
    let profile = profile.trim();
    if profile.is_empty() || profile == "default" {
        return None;
    }
    let valid = profile.len() <= 64
        && profile != "."
        && profile != ".."
        && !profile.ends_with('.')
        && profile.chars().enumerate().all(|(index, character)| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || (index > 0 && matches!(character, '.' | '_' | '-'))
        });
    valid.then(|| profile.to_string())
}

/// OMP resolves this directory from PI_CODING_AGENT_DIR when set, otherwise
/// from PI_CONFIG_DIR (default `.omp`) and the active OMP profile.
pub fn get_sessions_dir() -> Option<PathBuf> {
    if let Some(agent_dir) = std::env::var_os("PI_CODING_AGENT_DIR") {
        if !agent_dir.is_empty() {
            return Some(PathBuf::from(agent_dir).join("sessions"));
        }
    }

    let home = dirs::home_dir()?;
    let config_dir = std::env::var_os("PI_CONFIG_DIR").unwrap_or_else(|| ".omp".into());
    let mut agent_dir = home.join(config_dir);
    if let Some(profile) = active_profile() {
        agent_dir = agent_dir.join("profiles").join(profile);
    }
    Some(agent_dir.join("agent").join("sessions"))
}

fn is_session_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .is_some_and(|extension| extension == "jsonl")
}

fn session_files_in(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if is_session_file(&path) {
            files.push(path);
            continue;
        }
        if !path.is_dir()
            || root
                .join(entry.file_name())
                .with_extension("jsonl")
                .is_file()
        {
            continue;
        }
        if let Ok(project_entries) = fs::read_dir(path) {
            files.extend(
                project_entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| is_session_file(path)),
            );
        }
    }
    files
}

fn session_files() -> Vec<PathBuf> {
    get_sessions_dir()
        .as_deref()
        .map(session_files_in)
        .unwrap_or_default()
}

fn trim_to_nonempty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn parse_session_header(path: &Path) -> Option<SessionHeader> {
    let file = fs::File::open(path).ok()?;
    let mut lines = BufReader::new(file).lines();
    let first = lines.next()?.ok()?;
    let first: Value = serde_json::from_str(first.trim()).ok()?;
    let (title_override, header) = if first.get("type").and_then(Value::as_str) == Some("title") {
        let title = trim_to_nonempty(first.get("title").and_then(Value::as_str));
        let next = lines.next()?.ok()?;
        (title, serde_json::from_str::<Value>(next.trim()).ok()?)
    } else {
        (None, first)
    };

    if header.get("type").and_then(Value::as_str) != Some("session") {
        return None;
    }
    let id = trim_to_nonempty(header.get("id").and_then(Value::as_str))?;
    let cwd = trim_to_nonempty(header.get("cwd").and_then(Value::as_str))?;
    let title =
        title_override.or_else(|| trim_to_nonempty(header.get("title").and_then(Value::as_str)));
    let timestamp = header
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc).to_rfc3339());

    Some(SessionHeader {
        id,
        cwd,
        title,
        timestamp,
    })
}

pub fn extract_session_meta(path: &Path) -> Option<SessionMeta> {
    let header = parse_session_header(path)?;
    Some(SessionMeta {
        id: header.id,
        cwd: header.cwd,
    })
}

fn json_value_text(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return trim_to_nonempty(Some(text));
    }
    let text = value
        .as_array()?
        .iter()
        .filter_map(|block| match block {
            Value::String(text) => trim_to_nonempty(Some(text)),
            Value::Object(_) => block
                .get("text")
                .and_then(Value::as_str)
                .and_then(|text| trim_to_nonempty(Some(text))),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    trim_to_nonempty(Some(&text))
}

fn json_argument_text(value: &Value) -> String {
    value
        .as_str()
        .map(ToString::to_string)
        .unwrap_or_else(|| serde_json::to_string(value).unwrap_or_default())
}

fn display_blocks(content: &Value) -> Vec<DisplayContentBlock> {
    match content {
        Value::String(text) => trim_to_nonempty(Some(text))
            .map(|text| vec![DisplayContentBlock::Text { text }])
            .unwrap_or_default(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|block| {
                let block_type = block.get("type")?.as_str()?;
                match block_type {
                    "text" => trim_to_nonempty(block.get("text").and_then(Value::as_str))
                        .map(|text| DisplayContentBlock::Text { text }),
                    "thinking" => trim_to_nonempty(
                        block
                            .get("thinking")
                            .or_else(|| block.get("text"))
                            .and_then(Value::as_str),
                    )
                    .map(|thinking| DisplayContentBlock::Thinking { thinking }),
                    "toolCall" => {
                        let id = trim_to_nonempty(block.get("id").and_then(Value::as_str))?;
                        let name = trim_to_nonempty(
                            block
                                .get("name")
                                .or_else(|| block.get("toolName"))
                                .and_then(Value::as_str),
                        )?;
                        let input = block
                            .get("arguments")
                            .or_else(|| block.get("input"))
                            .map(json_argument_text)
                            .unwrap_or_default();
                        Some(DisplayContentBlock::ToolUse { id, name, input })
                    }
                    _ => None,
                }
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn display_message_from_entry(entry: &Value) -> Option<DisplayMessage> {
    if entry.get("type").and_then(Value::as_str) != Some("message") {
        return None;
    }
    let message = entry.get("message")?;
    let role = message.get("role")?.as_str()?;
    let (role, model, content) = match role {
        "user" => ("user", None, display_blocks(message.get("content")?)),
        "assistant" => (
            "assistant",
            message
                .get("model")
                .or_else(|| message.get("modelId"))
                .and_then(Value::as_str)
                .map(ToString::to_string),
            display_blocks(message.get("content")?),
        ),
        "toolResult" => {
            let tool_use_id = trim_to_nonempty(
                message
                    .get("toolCallId")
                    .or_else(|| message.get("toolUseId"))
                    .and_then(Value::as_str),
            )?;
            let content = json_value_text(message.get("content")?)?;
            let is_error = message
                .get("isError")
                .or_else(|| message.get("error"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            (
                "tool",
                None,
                vec![DisplayContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                }],
            )
        }
        _ => return None,
    };

    (!content.is_empty()).then_some(DisplayMessage {
        uuid: entry
            .get("id")
            .or_else(|| message.get("id"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        parent_uuid: None,
        role: role.to_string(),
        timestamp: message
            .get("timestamp")
            .or_else(|| entry.get("timestamp"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        model,
        content,
    })
}

fn nearest_visible_parent(
    parent_id: Option<&str>,
    parents_by_id: &HashMap<String, String>,
    visible_ids: &HashSet<String>,
) -> Option<String> {
    let mut current = parent_id?;
    for _ in 0..=parents_by_id.len() {
        if visible_ids.contains(current) {
            return Some(current.to_string());
        }
        current = parents_by_id.get(current)?;
    }
    None
}

fn display_messages(entries: Vec<Value>) -> Vec<DisplayMessage> {
    let parents_by_id: HashMap<_, _> = entries
        .iter()
        .filter_map(|entry| {
            Some((
                trim_to_nonempty(entry.get("id")?.as_str())?,
                trim_to_nonempty(entry.get("parentId")?.as_str())?,
            ))
        })
        .collect();
    let mut messages: Vec<_> = entries
        .iter()
        .filter_map(display_message_from_entry)
        .collect();
    let visible_ids: HashSet<_> = messages
        .iter()
        .filter_map(|message| message.uuid.clone())
        .collect();

    for message in &mut messages {
        message.parent_uuid = nearest_visible_parent(
            message
                .uuid
                .as_deref()
                .and_then(|id| parents_by_id.get(id).map(String::as_str)),
            &parents_by_id,
            &visible_ids,
        );
    }
    messages
}

pub fn parse_all_messages(path: &Path) -> Result<Vec<DisplayMessage>, String> {
    let file =
        fs::File::open(path).map_err(|error| format!("Failed to open OMP session: {error}"))?;
    let entries = BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
        .collect();
    Ok(display_messages(entries))
}

fn file_modified_iso(path: &Path) -> Option<String> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    Some(DateTime::<Utc>::from(modified).to_rfc3339())
}

fn session_entry(path: &Path) -> Option<SessionIndexEntry> {
    let header = parse_session_header(path)?;
    let messages = parse_all_messages(path).ok()?;
    let first_prompt = messages.iter().find_map(|message| {
        (message.role == "user").then_some(())?;
        message.content.iter().find_map(|block| match block {
            DisplayContentBlock::Text { text } => Some(text.chars().take(200).collect()),
            _ => None,
        })
    });

    Some(SessionIndexEntry {
        source: "omp".to_string(),
        session_id: header.id,
        file_path: path.to_string_lossy().into_owned(),
        first_prompt,
        thread_name: header.title,
        message_count: messages.len() as u32,
        created: header.timestamp,
        modified: file_modified_iso(path),
        git_branch: None,
        project_path: Some(header.cwd.clone()),
        is_sidechain: None,
        cwd: Some(header.cwd),
        model_provider: None,
        cli_version: None,
        alias: None,
        tags: None,
        status: if messages.is_empty() {
            SessionStatus::Empty
        } else {
            SessionStatus::Valid
        },
    })
}

fn cached_session(path: &Path) -> Option<CachedOmpSession> {
    Some(CachedOmpSession {
        modified_key: file_modified_key(path).ok()?,
        size: fs::metadata(path).ok()?.len(),
        entry: session_entry(path)?,
    })
}

fn reconcile_cache(mut cache: OmpDiskCache, files: Vec<PathBuf>) -> (OmpDiskCache, bool) {
    let mut old = std::mem::take(&mut cache.sessions_by_file);
    let mut sessions_by_file = HashMap::with_capacity(files.len());
    let mut changed = cache.version != DISK_CACHE_VERSION;
    cache.version = DISK_CACHE_VERSION;

    for path in files {
        let key = path.to_string_lossy().into_owned();
        let modified_key = file_modified_key(&path).ok();
        let size = fs::metadata(&path).ok().map(|metadata| metadata.len());
        match (old.remove(&key), modified_key, size) {
            (Some(cached), Some(modified_key), Some(size))
                if cached.modified_key == modified_key && cached.size == size =>
            {
                sessions_by_file.insert(key, cached);
            }
            _ => {
                changed = true;
                if let Some(session) = cached_session(&path) {
                    sessions_by_file.insert(key, session);
                }
            }
        }
    }

    if !old.is_empty() {
        changed = true;
    }
    cache.sessions_by_file = sessions_by_file;
    (cache, changed)
}

fn load_all_sessions() -> Vec<SessionIndexEntry> {
    let mut state = sessions_cache().lock();
    let cache = state.take().unwrap_or_else(read_disk_cache);
    let (cache, changed) = reconcile_cache(cache, session_files());
    if changed {
        save_disk_cache(&cache);
    }
    let sessions = cache
        .sessions_by_file
        .values()
        .map(|cached| cached.entry.clone())
        .collect();
    *state = Some(cache);
    sessions
}

fn rebuild_all_sessions() -> Vec<SessionIndexEntry> {
    let (cache, _) = reconcile_cache(OmpDiskCache::default(), session_files());
    save_disk_cache(&cache);
    let sessions = cache
        .sessions_by_file
        .values()
        .map(|cached| cached.entry.clone())
        .collect();
    *sessions_cache().lock() = Some(cache);
    sessions
}

fn projects_from_sessions(sessions: Vec<SessionIndexEntry>) -> Vec<ProjectEntry> {
    let mut grouped: BTreeMap<String, Vec<SessionIndexEntry>> = BTreeMap::new();
    for session in sessions {
        let Some(cwd) = session.cwd.clone() else {
            continue;
        };
        grouped.entry(cwd).or_default().push(session);
    }

    grouped
        .into_iter()
        .map(|(id, sessions)| {
            let short_name = Path::new(&id)
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .unwrap_or(&id)
                .to_string();
            ProjectEntry {
                source: "omp".to_string(),
                id: id.clone(),
                display_path: id.clone(),
                short_name,
                session_count: sessions
                    .iter()
                    .filter(|session| session.status == SessionStatus::Valid)
                    .count(),
                last_modified: sessions
                    .iter()
                    .filter_map(|session| session.modified.clone())
                    .max(),
                model_provider: None,
                alias: None,
                path_exists: Path::new(&id).exists(),
                is_virtual: false,
            }
        })
        .collect()
}

pub fn get_projects() -> Result<Vec<ProjectEntry>, String> {
    Ok(projects_from_sessions(load_all_sessions()))
}

pub fn refresh_projects_cache() -> Result<Vec<ProjectEntry>, String> {
    get_projects()
}

pub fn rebuild_projects_cache() -> Result<Vec<ProjectEntry>, String> {
    Ok(projects_from_sessions(rebuild_all_sessions()))
}

pub fn invalidate_sessions_cache() {
    *sessions_cache().lock() = None;
}

pub fn invalidate_paths(paths: &[PathBuf]) {
    let Some(root) = get_sessions_dir() else {
        return;
    };
    if paths.iter().any(|path| path.starts_with(&root)) {
        invalidate_sessions_cache();
    }
}

pub fn get_sessions(project_id: &str) -> Result<Vec<SessionIndexEntry>, String> {
    let mut sessions: Vec<_> = load_all_sessions()
        .into_iter()
        .filter(|session| session.cwd.as_deref() == Some(project_id))
        .collect();
    sessions.sort_by(|left, right| right.modified.cmp(&left.modified));
    Ok(sessions)
}
/// Resolve a session id to the on-disk file OMP can resume directly.
///
/// OMP session ids are stored in files whose names also contain a timestamp,
/// and the CLI may use a profile-specific session directory. Looking up the
/// session through the same provider scan avoids relying on the CLI's implicit
/// directory resolution.
pub fn find_session_file(project_id: &str, session_id: &str) -> Option<PathBuf> {
    get_sessions(project_id)
        .ok()?
        .into_iter()
        .find(|session| session.session_id == session_id)
        .map(|session| PathBuf::from(session.file_path))
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
    if project_id.trim().is_empty() {
        return Err("Invalid project id".to_string());
    }
    let project_name = Path::new(project_id)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(project_id)
        .to_string();
    let mut sessions_deleted = 0;
    for session in get_sessions(project_id)? {
        let path = match crate::paths::validate_session_file("omp", &session.file_path) {
            Ok(path) => path,
            Err(_) => continue,
        };
        let Some(metadata) = extract_session_meta(&path) else {
            continue;
        };
        if metadata.cwd != project_id || metadata.id != session.session_id {
            continue;
        }
        if crate::recyclebin::move_omp_session_to_recyclebin(
            &path,
            project_id,
            session.thread_name.clone().or(session.first_prompt.clone()),
            Some(project_name.clone()),
        )
        .is_ok()
        {
            sessions_deleted += 1;
            let _ = crate::metadata::remove_session_meta("omp", project_id, &session.session_id);
        }
    }
    invalidate_sessions_cache();
    Ok(super::claude::DeleteResult {
        sessions_deleted,
        config_cleaned: false,
        bookmarks_removed: 0,
    })
}

/// Permanently remove one validated top-level session and its optional
/// sibling artifact directory. Artifact removal happens first so failure
/// cannot leave a transcript pointing at missing attachments.
pub fn permanently_delete_session(path: &Path) -> Result<(), String> {
    let artifact_path = path.with_extension("");
    match fs::symlink_metadata(&artifact_path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("Failed to inspect OMP session artifacts: {error}")),
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err("OMP session artifact directory must not be a symbolic link".to_string());
        }
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(&artifact_path)
            .map_err(|error| format!("Failed to delete OMP session artifacts: {error}"))?,
        Ok(_) => return Err("OMP session artifact path is not a directory".to_string()),
    }
    fs::remove_file(path).map_err(|error| format!("Failed to delete OMP session: {error}"))?;
    invalidate_sessions_cache();
    Ok(())
}

pub fn parse_session_messages(
    path: &Path,
    page: usize,
    page_size: usize,
    from_end: bool,
) -> Result<PaginatedMessages, String> {
    if let Some(cached) = get_cached_page(path, page, page_size, from_end)? {
        return Ok(cached);
    }
    let messages = parse_all_messages(path)?;
    store_full_messages(path, &messages)?;
    paginate_from_range(&messages, messages.len(), page, page_size, from_end, 0)
        .ok_or_else(|| "Requested message page is outside OMP session range".to_string())
}

pub fn parse_messages_range(
    path: &Path,
    start: usize,
    end: usize,
) -> Result<RangeMessages, String> {
    if let Some((messages, total)) = get_cached_range(path, start, end)? {
        return Ok(RangeMessages {
            messages,
            total,
            start: start.min(total),
            end: end.min(total),
        });
    }
    let messages = parse_all_messages(path)?;
    let total = messages.len();
    store_full_messages(path, &messages)?;
    let start = start.min(total);
    let end = end.min(total).max(start);
    Ok(RangeMessages {
        messages: messages[start..end].to_vec(),
        total,
        start,
        end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_dir() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("ai-session-viewer-omp-{unique}"))
    }

    #[test]
    fn parses_titled_project_session_and_skips_artifact_transcripts() {
        let root = temporary_dir();
        let project_dir = root.join("project");
        fs::create_dir_all(project_dir.join("session-1")).unwrap();
        let title =
            serde_json::json!({"type": "title", "v": 1, "title": "Fixed title", "pad": " "});
        let header = serde_json::json!({
            "type": "session",
            "version": 1,
            "id": "session-1",
            "cwd": "/work/example",
            "title": "Stale header title",
            "timestamp": "2026-09-01T10:00:00.000Z"
        });
        let entries = [
            serde_json::json!({"type": "message", "message": {"role": "user", "content": "First prompt"}}),
            serde_json::json!({"type": "message", "message": {"role": "assistant", "model": "test", "content": [
                {"type": "thinking", "thinking": "reasoning"},
                {"type": "text", "text": "Answer"},
                {"type": "toolCall", "id": "call-1", "name": "read", "arguments": {"path": "src/main.rs"}}
            ]}}),
            serde_json::json!({"type": "message", "message": {"role": "toolResult", "toolCallId": "call-1", "content": [{"type": "text", "text": "file content"}]}}),
            serde_json::json!({"type": "status", "status": "complete"}),
        ];
        let session_path = project_dir.join("session-1.jsonl");
        let content = std::iter::once(title)
            .chain(std::iter::once(header))
            .chain(entries)
            .map(|entry| entry.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&session_path, format!("{content}\n")).unwrap();
        fs::write(
            project_dir.join("session-1").join("child.jsonl"),
            "not a project session",
        )
        .unwrap();

        let entry = session_entry(&session_path).unwrap();
        assert_eq!(entry.session_id, "session-1");
        assert_eq!(entry.thread_name.as_deref(), Some("Fixed title"));
        assert_eq!(entry.first_prompt.as_deref(), Some("First prompt"));
        assert_eq!(entry.message_count, 3);

        let messages = parse_all_messages(&session_path).unwrap();
        assert_eq!(messages.len(), 3);
        assert!(matches!(
            messages[1].content[0],
            DisplayContentBlock::Thinking { .. }
        ));
        assert!(matches!(
            messages[1].content[2],
            DisplayContentBlock::ToolUse { .. }
        ));
        assert!(matches!(
            messages[2].content[0],
            DisplayContentBlock::ToolResult { .. }
        ));
        assert_eq!(messages[2].role, "tool");

        let initial_range = parse_messages_range(&session_path, 0, 2).unwrap();
        assert_eq!(initial_range.total, 3);
        assert_eq!(initial_range.messages.len(), 2);
        assert_eq!(initial_range.messages[0].role, "user");
        assert_eq!(initial_range.messages[1].role, "assistant");

        let tool_range = parse_messages_range(&session_path, 2, 3).unwrap();
        assert_eq!(tool_range.messages.len(), 1);
        assert_eq!(tool_range.messages[0].role, "tool");
        assert_eq!(session_files_in(&root), vec![session_path.clone()]);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn preserves_rewind_branches_through_invisible_events() {
        let messages = display_messages(vec![
            serde_json::json!({"type": "message", "id": "user-1", "parentId": null, "message": {"role": "user", "content": "First"}}),
            serde_json::json!({"type": "message", "id": "assistant-1", "parentId": "user-1", "message": {"role": "assistant", "content": "Base reply"}}),
            serde_json::json!({"type": "title_change", "id": "title-change", "parentId": "assistant-1"}),
            serde_json::json!({"type": "message", "id": "user-2", "parentId": "title-change", "message": {"role": "user", "content": "Continue"}}),
            serde_json::json!({"type": "message", "id": "rewind-branch", "parentId": "assistant-1", "message": {"role": "user", "content": "Rewind here"}}),
        ]);

        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].parent_uuid, None);
        assert_eq!(messages[1].parent_uuid.as_deref(), Some("user-1"));
        assert_eq!(messages[2].parent_uuid.as_deref(), Some("assistant-1"));
        assert_eq!(messages[3].parent_uuid.as_deref(), Some("assistant-1"));

        let questions = crate::models::message::question_index(&messages);
        assert_eq!(questions.len(), 3);
        assert_eq!(questions[1].parent_message_index, Some(0));
        assert_eq!(questions[2].parent_message_index, Some(0));
    }

    #[test]
    fn rejects_header_without_id_or_cwd() {
        let root = temporary_dir();
        fs::create_dir_all(&root).unwrap();
        let path = root.join("invalid.jsonl");
        fs::write(&path, r#"{"type":"session","id":"only-id"}"#).unwrap();
        assert!(parse_session_header(&path).is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn permanently_deletes_session_with_artifact_directory() {
        let root = temporary_dir();
        fs::create_dir_all(root.join("session-1")).unwrap();
        let session_path = root.join("session-1.jsonl");
        fs::write(&session_path, "session").unwrap();
        fs::write(root.join("session-1").join("artifact.txt"), "artifact").unwrap();

        permanently_delete_session(&session_path).unwrap();

        assert!(!session_path.exists());
        assert!(!root.join("session-1").exists());
        let _ = fs::remove_dir_all(root);
    }
}
