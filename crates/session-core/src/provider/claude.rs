use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use parking_lot::Mutex;
use rayon::prelude::*;

use crate::models::message::{DisplayMessage, PaginatedMessages};
use crate::models::project::ProjectEntry;
use crate::models::session::{
    SessionIndexEntry, SessionStatus, SessionsIndex, SessionsIndexFileEntry,
};
use crate::parser::jsonl as claude_parser;
use crate::scan_progress::{self, Phase};
use crate::parser::path_encoder::{
    decode_project_path_validated, get_projects_dir, short_name_from_path,
};
use crate::state::{clear_message_cache, clear_message_cache_for_path};

#[derive(serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum DeleteLevel {
    SessionOnly,
    WithCcConfig,
    /// Level 3：清理 history.jsonl。保留后端实现，前端 UI 不暴露。
    WithHistory,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
    pub sessions_deleted: usize,
    pub config_cleaned: bool,
    pub bookmarks_removed: usize,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ProjectMeta {
    alias: Option<String>,
}

// v3 (2026-05): cache 现在存"全分类"会话（Valid + Empty + Corrupt 都在同一个
// Vec 里，按 status 字段区分）。v2 只存 Valid，反序列化后 get_invalid_sessions
// 拿不到 Empty/Corrupt 会误以为没有；直接 bump 版本号让老缓存被丢弃重扫。
const CACHE_VERSION: u32 = 3;

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct ClaudeCacheFile {
    version: u32,
    #[serde(default)]
    projects: Option<CachedProjects>,
    #[serde(default)]
    sessions_by_project: HashMap<String, CachedSessions>,
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

fn cache_path() -> Option<PathBuf> {
    let dir = dirs::config_dir()?.join("ai-session-viewer");
    let _ = fs::create_dir_all(&dir);
    Some(dir.join("claude-list-cache.json"))
}

fn read_cache_from_disk() -> ClaudeCacheFile {
    let path = match cache_path() {
        Some(path) => path,
        None => return ClaudeCacheFile::default(),
    };

    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return ClaudeCacheFile::default(),
    };

    let cache: ClaudeCacheFile = match serde_json::from_str(&content) {
        Ok(cache) => cache,
        Err(_) => return ClaudeCacheFile::default(),
    };

    if cache.version != CACHE_VERSION {
        return ClaudeCacheFile::default();
    }

    cache
}

fn cache_state() -> &'static Mutex<ClaudeCacheFile> {
    static CACHE_STATE: OnceLock<Mutex<ClaudeCacheFile>> = OnceLock::new();
    CACHE_STATE.get_or_init(|| Mutex::new(read_cache_from_disk()))
}

fn load_cache() -> ClaudeCacheFile {
    cache_state().lock().clone()
}

