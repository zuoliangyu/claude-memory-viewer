use std::fs;
use std::path::PathBuf;

use crate::models::message::{DisplayMessage, PaginatedMessages};
use crate::models::project::ProjectEntry;
use crate::models::session::{SessionIndexEntry, SessionsIndex, SessionsIndexFileEntry};
use crate::parser::jsonl as claude_parser;
use crate::parser::path_encoder::{decode_project_path, get_projects_dir, short_name_from_path};

/// Get all Claude projects
pub fn get_projects() -> Result<Vec<ProjectEntry>, String> {
    let projects_dir = get_projects_dir().ok_or("Could not find Claude projects directory")?;

    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    let mut projects: Vec<ProjectEntry> = Vec::new();

    let entries =
        fs::read_dir(&projects_dir).map_err(|e| format!("Failed to read projects dir: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let encoded_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        // Prefer originalPath from sessions-index.json for accurate display
        let display_path = {
            let index_path = path.join("sessions-index.json");
            fs::read_to_string(&index_path)
                .ok()
                .and_then(|c| serde_json::from_str::<SessionsIndex>(&c).ok())
                .and_then(|idx| idx.original_path)
                .unwrap_or_else(|| decode_project_path(&encoded_name))
        };
        let short_name = short_name_from_path(&display_path);

        let session_count = fs::read_dir(&path)
            .map(|rd| {
                rd.flatten()
                    .filter(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "jsonl")
                            .unwrap_or(false)
                    })
                    .count()
            })
            .unwrap_or(0);

        let last_modified = fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .map(|t| {
                let duration = t
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default();
                chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default()
            });

        if session_count > 0 {
            projects.push(ProjectEntry {
                source: "claude".to_string(),
                id: encoded_name,
                display_path,
                short_name,
                session_count,
                last_modified,
                model_provider: None,
            });
        }
    }

    projects.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
    Ok(projects)
}

/// Get sessions for a Claude project
pub fn get_sessions(encoded_name: &str) -> Result<Vec<SessionIndexEntry>, String> {
    let projects_dir = get_projects_dir().ok_or("Could not find Claude projects directory")?;
    let project_dir = projects_dir.join(encoded_name);

    if !project_dir.exists() {
        return Err(format!("Project directory not found: {}", encoded_name));
    }

    // Collect all .jsonl files on disk: session_id -> path
    let mut disk_sessions: std::collections::HashMap<String, PathBuf> =
        std::collections::HashMap::new();
    if let Ok(dir_entries) = fs::read_dir(&project_dir) {
        for entry in dir_entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                if let Some(session_id) = path.file_stem().and_then(|s| s.to_str()) {
                    if !session_id.is_empty() {
                        disk_sessions.insert(session_id.to_string(), path);
                    }
                }
            }
        }
    }

    // Try reading sessions-index.json
    let index_path = project_dir.join("sessions-index.json");
    if index_path.exists() {
        let content = fs::read_to_string(&index_path)
            .map_err(|e| format!("Failed to read sessions index: {}", e))?;
        let index: SessionsIndex = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse sessions index: {}", e))?;

        if !index.entries.is_empty() {
            let original_path = index.original_path.clone();

            // Collect indexed session IDs
            let indexed_ids: std::collections::HashSet<String> =
                index.entries.iter().map(|e| e.session_id.clone()).collect();

            // Start with index entries, fill missing project_path from original_path
            let mut entries: Vec<SessionIndexEntry> = index
                .entries
                .into_iter()
                .map(|e| {
                    let mut entry = convert_index_entry(e, &project_dir);
                    if entry.project_path.is_none() {
                        entry.project_path = original_path.clone();
                    }
                    entry
                })
                .collect();

            // Find sessions on disk but missing from index, scan them individually
            for (session_id, path) in &disk_sessions {
                if !indexed_ids.contains(session_id) {
                    if let Some(mut entry) = scan_single_session(path, session_id) {
                        if entry.project_path.is_none() {
                            entry.project_path = original_path.clone();
                        }
                        entries.push(entry);
                    }
                }
            }

            entries.sort_by(|a, b| b.modified.cmp(&a.modified));
            entries.retain(|e| e.message_count > 0);
            return Ok(entries);
        }
    }

    // Fallback: scan JSONL files directly
    scan_sessions_from_dir(&project_dir)
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

        let display_path = decode_project_path(&encoded_name);
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

fn convert_index_entry(e: SessionsIndexFileEntry, project_dir: &std::path::Path) -> SessionIndexEntry {
    let file_path = e
        .full_path
        .clone()
        .unwrap_or_else(|| {
            project_dir
                .join(format!("{}.jsonl", e.session_id))
                .to_string_lossy()
                .to_string()
        });

    SessionIndexEntry {
        source: "claude".to_string(),
        session_id: e.session_id,
        file_path,
        first_prompt: e.first_prompt,
        message_count: e.message_count.unwrap_or(0),
        created: e.created,
        modified: e.modified,
        git_branch: e.git_branch,
        project_path: e.project_path,
        is_sidechain: e.is_sidechain,
        cwd: None,
        model_provider: None,
        cli_version: None,
    }
}

fn scan_sessions_from_dir(project_dir: &std::path::Path) -> Result<Vec<SessionIndexEntry>, String> {
    let mut entries: Vec<SessionIndexEntry> = Vec::new();

    let dir_entries =
        fs::read_dir(project_dir).map_err(|e| format!("Failed to read project dir: {}", e))?;

    for entry in dir_entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
            let session_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            if session_id.is_empty() {
                continue;
            }

            if let Some(entry) = scan_single_session(&path, &session_id) {
                entries.push(entry);
            }
        }
    }

    entries.sort_by(|a, b| b.modified.cmp(&a.modified));
    entries.retain(|e| e.message_count > 0);
    Ok(entries)
}

fn scan_single_session(path: &std::path::Path, session_id: &str) -> Option<SessionIndexEntry> {
    let first_prompt = claude_parser::extract_first_prompt(path);
    let metadata = claude_parser::extract_session_metadata(path);
    let (_, git_branch, project_path) = metadata.unwrap_or((String::new(), None, None));
    let message_count = count_messages(path);

    let file_meta = fs::metadata(path).ok();
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

    Some(SessionIndexEntry {
        source: "claude".to_string(),
        session_id: session_id.to_string(),
        file_path: path.to_string_lossy().to_string(),
        first_prompt,
        message_count,
        created,
        modified,
        git_branch,
        project_path,
        is_sidechain: Some(false),
        cwd: None,
        model_provider: None,
        cli_version: None,
    })
}

fn count_messages(path: &std::path::Path) -> u32 {
    use std::io::{BufRead, BufReader};
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return 0,
    };
    let reader = BufReader::new(file);
    let mut count: u32 = 0;
    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.contains("\"type\":\"user\"") || trimmed.contains("\"type\":\"assistant\"") {
            count += 1;
        }
    }
    count
}
