use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use chrono_tz::Tz;
use parking_lot::Mutex;
use rayon::prelude::*;
use serde_json::Value;

use crate::models::message::{DisplayContentBlock, DisplayMessage, PaginatedMessages};
use crate::models::project::ProjectEntry;
use crate::models::session::{SessionIndexEntry, SessionStatus};
use crate::models::stats::{DailyTokenEntry, TokenUsageSummary};
use crate::state::{
    clear_message_cache, clear_message_cache_for_path, file_modified_key, get_cached_full_messages,
    get_cached_page, paginate_from_range, store_full_messages, store_partial_messages,
    tail_window_len,
};

// Bumped to 3 in v2.12.1 to invalidate stale caches built by v2.12.0 and
// earlier — those builds' `extract_session_meta` only scanned the first 5
// JSONL lines and had a too-narrow `is_interactive` allowlist, so projects
// whose `session_meta` row landed past line 5 (or whose `source` field was
// anything other than "cli" / "vscode") silently dropped out of the project
// list. Bumping the version number forces a full rescan on first launch of
// 2.12.1 so the fix actually takes effect without users having to delete
// the cache file by hand.
// Bumped to 5: added the parallel `file_index` (path → session_meta + mtime)
// shared by the project list, session list and invalid-session scans, and
// dropped the old per-project session_count reconciliation. Old caches lack
// the index and use stale semantics, so force a clean rebuild.
// Bumped to 6: session list entries now carry `thread_name` read from
// `~/.codex/session_index.jsonl` (Codex Desktop's human-readable thread title,
// preferred over first_prompt for display). Old cached session lists predate
// the field, so force a rebuild to populate it on first launch of this build.
// Bumped to 7: `file_index` now stores `originator`, used to bucket Codex
// Desktop's synthetic per-chat scratch workspaces (`…/Codex/<date>/<slug>`)
// into a `<codex-direct>/DATE` "直连对话" virtual project. Old indexes lack the
// field, so force a rebuild.
// Bumped to 8: `file_index` now stores a nanosecond mtime key and is reconciled
// against the sessions directory once per process. This picks up rollout files
// created, changed or deleted while AI Session Viewer was not running.
const DISK_CACHE_VERSION: u32 = 8;

/// Sentinel prefix for project IDs synthesized from sessions with no cwd.
/// Format: `<codex-unrooted>/YYYY-MM-DD`. The angle brackets are invalid in
/// Windows paths, so this can't collide with a real cwd. The Tauri/Axum
/// layers pass project IDs through as opaque strings, so no plumbing changes
/// are required beyond the codex provider itself.
const UNROOTED_PREFIX: &str = "<codex-unrooted>/";

pub fn virtual_project_id(date: &str) -> String {
    format!("{UNROOTED_PREFIX}{date}")
}

pub fn parse_virtual_project_id(id: &str) -> Option<&str> {
    id.strip_prefix(UNROOTED_PREFIX)
}

/// Sentinel prefix for project IDs that bucket Codex Desktop's "direct chat"
/// sessions by date. Codex Desktop creates a throwaway scratch workspace per
/// chat at `…/Codex/<YYYY-MM-DD…>/<slug>`, so each chat would otherwise surface
/// as its own cryptically-named project ("app", "wo-2", "j", …). We collapse
/// them into one `<codex-direct>/YYYY-MM-DD` virtual project per day. Same
/// opaque-string handling as `UNROOTED_PREFIX`.
const DIRECT_PREFIX: &str = "<codex-direct>/";

pub fn direct_project_id(date: &str) -> String {
    format!("{DIRECT_PREFIX}{date}")
}

pub fn parse_direct_project_id(id: &str) -> Option<&str> {
    id.strip_prefix(DIRECT_PREFIX)
}

/// Leading `YYYY-MM-DD` of a path segment (Codex date folders look like
/// `2026-06-24` or `2026-04-22-new-chat`). `None` if it doesn't start with one.
fn leading_date(seg: &str) -> Option<String> {
    let b = seg.as_bytes();
    let ok = b.len() >= 10
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit);
    ok.then(|| seg[..10].to_string())
}