fn save_cache(cache: &ClaudeCacheFile) {
    if let Some(path) = cache_path() {
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
        .map(|projects| projects.entries.clone())
}

fn store_projects_cache(entries: &[ProjectEntry]) {
    let cache = {
        let mut cache = cache_state().lock();
        cache.version = CACHE_VERSION;
        cache.projects = Some(CachedProjects {
            entries: entries.to_vec(),
        });
        cache.clone()
    };
    save_cache(&cache);
}

fn cached_sessions(project_id: &str) -> Option<Vec<SessionIndexEntry>> {
    cache_state()
        .lock()
        .sessions_by_project
        .get(project_id)
        .map(|sessions| sessions.entries.clone())
}

fn store_sessions_cache(project_id: &str, entries: &[SessionIndexEntry]) {
    let cache = {
        let mut cache = cache_state().lock();
        cache.version = CACHE_VERSION;
        cache.sessions_by_project.insert(
            project_id.to_string(),
            CachedSessions {
                entries: entries.to_vec(),
            },
        );
        cache.clone()
    };
    save_cache(&cache);
}

pub fn invalidate_cache() {
    *cache_state().lock() = ClaudeCacheFile::default();
    clear_message_cache();
    if let Some(path) = cache_path() {
        let _ = fs::remove_file(path);
    }
}

/// Surgically invalidate the cache for a single project (`project_id` is the
/// encoded directory name under `~/.claude/projects/`).
///
/// Unlike [`invalidate_cache`], this keeps every *other* project's cached
/// `session_count` intact, so the next `get_projects` re-aggregation reuses
/// them (see `scan_projects_from_disk`) and only deep-scans the one changed
/// project. This is what keeps app startup fast for active users — touching
/// one session file no longer forces a full re-scan of every project.
///
/// The `projects` list is dropped (forcing a cheap re-aggregation pass: one
/// `read_dir` + index/metadata read per project, no per-session-file scan for
/// unchanged projects), and only the changed project's session list is dropped.
pub fn invalidate_project(project_id: &str) {
    let cache = {
        let mut cache = cache_state().lock();
        cache.sessions_by_project.remove(project_id);
        cache.projects = None;
        cache.clone()
    };
    save_cache(&cache);
}

/// Map a changed path to its encoded project id (the first directory component
/// under `~/.claude/projects/`). `None` if the path isn't under `projects_dir`.
fn project_id_from_path(path: &Path, projects_dir: &Path) -> Option<String> {
    let rel = path.strip_prefix(projects_dir).ok()?;
    match rel.components().next()? {
        Component::Normal(name) => name.to_str().map(|s| s.to_string()),
        _ => None,
    }
}

/// Incrementally update the cache for a set of changed paths (called by the fs
/// watcher). When all of a project's changed paths are `.jsonl` files and that
/// project's session list is already cached, only those files are re-scanned
/// and spliced into the cached list — so appending a message to one session no
/// longer forces a re-scan of every session file in the project. Anything that
/// can't be handled surgically (a `sessions-index.json` change, an unmapped
/// path, or a cold project) falls back to the existing project- / full-level
/// invalidation.
pub fn invalidate_paths(changed: &[PathBuf]) {
    let projects_dir = match get_projects_dir() {
        Some(dir) => dir,
        None => {
            invalidate_cache();
            return;
        }
    };

    let mut by_project: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for path in changed {
        if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
            clear_message_cache_for_path(path);
        }
        match project_id_from_path(path, &projects_dir) {
            Some(pid) => by_project.entry(pid).or_default().push(path.clone()),
            // Unmappable path (e.g. the projects dir itself) → be safe.
            None => {
                invalidate_cache();
                return;
            }
        }
    }

    let mut touched = false;
    for (pid, paths) in by_project {
        let all_jsonl = paths
            .iter()
            .all(|p| p.extension().map(|e| e == "jsonl").unwrap_or(false));

        match (all_jsonl, cached_sessions(&pid)) {
            (true, Some(mut entries)) => {
                let project_dir = projects_dir.join(&pid);
                for p in &paths {
                    let Some(sid) = p.file_stem().and_then(|s| s.to_str()).map(String::from) else {
                        continue;
                    };
                    // Replace (or drop) the entry for this file; re-scan if it
                    // still exists.
                    entries.retain(|e| e.session_id != sid);
                    if let Some(updated) = scan_single_session_file(p.clone()) {
                        entries.push(updated);
                    }
                }
                // Re-apply sessions-index.json metadata + re-sort, matching a
                // full refresh — but without touching unchanged files.
                enrich_with_index(&project_dir, &mut entries);
                entries.sort_by(|a, b| b.modified.cmp(&a.modified));
                store_sessions_cache(&pid, &entries);
                touched = true;
            }
            _ => {
                // Non-jsonl change (e.g. sessions-index.json) or no warm cache:
                // drop just this project.
                invalidate_project(&pid);
                touched = true;
            }
        }
    }

    if touched {
        // Project session counts / last_modified may have shifted; force a
        // cheap re-aggregation (reuses the still-warm per-project caches).
        let snapshot = {
            let mut cache = cache_state().lock();
            cache.projects = None;
            cache.clone()
        };
        save_cache(&snapshot);
    }
}

fn project_path_from_index(index: &SessionsIndex) -> Option<String> {
    index
        .original_path
        .clone()
        .or_else(|| index.entries.iter().find_map(|entry| entry.project_path.clone()))
}

