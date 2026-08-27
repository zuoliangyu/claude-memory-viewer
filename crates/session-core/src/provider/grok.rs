use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use parking_lot::Mutex;
use serde_json::Value;

use crate::models::message::{
    DisplayContentBlock, DisplayMessage, PaginatedMessages, RangeMessages,
};
use crate::models::project::ProjectEntry;
use crate::models::session::{SessionIndexEntry, SessionStatus};
use crate::state::file_modified_key;

const CHAT_HISTORY_FILE: &str = "chat_history.jsonl";
const UNROOTED_PROJECT: &str = "<grok-unrooted>";
const DISK_CACHE_VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct GrokDiskCache {
    version: u32,
    sessions_by_dir: HashMap<String, CachedGrokSession>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CachedGrokSession {
    summary_modified_key: u64,
    history_modified_key: u64,
    entry: SessionIndexEntry,
}

fn sessions_cache() -> &'static Mutex<Option<GrokDiskCache>> {
    static CACHE: OnceLock<Mutex<Option<GrokDiskCache>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn disk_cache_path() -> Option<PathBuf> {
    let dir = dirs::config_dir()?.join("ai-session-viewer");
    let _ = fs::create_dir_all(&dir);
    Some(dir.join("grok-list-cache.json"))
}

fn read_disk_cache() -> GrokDiskCache {
    let Some(path) = disk_cache_path() else {
        return GrokDiskCache::default();
    };
    let Ok(content) = fs::read_to_string(path) else {
        return GrokDiskCache::default();
    };
    let Ok(cache) = serde_json::from_str::<GrokDiskCache>(&content) else {
        return GrokDiskCache::default();
    };
    if cache.version == DISK_CACHE_VERSION {
        cache
    } else {
        GrokDiskCache::default()
    }
}

fn save_disk_cache(cache: &GrokDiskCache) {
    let Some(path) = disk_cache_path() else {
        return;
    };
    let Ok(json) = serde_json::to_string(cache) else {
        return;
    };
    let tmp_path = path.with_extension("json.tmp");
    if fs::write(&tmp_path, json).is_err() {
        return;
    }
    if fs::rename(&tmp_path, &path).is_err() && fs::copy(&tmp_path, &path).is_ok() {
        let _ = fs::remove_file(tmp_path);
    }
}

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
    let info = summary.get("info")?;
    let session_id = info.get("id")?.as_str()?.to_string();
    let cwd = info
        .get("cwd")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let file_path = dir.join(CHAT_HISTORY_FILE);

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
        session_id,
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
        project_path: cwd.clone(),
        is_sidechain: None,
        cwd,
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

fn session_modified_keys(dir: &Path) -> Option<(u64, u64)> {
    Some((
        file_modified_key(&dir.join("summary.json")).ok()?,
        file_modified_key(&dir.join(CHAT_HISTORY_FILE)).ok()?,
    ))
}

fn cached_session_for_dir(dir: &Path) -> Option<CachedGrokSession> {
    let entry = session_entry(dir)?;
    let (summary_modified_key, history_modified_key) = session_modified_keys(dir)?;
    Some(CachedGrokSession {
        summary_modified_key,
        history_modified_key,
        entry,
    })
}

fn reconcile_sessions_cache_with<F>(
    mut cache: GrokDiskCache,
    dirs: Vec<PathBuf>,
    mut scan: F,
) -> (GrokDiskCache, bool)
where
    F: FnMut(&Path) -> Option<CachedGrokSession>,
{
    let mut cached_by_dir = std::mem::take(&mut cache.sessions_by_dir);
    let mut sessions_by_dir = HashMap::with_capacity(dirs.len());
    let mut changed = cache.version != DISK_CACHE_VERSION;
    cache.version = DISK_CACHE_VERSION;

    for dir in dirs {
        let key = dir.to_string_lossy().into_owned();
        let modified_keys = session_modified_keys(&dir);
        match (cached_by_dir.remove(&key), modified_keys) {
            (Some(cached), Some((summary_key, history_key)))
                if cached.summary_modified_key == summary_key
                    && cached.history_modified_key == history_key =>
            {
                sessions_by_dir.insert(key, cached);
            }
            _ => {
                changed = true;
                if let Some(session) = scan(&dir) {
                    sessions_by_dir.insert(key, session);
                }
            }
        }
    }

    if !cached_by_dir.is_empty() {
        changed = true;
    }
    cache.sessions_by_dir = sessions_by_dir;
    (cache, changed)
}

fn reconcile_sessions_cache(cache: GrokDiskCache, dirs: Vec<PathBuf>) -> (GrokDiskCache, bool) {
    reconcile_sessions_cache_with(cache, dirs, cached_session_for_dir)
}

fn sessions_from_cache(cache: &GrokDiskCache) -> Vec<SessionIndexEntry> {
    cache
        .sessions_by_dir
        .values()
        .map(|cached| cached.entry.clone())
        .collect()
}

fn load_all_sessions() -> Vec<SessionIndexEntry> {
    let mut state = sessions_cache().lock();
    let base = state.take().unwrap_or_else(read_disk_cache);
    let (cache, changed) = reconcile_sessions_cache(base, session_dirs());
    if changed {
        save_disk_cache(&cache);
    }
    let sessions = sessions_from_cache(&cache);
    *state = Some(cache);
    sessions
}

fn rebuild_all_sessions() -> Vec<SessionIndexEntry> {
    let (cache, _) = reconcile_sessions_cache(GrokDiskCache::default(), session_dirs());
    save_disk_cache(&cache);
    let sessions = sessions_from_cache(&cache);
    *sessions_cache().lock() = Some(cache);
    sessions
}

fn projects_from_sessions(sessions: Vec<SessionIndexEntry>) -> Vec<ProjectEntry> {
    let mut grouped: BTreeMap<String, Vec<SessionIndexEntry>> = BTreeMap::new();
    for entry in sessions {
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
                source: "grok".to_string(),
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
                is_virtual: id == UNROOTED_PROJECT,
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

fn changed_session_dirs(paths: &[PathBuf]) -> HashSet<PathBuf> {
    paths
        .iter()
        .filter_map(|path| {
            let file_name = path.file_name().and_then(|name| name.to_str());
            if file_name == Some("summary.json") || file_name == Some(CHAT_HISTORY_FILE) {
                path.parent().map(Path::to_path_buf)
            } else if path.join("summary.json").exists() || path.join(CHAT_HISTORY_FILE).exists() {
                Some(path.clone())
            } else {
                None
            }
        })
        .collect()
}

/// Update only changed Grok session directories when the shared snapshot is
/// already warm. A cold snapshot is reconciled lazily against the disk cache.
pub fn invalidate_paths(paths: &[PathBuf]) {
    let changed_dirs = changed_session_dirs(paths);
    if changed_dirs.is_empty() {
        return;
    }

    let snapshot = {
        let mut state = sessions_cache().lock();
        let Some(cache) = state.as_mut() else {
            return;
        };
        for dir in changed_dirs {
            let key = dir.to_string_lossy().into_owned();
            cache.sessions_by_dir.remove(&key);
            if let Some(session) = cached_session_for_dir(&dir) {
                cache.sessions_by_dir.insert(key, session);
            }
        }
        cache.clone()
    };
    save_disk_cache(&snapshot);
}

pub fn get_sessions(project_id: &str) -> Result<Vec<SessionIndexEntry>, String> {
    let mut sessions: Vec<_> = load_all_sessions()
        .into_iter()
        .filter(|entry| entry.cwd.as_deref().unwrap_or(UNROOTED_PROJECT) == project_id)
        .collect();
    sessions.sort_by(|left, right| right.modified.cmp(&left.modified));
    Ok(sessions)
}

pub fn refresh_sessions_cache(project_id: &str) -> Result<Vec<SessionIndexEntry>, String> {
    // Project refresh and file watchers rebuild/invalidate the shared snapshot.
    // Reuse it here so one frontend refresh cycle never parses every history twice.
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

    invalidate_sessions_cache();

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

    fn write_session(dir: &Path, id: &str, cwd: &str, text: Option<&str>) {
        fs::create_dir_all(dir).unwrap();
        let summary = serde_json::json!({
            "info": { "id": id, "cwd": cwd },
            "created_at": "2026-08-27T00:00:00Z",
            "updated_at": "2026-08-27T00:00:00Z"
        });
        fs::write(dir.join("summary.json"), summary.to_string()).unwrap();
        let history = text
            .map(|text| serde_json::json!({ "type": "user", "content": text }).to_string())
            .unwrap_or_default();
        fs::write(dir.join(CHAT_HISTORY_FILE), history).unwrap();
    }

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

    #[test]
    fn project_count_excludes_empty_sessions() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ai-session-viewer-grok-project-count-{}-{unique}",
            std::process::id()
        ));
        let empty = root.join("empty");
        let valid = root.join("valid");
        let cwd = r"C:\projects\grok-count-test";
        write_session(&empty, "empty", cwd, None);
        write_session(&valid, "valid", cwd, Some("有效会话"));

        let projects = projects_from_sessions(vec![
            session_entry(&empty).unwrap(),
            session_entry(&valid).unwrap(),
        ]);

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].session_count, 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reconcile_reuses_unchanged_sessions_and_rescans_only_changes() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ai-session-viewer-grok-cache-{}-{unique}",
            std::process::id()
        ));
        let first = root.join("first");
        let second = root.join("second");
        write_session(&first, "first", r"C:\projects\first", Some("one"));
        write_session(&second, "second", r"C:\projects\second", Some("two"));
        let dirs = vec![first.clone(), second.clone()];

        let (cache, changed) = reconcile_sessions_cache(GrokDiskCache::default(), dirs.clone());
        assert!(changed);
        assert_eq!(cache.sessions_by_dir.len(), 2);

        let mut scans = 0;
        let (cache, changed) = reconcile_sessions_cache_with(cache, dirs.clone(), |dir| {
            scans += 1;
            cached_session_for_dir(dir)
        });
        assert!(!changed);
        assert_eq!(scans, 0);

        fs::write(
            first.join(CHAT_HISTORY_FILE),
            serde_json::json!({ "type": "user", "content": "changed" }).to_string(),
        )
        .unwrap();
        filetime::set_file_mtime(
            first.join(CHAT_HISTORY_FILE),
            filetime::FileTime::from_unix_time(2_000_000_000, 0),
        )
        .unwrap();

        scans = 0;
        let (cache, changed) = reconcile_sessions_cache_with(cache, dirs, |dir| {
            scans += 1;
            cached_session_for_dir(dir)
        });
        assert!(changed);
        assert_eq!(scans, 1);

        fs::remove_dir_all(&second).unwrap();
        scans = 0;
        let (cache, changed) = reconcile_sessions_cache_with(cache, vec![first], |dir| {
            scans += 1;
            cached_session_for_dir(dir)
        });
        assert!(changed);
        assert_eq!(scans, 0);
        assert_eq!(cache.sessions_by_dir.len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