/// If this file is a Codex Desktop "direct chat" — `originator == "Codex
/// Desktop"` AND a cwd shaped like `…/Codex/<date>/…` (or `…/Codex/<date>`) —
/// return its date bucket. Real projects opened in Codex Desktop share the same
/// originator but don't live under the `Codex/<date>` scratch tree, so the path
/// shape is what disambiguates.
fn direct_chat_date(fm: &CodexFileMeta) -> Option<String> {
    if fm.originator.as_deref() != Some("Codex Desktop") || fm.cwd.is_empty() {
        return None;
    }
    let segs: Vec<&str> = fm.cwd.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
    for (i, seg) in segs.iter().enumerate() {
        if seg.eq_ignore_ascii_case("Codex") {
            if let Some(date) = segs.get(i + 1).and_then(|next| leading_date(next)) {
                return Some(date);
            }
        }
    }
    None
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct CodexDiskCache {
    version: u32,
    #[serde(default)]
    projects: Option<CachedProjects>,
    #[serde(default)]
    sessions_by_project: HashMap<String, CachedSessions>,
    /// One entry per rollout file (cheap `session_meta` + file timestamps),
    /// built once via a parallel scan and reused by the project list, session
    /// list and invalid-session scans — so they no longer each re-walk the
    /// whole `~/.codex/sessions` tree and re-open every file's meta.
    #[serde(default)]
    file_index: Option<Vec<CodexFileMeta>>,
}

/// Lightweight per-file record: everything `extract_session_meta` returns plus
/// the rollout path, its date bucket, and file timestamps. Lets the session
/// list be built by full-scanning only the files of the requested project,
/// without re-reading meta for unrelated files.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CodexFileMeta {
    path: String,
    #[serde(default)]
    modified_key: u64,
    id: String,
    cwd: String,
    cli_version: Option<String>,
    model_provider: Option<String>,
    git_branch: Option<String>,
    is_interactive: bool,
    #[serde(default)]
    originator: Option<String>,
    date: Option<String>,
    modified: Option<String>,
    created: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct CachedProjects {
    entries: Vec<ProjectEntry>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct CachedSessions {
    entries: Vec<SessionIndexEntry>,
}

fn disk_cache_path() -> Option<PathBuf> {
    let dir = dirs::config_dir()?.join("ai-session-viewer");
    let _ = fs::create_dir_all(&dir);
    Some(dir.join("codex-list-cache.json"))
}

fn read_disk_cache() -> CodexDiskCache {
    let path = match disk_cache_path() {
        Some(path) => path,
        None => return CodexDiskCache::default(),
    };

    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return CodexDiskCache::default(),
    };

    let mut cache: CodexDiskCache = match serde_json::from_str(&content) {
        Ok(cache) => cache,
        Err(_) => return CodexDiskCache::default(),
    };

    if cache.version != DISK_CACHE_VERSION {
        return CodexDiskCache::default();
    }

    if let Some(index) = cache.file_index.take() {
        let (index, changed) = reconcile_file_index(index, scan_all_session_files());
        cache.file_index = Some(index);
        if changed {
            cache.projects = None;
            cache.sessions_by_project.clear();
        }
    }

    cache
}

fn cache_state() -> &'static Mutex<CodexDiskCache> {
    static CACHE_STATE: OnceLock<Mutex<CodexDiskCache>> = OnceLock::new();
    CACHE_STATE.get_or_init(|| Mutex::new(read_disk_cache()))
}

fn save_disk_cache(cache: &CodexDiskCache) {
    if let Some(path) = disk_cache_path() {
        if let Ok(json) = serde_json::to_string(cache) {
            let tmp_path = path.with_extension("json.tmp");
            if fs::write(&tmp_path, json).is_ok() {
                let _ = fs::rename(&tmp_path, &path);
            }
        }
    }
}

fn cached_projects() -> Option<Vec<ProjectEntry>> {
    cache_state()
        .lock()
        .projects
        .as_ref()
        .map(|cached| cached.entries.clone())
}

fn store_projects(entries: &[ProjectEntry]) {
    let cache = {
        let mut cache = cache_state().lock();
        cache.version = DISK_CACHE_VERSION;
        cache.projects = Some(CachedProjects {
            entries: entries.to_vec(),
        });
        cache.clone()
    };
    save_disk_cache(&cache);
}

fn cached_project_sessions(cwd: &str) -> Option<Vec<SessionIndexEntry>> {
    cache_state()
        .lock()
        .sessions_by_project
        .get(cwd)
        .map(|cached| cached.entries.clone())
}

fn cached_file_index() -> Option<Vec<CodexFileMeta>> {
    cache_state().lock().file_index.clone()
}

fn store_file_index(index: &[CodexFileMeta]) {
    let cache = {
        let mut cache = cache_state().lock();
        cache.version = DISK_CACHE_VERSION;
        cache.file_index = Some(index.to_vec());
        cache.clone()
    };
    save_disk_cache(&cache);
}

fn store_project_sessions(cwd: &str, entries: &[SessionIndexEntry]) {
    let cache = {
        let mut cache = cache_state().lock();
        cache.version = DISK_CACHE_VERSION;
        cache.sessions_by_project.insert(
            cwd.to_string(),
            CachedSessions {
                entries: entries.to_vec(),
            },
        );
        cache.clone()
    };
    save_disk_cache(&cache);
}

fn clear_disk_cache() {
    if let Some(path) = disk_cache_path() {
        let _ = fs::remove_file(path);
    }
}

pub fn invalidate_sessions_cache() {
    *cache_state().lock() = CodexDiskCache::default();
    clear_message_cache();
    clear_disk_cache();
}

/// Maximum size for text content blocks sent to frontend (20KB)
const MAX_TEXT_BLOCK_SIZE: usize = 20_000;
/// Maximum size for tool output blocks sent to frontend (30KB)
const MAX_OUTPUT_BLOCK_SIZE: usize = 30_000;
/// Maximum size for function call arguments (10KB)
const MAX_ARGS_SIZE: usize = 10_000;

// ── Directory scanning ──

fn get_codex_home() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".codex")))
}

pub fn get_sessions_dir() -> Option<PathBuf> {
    get_codex_home().map(|h| h.join("sessions"))
}

/// `~/.codex/session_index.jsonl` — Codex Desktop's index of human-readable
/// thread titles, one JSON object per line: `{ "id", "thread_name", "updated_at" }`.
fn session_index_path() -> Option<PathBuf> {
    get_codex_home().map(|h| h.join("session_index.jsonl"))
}

/// Build a `session_id → thread_name` map from `session_index.jsonl`. Read once
/// per session-list build and looked up per entry. Missing file / blank titles
/// are simply absent from the map (callers fall back to `first_prompt`). Cheap:
/// one small file, line-by-line, no full rollout reads.
pub fn load_thread_names() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Some(path) = session_index_path() else {
        return map;
    };
    let Ok(file) = fs::File::open(&path) else {
        return map;
    };
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else { continue };
        let trimmed = line.trim_start_matches('\u{feff}').trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(row) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        let id = row.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let name = row
            .get("thread_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if !id.is_empty() && !name.is_empty() {
            // Last write wins: later lines in the index are newer.
            map.insert(id.to_string(), name.to_string());
        }
    }
    map
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