fn read_sessions_index(project_dir: &Path) -> Option<SessionsIndex> {
    let index_path = project_dir.join("sessions-index.json");
    fs::read_to_string(&index_path)
        .ok()
        .and_then(|content| serde_json::from_str::<SessionsIndex>(&content).ok())
}

/// 读取项目别名。文件不存在或 alias 字段缺失时返回 Ok(None)，不报错。
/// 使用 canonicalize + starts_with 防止路径遍历。
pub fn get_project_alias(project_id: &str) -> Result<Option<String>, String> {
    let projects_dir = get_projects_dir()
        .ok_or_else(|| "Cannot find Claude projects directory".to_string())?;
    let project_dir = projects_dir.join(project_id);
    if !project_dir.exists() {
        return Ok(None);
    }
    // 防止路径遍历：规范化后验证仍在 projects_dir 内
    let canonical_dir = project_dir
        .canonicalize()
        .map_err(|e| format!("Failed to resolve project path: {}", e))?;
    let canonical_base = projects_dir
        .canonicalize()
        .map_err(|e| format!("Failed to resolve projects directory: {}", e))?;
    if !canonical_dir.starts_with(&canonical_base) {
        return Ok(None); // 路径逃逸，静默返回无别名
    }
    let meta_path = canonical_dir.join(".project-meta.json");
    if !meta_path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&meta_path)
        .map_err(|e| format!("Failed to read project meta: {}", e))?;
    let meta: ProjectMeta = serde_json::from_str(&content)
        .unwrap_or_default();
    Ok(meta.alias)
}

/// 写入别名。
///
/// - alias 为 Some(s)：写入 / 更新 .project-meta.json 中的 alias 字段
/// - alias 为 None：删除 alias 字段；若文件其余字段为空（{}）则删除文件
///
/// 使用 canonicalize + starts_with 防止路径遍历。
pub fn set_project_alias(project_id: &str, alias: Option<String>) -> Result<(), String> {
    let projects_dir = get_projects_dir()
        .ok_or_else(|| "Cannot find Claude projects directory".to_string())?;
    let project_dir = projects_dir.join(project_id);

    if !project_dir.exists() {
        return Err(format!("Project not found: {}", project_id));
    }
    let canonical_dir = project_dir
        .canonicalize()
        .map_err(|e| format!("Failed to resolve project path: {}", e))?;
    let canonical_base = projects_dir
        .canonicalize()
        .map_err(|e| format!("Failed to resolve projects directory: {}", e))?;
    if !canonical_dir.starts_with(&canonical_base) {
        return Err(format!("Invalid project id: {}", project_id));
    }

    let meta_path = canonical_dir.join(".project-meta.json");

    let mut raw: serde_json::Map<String, serde_json::Value> = if meta_path.exists() {
        let content = fs::read_to_string(&meta_path)
            .map_err(|e| format!("Failed to read project meta: {}", e))?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        serde_json::Map::new()
    };

    match alias {
        Some(a) => {
            let a = a.trim().to_string();
            if a.is_empty() {
                raw.remove("alias");
            } else {
                raw.insert("alias".to_string(), serde_json::Value::String(a));
            }
        }
        None => {
            raw.remove("alias");
        }
    }

    if raw.is_empty() {
        if meta_path.exists() {
            fs::remove_file(&meta_path)
                .map_err(|e| format!("Failed to remove project meta: {}", e))?;
        }
    } else {
        let json = serde_json::to_string(&raw)
            .map_err(|e| format!("Failed to serialize project meta: {}", e))?;
        let tmp_path = meta_path.with_extension("json.tmp");
        fs::write(&tmp_path, json)
            .map_err(|e| format!("Failed to write project meta tmp: {}", e))?;
        fs::rename(&tmp_path, &meta_path)
            .map_err(|e| format!("Failed to rename project meta: {}", e))?;
    }

    invalidate_cache();
    Ok(())
}

fn scan_projects_from_disk(projects_dir: &Path) -> Result<Vec<ProjectEntry>, String> {
    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    let cache = std::sync::Arc::new(load_cache());

    // Collect all project directory entries first (fast, sequential)
    let dir_entries: Vec<PathBuf> = fs::read_dir(projects_dir)
        .map_err(|e| format!("Failed to read projects dir: {}", e))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();

    // Parallel processing: read index.json + count files for each project dir
    let progress = scan_progress::begin(Phase::Projects, dir_entries.len() as u64);
    let mut projects: Vec<ProjectEntry> = dir_entries
        .into_par_iter()
        .filter_map(|path| {
            let result = (|| {
            let encoded_name = path.file_name().and_then(|n| n.to_str())?.to_string();

            let parsed_index = read_sessions_index(&path);

            let (display_path, path_exists) = parsed_index
                .as_ref()
                .and_then(project_path_from_index)
                .map(|p| {
                    let exists = std::path::Path::new(&p).exists();
                    (p, exists)
                })
                .unwrap_or_else(|| {
                    let decoded = decode_project_path_validated(&encoded_name);
                    (decoded.display_path, decoded.path_exists)
                });
            let short_name = short_name_from_path(&display_path);

            let fast_count = count_jsonl_files_fast(&path);
            if fast_count == 0 {
                return None;
            }

            let session_count = cache
                .sessions_by_project
                .get(&encoded_name)
                .map(|cached| {
                    cached
                        .entries
                        .iter()
                        .filter(|entry| entry.status == SessionStatus::Valid)
                        .count()
                })
                .unwrap_or_else(|| count_valid_jsonl_files(&path));
            if session_count == 0 {
                return None;
            }

            let last_modified = fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .map(|t| {
                    let duration = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                    chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default()
                });

            let alias = get_project_alias(&encoded_name).unwrap_or(None);
            Some(ProjectEntry {
                source: "claude".to_string(),
                id: encoded_name,
                display_path,
                short_name,
                session_count,
                last_modified,
                model_provider: None,
                alias,
                path_exists,
                is_virtual: false,
            })
            })();
            scan_progress::inc(progress);
            result
        })
        .collect();
    scan_progress::finish(progress);

    projects.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
    Ok(projects)
}

pub fn refresh_projects_cache() -> Result<Vec<ProjectEntry>, String> {
    let projects_dir = get_projects_dir().ok_or("Could not find Claude projects directory")?;
    let projects = scan_projects_from_disk(&projects_dir)?;
    store_projects_cache(&projects);
    Ok(projects)
}

/// Get all Claude projects
pub fn get_projects() -> Result<Vec<ProjectEntry>, String> {
    if let Some(projects) = cached_projects() {
        return Ok(projects);
    }

    refresh_projects_cache()
}

/// 扫描并写入"全分类"会话缓存（Valid + Empty + Corrupt 都在内）。
///
/// **API 契约变化（v3）**：返回的 Vec 不再 filter，**包含所有 status**。
/// 前端按 `entry.status` 自行 filter：
/// - 主列表：filter `status === "valid"`
/// - 损坏会话横幅 / 空会话清理：filter `status !== "valid"`
///
/// 这样把过去 SessionsPage 同时打两个 RPC（getSessions + getInvalidSessions）
/// 合并成一个 RPC，IO 减半，**避免冷缓存下两个调用各自启动一次完整扫描**。
pub fn refresh_sessions_cache(encoded_name: &str) -> Result<Vec<SessionIndexEntry>, String> {
    let projects_dir = get_projects_dir().ok_or("Could not find Claude projects directory")?;
    let project_dir = projects_dir.join(encoded_name);

    if !project_dir.exists() {
        return Err(format!("Project directory not found: {}", encoded_name));
    }

    // 并行扫描全部 .jsonl，按 SessionStatus 分类返回。
    let mut all_sessions = scan_project_dir_all(&project_dir);

    // index 元数据补充（可选，失败时静默跳过）
    enrich_with_index(&project_dir, &mut all_sessions);

    all_sessions.sort_by(|a, b| b.modified.cmp(&a.modified));
    store_sessions_cache(encoded_name, &all_sessions);
    Ok(all_sessions)
}