pub(crate) fn extract_date_from_path(path: &Path) -> Option<String> {
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
    /// Session source: "cli", "vscode", "exec", "mcp", or an object for "subagent".
    /// Only "cli" and "vscode" are interactive sessions that should be shown to users.
    pub is_interactive: bool,
    /// Who created the session, e.g. "Codex Desktop". Used to detect Codex
    /// Desktop's synthetic per-chat scratch workspaces (`…/Codex/<date>/<slug>`)
    /// so they can be bucketed into a "直连对话" virtual project by date.
    pub originator: Option<String>,
}

/// How far into a rollout file we'll scan looking for the `session_meta`
/// row. Most rollouts have it on line 1, but newer codex builds and the
/// app-server can prepend a handful of housekeeping rows. Reading a few
/// dozen lines up front is cheap and avoids "session is searchable but
/// missing from the project list" mismatches.
const SESSION_META_SCAN_LINES: usize = 50;

pub fn extract_session_meta(path: &Path) -> Option<SessionMeta> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines().take(SESSION_META_SCAN_LINES) {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        // Strip a UTF-8 BOM that can sneak in if a tool re-wrote the file
        // through a Windows text editor; serde_json would otherwise fail
        // on the leading `\u{feff}`.
        let trimmed = line.trim_start_matches('\u{feff}').trim();
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
                let originator = payload
                    .get("originator")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                // Use a *blocklist* rather than an allowlist for the
                // `source` field. Anything we know is non-interactive
                // (`exec`, `mcp`, the `subagent` object) gets hidden from
                // the project list, but new/unknown source names default
                // to "interactive" so the user still sees them. This
                // matches the global-search behaviour, which always
                // indexes the file — keeping the two views in sync.
                let is_interactive = match payload.get("source") {
                    Some(Value::String(s)) => {
                        let s = s.as_str();
                        s != "exec" && s != "mcp"
                    }
                    Some(Value::Object(_)) => false, // subagent object
                    _ => true, // missing / unknown shape → assume interactive
                };

                return Some(SessionMeta {
                    id,
                    cwd,
                    cli_version,
                    model_provider,
                    git_branch,
                    is_interactive,
                    originator,
                });
            }
        }
    }
    None
}

// ── Single-pass file scan ──

struct SessionFileScan {
    first_prompt: Option<String>,
    message_count: u32,
}