/// 返回项目的全部会话（Valid + Empty + Corrupt 都在内）。
/// 由前端按 `status` 字段分别渲染主列表 / 异常项 banner。
pub fn get_sessions(encoded_name: &str) -> Result<Vec<SessionIndexEntry>, String> {
    if let Some(sessions) = cached_sessions(encoded_name) {
        return Ok(sessions);
    }
    refresh_sessions_cache(encoded_name)
}

/// 返回异常项清理页用的会话（仅 Empty + Corrupt）。
///
/// 与 `get_sessions` 共享同一份全分类缓存。若缓存已存在则不再读盘，
/// 否则触发一次 `refresh_sessions_cache` 然后从结果里 filter。这是
/// `InvalidItemsPage` 每个项目独立调用的入口。
pub fn get_invalid_sessions(encoded_name: &str) -> Result<Vec<SessionIndexEntry>, String> {
    let all = get_sessions(encoded_name)?;
    Ok(all
        .into_iter()
        .filter(|s| s.status != SessionStatus::Valid)
        .collect())
}


/// Parse messages from a Claude JSONL file
pub fn parse_session_messages(
    path: &std::path::Path,
    page: usize,
    page_size: usize,
    from_end: bool,
) -> Result<PaginatedMessages, String> {
    claude_parser::parse_session_messages(path, page, page_size, from_end)
}

/// Load `[start, end)` slice for the windowed message view.
pub fn parse_messages_range(
    path: &std::path::Path,
    start: usize,
    end: usize,
) -> Result<crate::models::message::RangeMessages, String> {
    claude_parser::parse_messages_range(path, start, end)
}

/// Parse all messages (for search)
pub fn parse_all_messages(path: &std::path::Path) -> Result<Vec<DisplayMessage>, String> {
    claude_parser::parse_all_messages(path)
}

/// Collect all JSONL files for search
pub fn collect_all_jsonl_files() -> Vec<(String, String, PathBuf)> {
    let projects_dir = match get_projects_dir() {
        Some(d) if d.exists() => d,
        _ => return Vec::new(),
    };

    let mut files: Vec<(String, String, PathBuf)> = Vec::new();

    let project_dirs = match fs::read_dir(&projects_dir) {
        Ok(d) => d,
        Err(_) => return files,
    };

    for entry in project_dirs.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let encoded_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        // Prefer originalPath from sessions-index.json (same as get_projects)
        let index_path = path.join("sessions-index.json");
        let display_path = fs::read_to_string(&index_path)
            .ok()
            .and_then(|c| serde_json::from_str::<SessionsIndex>(&c).ok())
            .and_then(|idx| {
                idx.original_path
                    .or_else(|| idx.entries.iter().find_map(|e| e.project_path.clone()))
            })
            .unwrap_or_else(|| decode_project_path_validated(&encoded_name).display_path);
        let project_name = short_name_from_path(&display_path);

        if let Ok(dir_files) = fs::read_dir(&path) {
            for file_entry in dir_files.flatten() {
                let file_path = file_entry.path();
                if file_path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                    files.push((encoded_name.clone(), project_name.clone(), file_path));
                }
            }
        }
    }

    files
}

// ── internal helpers ──

/// 把 SystemTime 转成 RFC3339 字符串。给 created/modified 字段用。
fn fmt_system_time(t: std::time::SystemTime) -> String {
    let d = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

/// 并行扫描项目目录下所有 .jsonl，返回全分类（Valid + Empty + Corrupt）的
/// SessionIndexEntry 列表。调用方按 entry.status 自己 filter。
///
/// 性能：先 collect 出目录条目，再走 rayon `into_par_iter` —— 每个文件的
/// `scan_session_file_once`（读盘+解析）独立无依赖，按 CPU 核数并行。
/// 8 核机器上 100 个文件大约 8x 加速；磁盘 IO 是上限。
fn scan_project_dir_all(project_dir: &Path) -> Vec<SessionIndexEntry> {
    let entries: Vec<PathBuf> = match fs::read_dir(project_dir) {
        Ok(e) => e.flatten().map(|e| e.path()).collect(),
        Err(_) => return Vec::new(),
    };

    let progress = scan_progress::begin(Phase::Sessions, entries.len() as u64);
    let result = entries
        .into_par_iter()
        .filter_map(|p| {
            let entry = scan_single_session_file(p);
            scan_progress::inc(progress);
            entry
        })
        .collect();
    scan_progress::finish(progress);
    result
}

/// 扫描单个文件，返回带 status 的 entry；不是合法 .jsonl 或无法读取返回 None。
fn scan_single_session_file(path: PathBuf) -> Option<SessionIndexEntry> {
    if !path.is_file() {
        return None;
    }
    let ext = path.extension()?;
    if ext != "jsonl" {
        return None;
    }
    let session_id = path.file_stem().and_then(|s| s.to_str())?.to_string();
    if session_id.is_empty() {
        return None;
    }

    let file_meta = fs::metadata(&path).ok();
    let file_size = file_meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let modified = file_meta
        .as_ref()
        .and_then(|m| m.modified().ok())
        .map(fmt_system_time);
    let created = file_meta
        .as_ref()
        .and_then(|m| m.created().ok())
        .map(fmt_system_time);

    if file_size == 0 {
        // 0 字节文件直接归 Empty，不用打开
        return Some(SessionIndexEntry {
            source: "claude".to_string(),
            session_id,
            file_path: path.to_string_lossy().to_string(),
            first_prompt: None,
            thread_name: None,
            message_count: 0,
            created,
            modified,
            git_branch: None,
            project_path: None,
            is_sidechain: Some(false),
            cwd: None,
            model_provider: None,
            cli_version: None,
            alias: None,
            tags: None,
            status: SessionStatus::Empty,
        });
    }

    let scan = claude_parser::scan_session_file_once(&path)?;
    let status = if !scan.has_messages {
        SessionStatus::Empty
    } else if scan.is_corrupt {
        SessionStatus::Corrupt
    } else {
        SessionStatus::Valid
    };

    Some(SessionIndexEntry {
        source: "claude".to_string(),
        session_id,
        file_path: path.to_string_lossy().to_string(),
        first_prompt: scan.first_prompt,
        thread_name: None,
        message_count: scan.message_count,
        created,
        modified,
        git_branch: scan.git_branch,
        project_path: scan.project_path,
        is_sidechain: Some(false),
        cwd: None,
        model_provider: None,
        cli_version: None,
        alias: scan.custom_title,
        tags: None,
        status,
    })
}

/// Step 2：用 sessions-index.json 的元数据补充 valid_sessions。
/// 若 index 不存在或解析失败，直接返回，不影响 valid_sessions。
fn enrich_with_index(project_dir: &Path, sessions: &mut [SessionIndexEntry]) {
    let index_path = project_dir.join("sessions-index.json");
    let content = match fs::read_to_string(&index_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let index: SessionsIndex = match serde_json::from_str(&content) {
        Ok(i) => i,
        Err(_) => return,
    };

    let original_path = index.original_path.clone();

    // 构建 index_map: session_id -> IndexEntry
    let index_map: std::collections::HashMap<&str, &SessionsIndexFileEntry> = index
        .entries
        .iter()
        .map(|e| (e.session_id.as_str(), e))
        .collect();

    for session in sessions.iter_mut() {
        if let Some(idx_entry) = index_map.get(session.session_id.as_str()) {
            // 用 index 的时间戳覆盖（更准确）
            if let Some(ref created) = idx_entry.created {
                session.created = Some(created.clone());
            }
            if let Some(ref modified) = idx_entry.modified {
                session.modified = Some(modified.clone());
            }
            // 补充 git_branch（Step 1 未提取到时）
            if session.git_branch.is_none() {
                session.git_branch = idx_entry.git_branch.clone();
            }
            // 补充 is_sidechain
            if session.is_sidechain.is_none() {
                session.is_sidechain = idx_entry.is_sidechain;
            }
            // idx_entry.project_path 优先
            if session.project_path.is_none() {
                session.project_path = idx_entry.project_path.clone();
            }
        }
        // original_path 作为最终兜底（index 中没有该 session 或 idx_entry.project_path 为空时）
        if session.project_path.is_none() {
            session.project_path = original_path.clone();
        }
    }
}

/// 删除项目目录，并根据 level 执行额外清理。
pub fn delete_project(project_id: &str, level: DeleteLevel) -> Result<DeleteResult, String> {
    // 安全检查：project_id 不能为空、"."、".."
    if project_id.is_empty() || project_id == "." || project_id == ".." {
        return Err(format!("Invalid project id: {}", project_id));
    }

    let projects_dir = get_projects_dir()
        .ok_or_else(|| "Cannot find Claude projects directory".to_string())?;
    let dir = projects_dir.join(project_id);
    if !dir.exists() {
        return Err(format!("Project not found: {}", project_id));
    }

    // 防路径遍历
    let canonical_dir = dir
        .canonicalize()
        .map_err(|e| format!("Failed to resolve project path: {}", e))?;
    let canonical_base = projects_dir
        .canonicalize()
        .map_err(|e| format!("Failed to resolve projects directory: {}", e))?;
    if !canonical_dir.starts_with(&canonical_base) {
        return Err(format!("Invalid project id: {}", project_id));
    }

    // Level 2+：删除目录前先内联读取 real_path（sessions-index.json 在目录内，必须先读）
    let project_path = read_sessions_index(&canonical_dir)
        .as_ref()
        .and_then(project_path_from_index)
        .unwrap_or_else(|| decode_project_path_validated(project_id).display_path);
    let real_path = if level == DeleteLevel::WithCcConfig || level == DeleteLevel::WithHistory {
        Some(project_path.clone())
    } else {
        None
    };

    // 统计 session 文件数（回收站将使用此计数）
    let sessions_deleted = count_valid_jsonl_files(&canonical_dir);

    // 将顶层 .jsonl 文件逐个移入回收站
    let project_name = short_name_from_path(&project_path);

    if let Ok(dir_entries) = fs::read_dir(&canonical_dir) {
        for entry in dir_entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.extension().map(|e| e == "jsonl").unwrap_or(false) {
                if let Err(e) = crate::recyclebin::move_to_recyclebin(
                    &p,
                    "project",
                    "ManualDelete",
                    "claude",
                    project_id,
                    None,
                    Some(project_name.clone()),
                ) {
                    eprintln!("[delete_project] Failed to move {:?} to recyclebin: {}", p, e);
                }
            }
        }
    }

    // 删除（现已清空 jsonl 的）项目目录
    if let Err(e) = fs::remove_dir_all(&canonical_dir) {
        // 可能还有非 jsonl 文件，或目录不为空，静默记录
        eprintln!("[delete_project] Failed to remove dir {:?}: {}", canonical_dir, e);
    }

    let mut config_cleaned = false;
    let mut bookmarks_removed = 0;

    if level == DeleteLevel::WithCcConfig || level == DeleteLevel::WithHistory {
        // Level 2a：清理 ~/.claude.json
        config_cleaned = clean_claude_config(real_path.as_deref().unwrap_or(""));

        // Level 2b：清理书签
        bookmarks_removed = clean_bookmarks_for_project(project_id);
    }
    // Level 3 (WithHistory) 的 history.jsonl 清理逻辑预留此处，本次不实现具体功能。

    invalidate_cache();

    Ok(DeleteResult {
        sessions_deleted,
        config_cleaned,
        bookmarks_removed,
    })
}

/// 从 ~/.claude.json 中删除 projects[real_path] key。
/// real_path 反查失败时静默返回 false（不报错）。
fn clean_claude_config(real_path: &str) -> bool {
    if real_path.is_empty() {
        return false;
    }

    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return false,
    };
    let config_path = home.join(".claude.json");
    if !config_path.exists() {
        return false;
    }

    let content = match fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let mut json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };

    // 找到 projects 对象并删除对应 key
    let removed = if let Some(projects) = json.get_mut("projects").and_then(|p| p.as_object_mut()) {
        projects.remove(real_path).is_some()
    } else {
        false
    };

    if !removed {
        return false;
    }

    // 原子写回
    let new_content = match serde_json::to_string_pretty(&json) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let tmp_path = config_path.with_extension("json.tmp");
    if fs::write(&tmp_path, &new_content).is_err() {
        return false;
    }
    fs::rename(&tmp_path, &config_path).is_ok()
}