/// Read a session JSONL file once and extract meta + first_prompt + message count.
fn scan_session_file(path: &Path) -> SessionFileScan {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => {
            return SessionFileScan {
                first_prompt: None,
                message_count: 0,
            }
        }
    };
    let reader = BufReader::new(file);

    let mut first_prompt: Option<String> = None;
    let mut message_count: u32 = 0;
    let mut meta_found = false;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Fast path: count messages without JSON parsing
        if first_prompt.is_some() && meta_found {
            // Only need message count from here — use cheap string check
            if (trimmed.contains("\"type\":\"response_item\"")
                || trimmed.contains("\"type\": \"response_item\""))
                && (trimmed.contains("\"type\":\"message\"")
                    || trimmed.contains("\"type\": \"message\""))
                && !trimmed.contains("\"developer\"")
                && !trimmed.contains("\"system\"")
            {
                message_count += 1;
            }
            continue;
        }

        // Need to parse JSON for meta or first_prompt
        let row: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let row_type = row.get("type").and_then(|v| v.as_str()).unwrap_or("");

        if !meta_found && row_type == "session_meta" {
            meta_found = true;
            continue;
        }

        if row_type == "response_item" {
            if let Some(payload) = row.get("payload") {
                let payload_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if payload_type == "message" {
                    let role = payload.get("role").and_then(|v| v.as_str()).unwrap_or("");
                    if role != "developer" && role != "system" {
                        message_count += 1;
                        if first_prompt.is_none() && role == "user" {
                            if let Some(content) =
                                payload.get("content").and_then(|c| c.as_array())
                            {
                                for item in content {
                                    let item_type =
                                        item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                                    if item_type == "input_text" || item_type == "text" {
                                        if let Some(text) =
                                            item.get("text").and_then(|v| v.as_str())
                                        {
                                            if !text.is_empty() {
                                                first_prompt =
                                                    Some(truncate_string(text, 200));
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
        }
    }

    SessionFileScan {
        first_prompt,
        message_count,
    }
}

#[derive(Default)]
struct SessionVisibilityScan {
    first_prompt: Option<String>,
    message_count: u32,
    has_visible_activity: bool,
}

fn scan_session_visibility(path: &Path) -> SessionVisibilityScan {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return SessionVisibilityScan::default(),
    };
    let reader = BufReader::new(file);

    let mut scan = SessionVisibilityScan::default();

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
        if row.get("type").and_then(|v| v.as_str()) != Some("response_item") {
            continue;
        }

        let Some(payload) = row.get("payload") else {
            continue;
        };

        let payload_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match payload_type {
            "message" => {
                let role = payload.get("role").and_then(|v| v.as_str()).unwrap_or("");
                if role == "developer" || role == "system" {
                    continue;
                }

                if let Some(first_prompt) = extract_first_prompt_text(payload) {
                    if scan.first_prompt.is_none() && role == "user" {
                        scan.first_prompt = Some(truncate_string(&first_prompt, 200));
                    }
                }

                if !extract_message_content(payload).is_empty() {
                    scan.message_count += 1;
                    scan.has_visible_activity = true;
                }
            }
            "function_call"
                if payload
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|name| !name.trim().is_empty())
                    .unwrap_or(false) =>
            {
                scan.has_visible_activity = true;
            }
            "function_call_output"
                if payload
                    .get("output")
                    .map(value_has_visible_content)
                    .unwrap_or(false) =>
            {
                scan.has_visible_activity = true;
            }
            "reasoning"
                if extract_reasoning_text(payload)
                    .map(|text| !text.trim().is_empty())
                    .unwrap_or(false) =>
            {
                scan.has_visible_activity = true;
            }
            _ => {}
        }
    }

    scan
}

fn extract_first_prompt_text(payload: &Value) -> Option<String> {
    let content = payload.get("content")?.as_array()?;
    for item in content {
        let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if item_type == "input_text" || item_type == "text" {
            let text = item.get("text").and_then(|v| v.as_str())?;
            if !text.trim().is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn extract_reasoning_text(payload: &Value) -> Option<String> {
    let value = payload
        .get("text")
        .or_else(|| payload.get("summary").and_then(|summary| summary.get(0)))?;

    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }

    if let Some(arr) = value.as_array() {
        let text = arr
            .iter()
            .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<&str>>()
            .join("\n");
        return (!text.trim().is_empty()).then_some(text);
    }

    Some(value.to_string())
}

fn value_has_visible_content(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.trim().is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
        Value::Bool(_) | Value::Number(_) => true,
    }
}

fn systemtime_to_rfc3339(t: std::time::SystemTime) -> String {
    let d = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

/// Read one rollout file's `session_meta` + timestamps into an index entry.
/// `None` when the file has no `session_meta` row (or can't be read).
fn file_meta_for(path: &Path) -> Option<CodexFileMeta> {
    let meta = extract_session_meta(path)?;
    let file_meta = fs::metadata(path).ok();
    let modified = file_meta
        .as_ref()
        .and_then(|m| m.modified().ok())
        .map(systemtime_to_rfc3339);
    let created = file_meta
        .as_ref()
        .and_then(|m| m.created().ok())
        .map(systemtime_to_rfc3339);
    Some(CodexFileMeta {
        path: path.to_string_lossy().to_string(),
        modified_key: file_modified_key(path).unwrap_or_default(),
        id: meta.id,
        cwd: meta.cwd,
        cli_version: meta.cli_version,
        model_provider: meta.model_provider,
        git_branch: meta.git_branch,
        is_interactive: meta.is_interactive,
        originator: meta.originator,
        date: extract_date_from_path(path),
        modified,
        created,
    })
}

/// The project id (cache key) an indexed file belongs to:
/// - `<codex-direct>/DATE` for Codex Desktop direct-chat scratch workspaces,
/// - `<codex-unrooted>/DATE` when it has no recorded cwd,
/// - otherwise its real cwd.
fn project_key_for(fm: &CodexFileMeta) -> String {
    if let Some(date) = direct_chat_date(fm) {
        direct_project_id(&date)
    } else if fm.cwd.is_empty() {
        virtual_project_id(fm.date.as_deref().unwrap_or("unknown"))
    } else {
        fm.cwd.clone()
    }
}

/// Build the per-file index with a single parallel pass over every rollout
/// file: one `extract_session_meta` (≤50 lines) + one `fs::metadata` per file.
/// This is the only place that opens every file; the project/session lists are
/// then derived from this in-memory index.
fn build_file_index() -> Vec<CodexFileMeta> {
    let files = scan_all_session_files();
    crate::scan_progress::begin(crate::scan_progress::Phase::Index, files.len() as u64);
    let index = files
        .into_par_iter()
        .filter_map(|file_path| {
            let meta = file_meta_for(&file_path);
            crate::scan_progress::inc();
            meta
        })
        .collect();
    crate::scan_progress::finish();
    index
}

/// Reconcile a persisted index with the current rollout files. Unchanged files
/// keep their cached metadata; only new or modified files are opened again.
fn reconcile_file_index(
    cached: Vec<CodexFileMeta>,
    files: Vec<PathBuf>,
) -> (Vec<CodexFileMeta>, bool) {
    let cached_len = cached.len();
    let mut cached_by_path: HashMap<String, CodexFileMeta> = cached
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect();
    let mut changed = cached_by_path.len() != cached_len;
    let mut index = Vec::with_capacity(files.len());

    for path in files {
        let path_string = path.to_string_lossy().to_string();
        match cached_by_path.remove(&path_string) {
            Some(entry) if entry.modified_key == file_modified_key(&path).unwrap_or_default() => {
                index.push(entry);
            }
            Some(_) => {
                changed = true;
                if let Some(entry) = file_meta_for(&path) {
                    index.push(entry);
                }
            }
            None => {
                if let Some(entry) = file_meta_for(&path) {
                    changed = true;
                    index.push(entry);
                }
            }
        }
    }

    if !cached_by_path.is_empty() {
        changed = true;
    }

    (index, changed)
}

/// Incrementally update the index for changed rollout files (called by the fs
/// watcher) instead of wiping the whole Codex cache. Only the changed files are
/// re-read; the affected projects' session lists and the project aggregate are
/// dropped so they rebuild from the still-warm index — no full tree walk.
pub fn invalidate_paths(changed: &[PathBuf]) {
    let jsonl: Vec<&PathBuf> = changed
        .iter()
        .filter(|p| p.extension().map(|e| e == "jsonl").unwrap_or(false))
        .collect();
    for &p in &jsonl {
        clear_message_cache_for_path(p);
    }
    if jsonl.is_empty() {
        return;
    }

    // No warm index → nothing to preserve; drop derived caches so they rebuild
    // lazily (a single parallel scan), and we're done.
    if cache_state().lock().file_index.is_none() {
        let snapshot = {
            let mut cache = cache_state().lock();
            cache.projects = None;
            cache.sessions_by_project.clear();
            cache.clone()
        };
        save_disk_cache(&snapshot);
        return;
    }

    // Re-read changed files off-lock (file I/O), then splice under lock.
    let updates: Vec<(String, Option<CodexFileMeta>)> = jsonl
        .iter()
        .map(|&p| {
            let path_str = p.to_string_lossy().to_string();
            let new_meta = if p.exists() { file_meta_for(p) } else { None };
            (path_str, new_meta)
        })
        .collect();

    let snapshot = {
        let mut cache = cache_state().lock();
        let mut index = cache.file_index.take().unwrap_or_default();
        let mut affected: HashSet<String> = HashSet::new();
        for (path_str, new_meta) in &updates {
            if let Some(pos) = index.iter().position(|fm| &fm.path == path_str) {
                let old = index.remove(pos);
                affected.insert(project_key_for(&old));
            }
            if let Some(fm) = new_meta {
                affected.insert(project_key_for(fm));
                index.push(fm.clone());
            }
        }
        cache.file_index = Some(index);
        for key in &affected {
            cache.sessions_by_project.remove(key);
        }
        cache.projects = None;
        cache.clone()
    };
    save_disk_cache(&snapshot);
}

fn load_or_build_file_index() -> Vec<CodexFileMeta> {
    if let Some(index) = cached_file_index() {
        return index;
    }
    let index = build_file_index();
    store_file_index(&index);
    index
}

/// Whether an indexed file belongs to the given project id. Unified across all
/// three project kinds (real cwd, `<codex-unrooted>/DATE`, `<codex-direct>/DATE`)
/// by comparing the file's own derived bucket key against the requested id.
fn file_matches_project(fm: &CodexFileMeta, project_id: &str) -> bool {
    fm.is_interactive && project_key_for(fm) == project_id
}

/// Indexed files belonging to one project — the only files a session/invalid
/// scan needs to open, instead of re-walking the whole tree.
fn project_files(cwd: &str) -> Vec<CodexFileMeta> {
    load_or_build_file_index()
        .into_iter()
        .filter(|fm| file_matches_project(fm, cwd))
        // Self-heal stale index entries: a rollout deleted out from under us
        // (e.g. archived/removed in Codex desktop while the app was closed, or
        // a filesystem event the watcher missed) would otherwise linger here
        // and render as a "(无标题) · 0 条消息" ghost the user can't get rid of.
        // The callers already open every file they keep, so this extra `exists`
        // stat is negligible. Indexed paths are concrete files (no glob), so
        // `Path::exists` is the right check.
        .filter(|fm| Path::new(&fm.path).exists())
        .collect()
}

fn scan_projects_from_meta() -> Result<Vec<ProjectEntry>, String> {
    let index = load_or_build_file_index();
    let mut project_map: HashMap<String, ProjectEntry> = HashMap::new();

    for fm in &index {
        if !fm.is_interactive {
            continue;
        }

        // Drop entries whose rollout file has since vanished (archived/deleted
        // in Codex desktop, etc.) so a ghost project/session-count doesn't
        // outlive the file. Mirrors the self-heal in `project_files`; this scan
        // only runs on a cold/invalidated cache, so the extra stat is cheap.
        if !Path::new(&fm.path).exists() {
            continue;
        }

        // Bucketing (see `project_key_for`):
        // - Codex Desktop direct chats → one `<codex-direct>/DATE` per day,
        // - sessions with no cwd → `<codex-unrooted>/DATE` per day,
        // - real cwd sessions → the cwd itself.
        let project_key = project_key_for(fm);
        let (is_virtual, display_path, short_name, path_exists) =
            if let Some(date) = parse_direct_project_id(&project_key) {
                let display = format!("Codex 直连对话 · {date}");
                let short = format!("Codex 直连对话 · {date}");
                (true, display, short, true)
            } else if let Some(date) = parse_virtual_project_id(&project_key) {
                let display = format!("未归属会话 · {date}");
                let short = format!("未归属 · {date}");
                (true, display, short, true)
            } else {
                let display = fm.cwd.clone();
                let short = short_name_from_path(&fm.cwd);
                let exists = std::path::Path::new(&fm.cwd).exists();
                (false, display, short, exists)
            };

        let entry = project_map
            .entry(project_key.clone())
            .or_insert_with(|| ProjectEntry {
                source: "codex".to_string(),
                id: project_key.clone(),
                display_path,
                short_name,
                session_count: 0,
                last_modified: None,
                model_provider: fm.model_provider.clone(),
                alias: None,
                path_exists,
                is_virtual,
            });

        entry.session_count += 1;

        if let Some(ref modified) = fm.modified {
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

fn build_projects_cache() -> Result<Vec<ProjectEntry>, String> {
    let projects = scan_projects_from_meta()?;
    store_projects(&projects);
    Ok(projects)
}

/// Explicit deep refresh for the manual UI action. Background/watcher refreshes
/// must keep using `get_projects` after their incremental invalidation.
pub fn rebuild_projects_cache() -> Result<Vec<ProjectEntry>, String> {
    invalidate_sessions_cache();
    build_projects_cache()
}

pub fn get_projects() -> Result<Vec<ProjectEntry>, String> {
    if let Some(projects) = cached_projects() {
        return Ok(projects);
    }

    build_projects_cache()
}

/// 删除一个 Codex「项目」。Codex 的会话是散落在 `<year>/<month>/<day>/` 下的
/// 独立 rollout 文件，没有 Claude 那样的项目目录，所以删除即把该项目下每个
/// rollout 文件移入回收站（可还原），并清理对应会话元数据。
///
/// `project_id` 可以是真实 cwd，也可以是 `<codex-unrooted>/DATE` 虚拟桶。
/// 复用 [`super::claude::DeleteResult`] 作为统一返回类型（codex 无 cc 配置 /
/// 书签清理，对应字段恒为 false / 0）。
pub fn delete_project(project_id: &str) -> Result<super::claude::DeleteResult, String> {
    if project_id.is_empty() {
        return Err("Invalid project id".to_string());
    }

    let sessions = get_sessions(project_id)?;

    // 回收站条目展示用的项目名：直连/未归属虚拟桶用其日期描述，真实 cwd 取末段目录名。
    let project_name = if let Some(date) = parse_direct_project_id(project_id) {
        format!("Codex 直连对话 · {date}")
    } else if let Some(date) = parse_virtual_project_id(project_id) {
        format!("未归属 · {date}")
    } else {
        short_name_from_path(project_id)
    };

    let mut sessions_deleted = 0;
    for s in &sessions {
        let path = Path::new(&s.file_path);
        if !path.exists() {
            continue;
        }
        match crate::recyclebin::move_to_recyclebin(
            path,
            "project",
            "ManualDelete",
            "codex",
            project_id,
            None,
            Some(project_name.clone()),
        ) {
            Ok(_) => {
                sessions_deleted += 1;
                let _ = crate::metadata::remove_session_meta("codex", project_id, &s.session_id);
            }
            Err(e) => {
                eprintln!("[codex::delete_project] Failed to recycle {:?}: {}", path, e);
            }
        }
    }

    invalidate_sessions_cache();

    Ok(super::claude::DeleteResult {
        sessions_deleted,
        config_cleaned: false,
        bookmarks_removed: 0,
    })
}

pub fn refresh_sessions_cache(cwd: &str) -> Result<Vec<SessionIndexEntry>, String> {
    // Only the files belonging to this project (from the shared index) get
    // opened — no full tree walk, and no meta re-read (it's cached in the
    // index). Just the unavoidable full scan for first_prompt + message count.
    let thread_names = load_thread_names();
    let mut entries: Vec<SessionIndexEntry> = project_files(cwd)
        .into_par_iter()
        .map(|fm| {
            let scan = scan_session_file(Path::new(&fm.path));
            let thread_name = thread_names.get(&fm.id).cloned();
            SessionIndexEntry {
                source: "codex".to_string(),
                session_id: fm.id,
                file_path: fm.path,
                first_prompt: scan.first_prompt,
                thread_name,
                message_count: scan.message_count,
                created: fm.created,
                modified: fm.modified,
                git_branch: fm.git_branch,
                project_path: None,
                is_sidechain: None,
                cwd: Some(fm.cwd),
                model_provider: fm.model_provider,
                cli_version: fm.cli_version,
                alias: None,
                tags: None,
                status: SessionStatus::Valid,
            }
        })
        .collect();

    entries.sort_by(|a, b| b.modified.cmp(&a.modified));
    store_project_sessions(cwd, &entries);
    Ok(entries)
}

pub fn get_sessions(cwd: &str) -> Result<Vec<SessionIndexEntry>, String> {
    if let Some(entries) = cached_project_sessions(cwd) {
        return Ok(entries);
    }

    refresh_sessions_cache(cwd)
}

pub fn get_invalid_sessions(cwd: &str) -> Result<Vec<SessionIndexEntry>, String> {
    // Same project-file selection as the session list, but keep only files with
    // no visible activity (empty rollouts). Uses the shared index, so no full
    // tree walk and no meta re-read.
    let thread_names = load_thread_names();
    let mut entries: Vec<SessionIndexEntry> = project_files(cwd)
        .into_par_iter()
        .filter_map(|fm| {
            let scan = scan_session_visibility(Path::new(&fm.path));
            if scan.has_visible_activity {
                return None;
            }

            let thread_name = thread_names.get(&fm.id).cloned();
            Some(SessionIndexEntry {
                source: "codex".to_string(),
                session_id: fm.id,
                file_path: fm.path,
                first_prompt: scan.first_prompt,
                thread_name,
                message_count: scan.message_count,
                created: fm.created,
                modified: fm.modified,
                git_branch: fm.git_branch,
                project_path: None,
                is_sidechain: None,
                cwd: Some(fm.cwd),
                model_provider: fm.model_provider,
                cli_version: fm.cli_version,
                alias: None,
                tags: None,
                status: SessionStatus::Empty,
            })
        })
        .collect();

    entries.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(entries)
}

// ── Message parsing ──

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

pub fn parse_all_messages(path: &Path) -> Result<Vec<DisplayMessage>, String> {
    if let Ok(Some(cached)) = get_cached_full_messages(path) {
        return Ok(cached);
    }

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
        if let Some(message) = display_message_from_row(&row) {
            messages.push(message);
        }
    }

    let _ = store_full_messages(path, &messages);
    Ok(messages)
}

/// Load `[start, end)` slice for the windowed message view. Mirrors the
/// claude implementation: serve from the partial-range cache when
/// possible, otherwise full-parse + memoize.
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

fn parse_tail_messages(path: &Path, page: usize, page_size: usize) -> Result<PaginatedMessages, String> {
    let file = fs::File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let reader = BufReader::new(file);
    let window_len = tail_window_len(page, page_size);
    let mut tail_messages: VecDeque<DisplayMessage> = VecDeque::with_capacity(window_len);
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

        let row: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(message) = display_message_from_row(&row) {
            total += 1;
            if tail_messages.len() == window_len {
                tail_messages.pop_front();
            }
            tail_messages.push_back(message);
        }
    }

    let messages: Vec<DisplayMessage> = tail_messages.into_iter().collect();
    let range_start = total.saturating_sub(messages.len());
    let _ = store_partial_messages(path, total, range_start, &messages);

    paginate_from_range(&messages, total, page, page_size, true, range_start)
        .ok_or_else(|| "Failed to paginate tail messages".to_string())
}

fn display_message_from_row(row: &Value) -> Option<DisplayMessage> {
    let row_type = row.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if row_type != "response_item" {
        return None;
    }

    let timestamp = row.get("timestamp").and_then(|v| v.as_str()).map(String::from);
    let payload = row.get("payload")?;
    let payload_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match payload_type {
        "message" => {
            let role = payload
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if role == "developer" || role == "system" {
                return None;
            }
            if role != "user" && role != "assistant" {
                return None;
            }

            let content_blocks = extract_message_content(payload);
            if content_blocks.is_empty() {
                return None;
            }

            Some(DisplayMessage {
                uuid: None,
                parent_uuid: None,
                role: role.to_string(),
                timestamp,
                model: None,
                content: content_blocks,
            })
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

            Some(DisplayMessage {
                uuid: None,
                parent_uuid: None,
                role: "assistant".to_string(),
                timestamp,
                model: None,
                content: vec![DisplayContentBlock::FunctionCall {
                    name,
                    arguments: truncate_string(&arguments, MAX_ARGS_SIZE),
                    call_id,
                }],
            })
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

            Some(DisplayMessage {
                uuid: None,
                parent_uuid: None,
                role: "tool".to_string(),
                timestamp,
                model: None,
                content: vec![DisplayContentBlock::FunctionCallOutput {
                    call_id,
                    output: truncate_string(&output, MAX_OUTPUT_BLOCK_SIZE),
                }],
            })
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

            if text.is_empty() {
                return None;
            }

            Some(DisplayMessage {
                uuid: None,
                parent_uuid: None,
                role: "assistant".to_string(),
                timestamp,
                model: None,
                content: vec![DisplayContentBlock::Reasoning { text }],
            })
        }
        _ => None,
    }
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

/// Extract per-turn token deltas from a Codex rollout. The rollout emits
/// cumulative `total_token_usage` after each turn, so the per-turn cost is
/// the delta between consecutive events. The first event's delta equals its
/// own totals (everything up to that turn).
fn extract_token_events(path: &Path) -> Vec<TokenEvent> {
    let Ok(file) = fs::File::open(path) else {
        return Vec::new();
    };
    let reader = BufReader::new(file);
    let mut events: Vec<TokenEvent> = Vec::new();
    let mut prev_input: u64 = 0;
    let mut prev_output: u64 = 0;

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

        if row.get("type").and_then(|v| v.as_str()) != Some("event_msg") {
            continue;
        }

        let Some(payload) = row.get("payload") else {
            continue;
        };
        if payload.get("type").and_then(|v| v.as_str()) != Some("token_count") {
            continue;
        }

        // Prefer the per-turn `last_token_usage` field when present; otherwise
        // derive deltas from cumulative totals.
        let timestamp = row
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let info = payload.get("info");
        let last = info.and_then(|i| i.get("last_token_usage"));
        let total = info.and_then(|i| i.get("total_token_usage"));

        let (delta_input, delta_output) = if let Some(last) = last {
            (
                last.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                last.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
            )
        } else if let Some(total) = total {
            let cur_input = total
                .get("input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let cur_output = total
                .get("output_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let di = cur_input.saturating_sub(prev_input);
            let do_ = cur_output.saturating_sub(prev_output);
            prev_input = cur_input;
            prev_output = cur_output;
            (di, do_)
        } else {
            continue;
        };

        if delta_input == 0 && delta_output == 0 {
            continue;
        }

        events.push(TokenEvent {
            timestamp,
            input_tokens: delta_input,
            output_tokens: delta_output,
        });
    }

    events
}

struct TokenEvent {
    timestamp: String,
    input_tokens: u64,
    output_tokens: u64,
}

/// Collect per-turn request records across all rollouts. Codex doesn't
/// expose cache_read/cache_creation streams (those are baked into
/// `input_tokens` server-side), so those fields are 0 here and the cache
/// hit-rate chart simply has no Codex line.
pub fn collect_requests() -> Result<Vec<crate::models::stats::RequestRecord>, String> {
    use crate::models::pricing;
    use crate::models::stats::RequestRecord;

    let files = scan_all_session_files();
    let mut all: Vec<RequestRecord> = Vec::new();

    for file_path in files {
        let meta = extract_session_meta(&file_path);
        let session_id = meta
            .as_ref()
            .map(|m| m.id.clone())
            .unwrap_or_default();
        let project_id = meta
            .as_ref()
            .map(|m| {
                if m.cwd.is_empty() {
                    let date = extract_date_from_path(&file_path)
                        .unwrap_or_else(|| "unknown".to_string());
                    virtual_project_id(&date)
                } else {
                    m.cwd.clone()
                }
            })
            .unwrap_or_default();
        let model = meta
            .as_ref()
            .and_then(|m| m.model_provider.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let file_path_str = file_path.to_string_lossy().into_owned();
        let events = extract_token_events(&file_path);
        for ev in events {
            let cost = pricing::compute_cost(&model, ev.input_tokens, 0, 0, ev.output_tokens);
            let total = ev.input_tokens + ev.output_tokens;
            all.push(RequestRecord {
                timestamp: ev.timestamp.clone(),
                source: "codex".to_string(),
                project_id: project_id.clone(),
                session_id: session_id.clone(),
                file_path: file_path_str.clone(),
                model: model.clone(),
                input_tokens: ev.input_tokens,
                output_tokens: ev.output_tokens,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
                total_tokens: total,
                cost_usd: cost,
                is_priced: pricing::is_priced(&model),
                duration_ms: None,
                message_uuid: None,
            });
        }
    }

    Ok(all)
}

// ── Stats ──

pub fn get_stats(time_zone: Tz) -> Result<TokenUsageSummary, String> {
    use crate::models::pricing;

    let files = scan_all_session_files();
    let mut summary = crate::stats::empty_summary();
    let mut tokens_by_model: HashMap<String, u64> = HashMap::new();
    let mut cost_by_model: HashMap<String, f64> = HashMap::new();
    let mut daily_map: HashMap<String, DailyAccum> = HashMap::new();
    let mut unpriced_models: HashSet<String> = HashSet::new();
    let mut session_count: u64 = 0;

    for file_path in &files {
        session_count += 1;

        let meta = extract_session_meta(file_path);
        let model_provider = meta
            .as_ref()
            .and_then(|m| m.model_provider.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let session_date = extract_date_from_path(file_path);
        let events = extract_token_events(file_path);
        for ev in events {
            let cost =
                pricing::compute_cost(&model_provider, ev.input_tokens, 0, 0, ev.output_tokens);
            summary.total_input_tokens += ev.input_tokens;
            summary.total_output_tokens += ev.output_tokens;
            summary.total_tokens += ev.input_tokens + ev.output_tokens;
            summary.total_cost_usd += cost;
            summary.message_count += 1;
            *tokens_by_model.entry(model_provider.clone()).or_insert(0) +=
                ev.input_tokens + ev.output_tokens;
            *cost_by_model.entry(model_provider.clone()).or_insert(0.0) += cost;
            let priced = pricing::is_priced(&model_provider);
            if !priced {
                unpriced_models.insert(model_provider.clone());
            }

            let date = crate::stats::date_in_time_zone(&ev.timestamp, time_zone)
                .or_else(|| session_date.clone());
            if let Some(date) = date {
                let entry = daily_map.entry(date).or_default();
                entry.messages += 1;
                entry.input += ev.input_tokens;
                entry.output += ev.output_tokens;
                entry.cost += cost;
                *entry
                    .tokens_by_model
                    .entry(model_provider.clone())
                    .or_insert(0) += ev.input_tokens + ev.output_tokens;
                *entry
                    .cost_by_model
                    .entry(model_provider.clone())
                    .or_insert(0.0) += cost;
                if !priced {
                    entry.unpriced_models.insert(model_provider.clone());
                }
            }
        }
    }

    let mut daily_tokens: Vec<DailyTokenEntry> = daily_map
        .into_iter()
        .map(|(date, b)| DailyTokenEntry {
            date,
            input_tokens: b.input,
            output_tokens: b.output,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            total_tokens: b.input + b.output,
            cost_usd: b.cost,
            tokens_by_model: b.tokens_by_model,
            cost_by_model: b.cost_by_model,
            unpriced_models: {
                let mut models: Vec<String> = b.unpriced_models.into_iter().collect();
                models.sort();
                models
            },
            message_count: b.messages,
            cache_hit_ratio_by_model: HashMap::new(),
        })
        .collect();
    daily_tokens.sort_by(|a, b| a.date.cmp(&b.date));

    summary.tokens_by_model = tokens_by_model;
    summary.cost_by_model = cost_by_model;
    summary.unpriced_models = unpriced_models.into_iter().collect();
    summary.unpriced_models.sort();
    summary.daily_tokens = daily_tokens;
    summary.session_count = session_count;
    Ok(summary)
}

#[derive(Default, Clone)]
struct DailyAccum {
    input: u64,
    output: u64,
    cost: f64,
    messages: u64,
    tokens_by_model: HashMap<String, u64>,
    cost_by_model: HashMap<String, f64>,
    unpriced_models: HashSet<String>,
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{}...", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_rollout(path: &Path, id: &str, cwd: &str) {
        let row = serde_json::json!({
            "type": "session_meta",
            "payload": {
                "id": id,
                "cwd": cwd,
                "source": "cli"
            }
        });
        fs::write(path, format!("{row}\n")).unwrap();
    }

    #[test]
    fn reconcile_adds_rollout_created_while_viewer_was_closed() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ai-session-viewer-codex-reconcile-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();

        let existing = dir.join("rollout-existing.jsonl");
        write_rollout(&existing, "existing", r"C:\projects\existing");
        let cached = vec![file_meta_for(&existing).unwrap()];

        let added = dir.join("rollout-added.jsonl");
        write_rollout(&added, "added", r"C:\Users\zuolan\Desktop\CCS2KEIL");
        let added_path = added.to_string_lossy().into_owned();

        let (index, changed) = reconcile_file_index(cached, vec![existing, added]);

        assert!(changed);
        assert_eq!(index.len(), 2);
        assert!(index.iter().any(|entry| {
            entry.path == added_path && entry.cwd == r"C:\Users\zuolan\Desktop\CCS2KEIL"
        }));

        fs::remove_dir_all(dir).unwrap();
    }
}