/// 从 ~/.session-viewer-bookmarks.json 删除 project_id 匹配的书签。
/// 返回删除数量，失败时返回 0（静默）。
fn clean_bookmarks_for_project(project_id: &str) -> usize {
    use crate::bookmarks::load_bookmarks;

    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return 0,
    };
    let path = home.join(".session-viewer-bookmarks.json");

    let mut file = load_bookmarks();
    let before = file.bookmarks.len();
    file.bookmarks.retain(|b| b.project_id != project_id);
    let removed = before - file.bookmarks.len();

    if removed == 0 {
        return 0;
    }

    // 原子写回
    let json = match serde_json::to_string_pretty(&file) {
        Ok(j) => j,
        Err(_) => return 0,
    };
    let tmp_path = path.with_extension("json.tmp");
    if fs::write(&tmp_path, &json).is_err() {
        return 0;
    }
    if fs::rename(&tmp_path, &path).is_err() {
        return 0;
    }
    removed
}

/// 扫描所有项目目录，将孤儿 UUID 子目录批量移入回收站。
/// 返回成功移入的数量。
pub fn cleanup_all_orphan_dirs() -> Result<usize, String> {
    let projects_dir = get_projects_dir()
        .ok_or_else(|| "Cannot find Claude projects directory".to_string())?;
    if !projects_dir.exists() {
        return Ok(0);
    }

    let project_dirs: Vec<PathBuf> = fs::read_dir(&projects_dir)
        .map_err(|e| format!("Failed to read projects dir: {}", e))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();

    let mut moved = 0usize;

    for project_dir in project_dirs {
        let project_id = match project_dir.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // 收集本项目中已有的 valid session ID
        let valid_ids: std::collections::HashSet<String> = match fs::read_dir(&project_dir) {
            Ok(entries) => entries
                .flatten()
                .filter_map(|e| {
                    let p = e.path();
                    if p.is_file() && p.extension().map(|ext| ext == "jsonl").unwrap_or(false) {
                        p.file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect(),
            Err(_) => continue,
        };

        // 查找孤儿 UUID 子目录
        if let Ok(entries) = fs::read_dir(&project_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let dir_name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                // UUID 格式：36 字符，含 4 个连字符；跳过最近 5 分钟内修改的（可能是 CC subagent 正在使用）
                let recently_modified = fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .and_then(|t| std::time::SystemTime::now().duration_since(t).map_err(|e| std::io::Error::other(e.to_string())))
                    .map(|d| d.as_secs() < 300)
                    .unwrap_or(true);
                if dir_name.len() == 36
                    && dir_name.chars().filter(|&c| c == '-').count() == 4
                    && !valid_ids.contains(&dir_name)
                    && !recently_modified
                {
                    match crate::recyclebin::move_to_recyclebin(
                        &path,
                        "orphanDir",
                        "OrphanDir",
                        "claude",
                        &project_id,
                        None,
                        None,
                    ) {
                        Ok(_) => moved += 1,
                        Err(e) => eprintln!("[cleanup_orphan] Failed to move {:?}: {}", path, e),
                    }
                }
            }
        }
    }

    Ok(moved)
}

/// Fast project-list count: count top-level `.jsonl` files only.
/// Does not inspect file contents, so it may differ from the filtered session list.
fn count_jsonl_files_fast(dir: &Path) -> usize {
    fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| {
                    let p = e.path();
                    p.is_file()
                        && p.extension().map(|ext| ext == "jsonl").unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

/// Count valid session files at the TOP LEVEL of dir.
/// Does NOT descend into subdirectories.
/// A valid session must be non-empty, contain messages, and not be corrupt.
fn count_valid_jsonl_files(dir: &Path) -> usize {
    fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| {
                    let p = e.path();
                    p.is_file()
                        && p.extension().map(|ext| ext == "jsonl").unwrap_or(false)
                        && e.metadata().map(|m| m.len() > 0).unwrap_or(false)
                        && claude_parser::scan_session_file_once(&p)
                            .map(|scan| scan.has_messages && !scan.is_corrupt)
                            .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}
