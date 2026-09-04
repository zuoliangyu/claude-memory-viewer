use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::parser::path_encoder::get_projects_dir;
use crate::provider::codex;

/// Per-session metadata (alias + tags)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// The metadata file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataFile {
    pub version: u32,
    pub sessions: HashMap<String, SessionMeta>,
}

impl Default for MetadataFile {
    fn default() -> Self {
        Self {
            version: 1,
            sessions: HashMap::new(),
        }
    }
}

/// Resolve the metadata file path for a given source and project
fn metadata_path(source: &str, project_id: &str) -> Option<PathBuf> {
    match source {
        "claude" => {
            resolve_claude_project_dir(project_id).map(|dir| dir.join(".session-viewer-meta.json"))
        }
        "codex" => {
            let codex_home = codex::get_sessions_dir()?.parent()?.to_path_buf();
            Some(codex_home.join(".session-viewer-meta.json"))
        }
        _ => None,
    }
}

fn is_single_normal_component(value: &str) -> bool {
    let mut components = Path::new(value).components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

/// Reject session ids that could escape a project directory or otherwise be
/// abused to address arbitrary files on disk. Real session ids are UUIDs (or
/// codex thread ids); anything containing a path separator, `..`, NUL, or an
/// outright empty string is bogus and likely an attack.
pub fn validate_session_id(session_id: &str) -> Result<(), String> {
    if session_id.is_empty() {
        return Err("session_id is empty".to_string());
    }
    if session_id.len() > 256 {
        return Err("session_id is too long".to_string());
    }
    if session_id.contains('/')
        || session_id.contains('\\')
        || session_id.contains('\0')
        || session_id == "."
        || session_id == ".."
        || session_id.starts_with("..")
    {
        return Err(format!("invalid session_id: {}", session_id));
    }
    if !is_single_normal_component(session_id) {
        return Err(format!("invalid session_id: {}", session_id));
    }
    Ok(())
}

fn resolve_claude_project_dir(project_id: &str) -> Option<PathBuf> {
    if !is_single_normal_component(project_id) {
        return None;
    }

    let projects_dir = get_projects_dir()?;
    let canonical_base = fs::canonicalize(&projects_dir).ok()?;
    let project_dir = projects_dir.join(project_id);
    if !project_dir.exists() {
        return None;
    }

    let canonical_dir = fs::canonicalize(project_dir).ok()?;
    if !canonical_dir.is_dir() {
        return None;
    }
    let relative = canonical_dir.strip_prefix(&canonical_base).ok()?;

    if relative.components().count() != 1 {
        return None;
    }

    Some(canonical_dir)
}

/// Load metadata file; returns default if not found
pub fn load_metadata(source: &str, project_id: &str) -> MetadataFile {
    let path = match metadata_path(source, project_id) {
        Some(p) => p,
        None => return MetadataFile::default(),
    };

    if !path.exists() {
        return MetadataFile::default();
    }

    fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

/// Save metadata file (atomic: write tmp + rename)
pub fn save_metadata(source: &str, project_id: &str, meta: &MetadataFile) -> Result<(), String> {
    let path = metadata_path(source, project_id)
        .ok_or_else(|| "Cannot resolve metadata path".to_string())?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create metadata directory: {}", e))?;
    }

    let content =
        serde_json::to_string_pretty(meta).map_err(|e| format!("Failed to serialize: {}", e))?;

    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, content).map_err(|e| format!("Failed to write tmp: {}", e))?;
    fs::rename(&tmp_path, &path).map_err(|e| format!("Failed to rename: {}", e))?;

    Ok(())
}

/// Update metadata for a single session
pub fn update_session_meta(
    source: &str,
    project_id: &str,
    session_id: &str,
    alias: Option<String>,
    tags: Vec<String>,
) -> Result<(), String> {
    let mut meta = load_metadata(source, project_id);

    // If both alias and tags are empty, remove the entry
    if alias.is_none() && tags.is_empty() {
        meta.sessions.remove(session_id);
    } else {
        meta.sessions
            .insert(session_id.to_string(), SessionMeta { alias, tags });
    }

    save_metadata(source, project_id, &meta)
}

/// Rename a chat session's alias by project path (used by /rename in ChatInput).
///
/// For Claude: encodes the project_path → encoded directory name, finds
/// `<projects>/<encoded>/<session_id>.jsonl`, and appends a custom-title record
/// the same way the metadata editor does. Existing tags are preserved.
///
/// For Codex: writes the alias to the global metadata file. Existing tags
/// are preserved.
///
/// Returns the encoded project_id used so callers can invalidate caches.
pub fn rename_chat_session(
    source: &str,
    project_path: &str,
    session_id: &str,
    new_alias: Option<&str>,
) -> Result<String, String> {
    // Reject any session_id that could escape the project directory before
    // it's joined into a filesystem path.
    validate_session_id(session_id)?;

    let trimmed = new_alias
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    match source {
        "claude" => {
            let encoded = crate::parser::path_encoder::encode_project_path(project_path);
            let projects_dir = crate::parser::path_encoder::get_projects_dir()
                .ok_or_else(|| "Cannot resolve Claude projects directory".to_string())?;
            let project_dir = projects_dir.join(&encoded);
            if !project_dir.is_dir() {
                return Err(format!(
                    "Claude project directory not found for path: {}",
                    project_path
                ));
            }
            let jsonl_path = project_dir.join(format!("{}.jsonl", session_id));
            if !jsonl_path.exists() {
                return Err(format!("Session file not found: {}", jsonl_path.display()));
            }
            crate::parser::jsonl::append_custom_title(&jsonl_path, session_id, trimmed.as_deref())?;

            let existing = load_metadata("claude", &encoded);
            let tags = existing
                .sessions
                .get(session_id)
                .map(|s| s.tags.clone())
                .unwrap_or_default();
            update_session_meta("claude", &encoded, session_id, None, tags)?;
            Ok(encoded)
        }
        "codex" => {
            let existing = load_metadata("codex", "");
            let tags = existing
                .sessions
                .get(session_id)
                .map(|s| s.tags.clone())
                .unwrap_or_default();
            update_session_meta("codex", project_path, session_id, trimmed, tags)?;
            Ok(project_path.to_string())
        }
        _ => Err(format!("Unknown source: {}", source)),
    }
}

/// Remove metadata for a single session
pub fn remove_session_meta(source: &str, project_id: &str, session_id: &str) -> Result<(), String> {
    let mut meta = load_metadata(source, project_id);
    if meta.sessions.remove(session_id).is_some() {
        save_metadata(source, project_id, &meta)?;
    }
    Ok(())
}

/// Get all unique tags used in a project (for autocomplete)
pub fn get_all_tags(source: &str, project_id: &str) -> Vec<String> {
    let meta = load_metadata(source, project_id);
    let mut tags: Vec<String> = meta
        .sessions
        .values()
        .flat_map(|s| s.tags.iter().cloned())
        .collect::<std::collections::HashSet<String>>()
        .into_iter()
        .collect();
    tags.sort();
    tags
}

/// Get tags for all projects across the given source.
/// Returns a map: project_id (encoded_name for Claude, "" for Codex) → deduplicated sorted tags.
///
/// Performance: for Claude, this used to read each project's
/// `.session-viewer-meta.json` sequentially. With many projects (50+) the
/// cumulative IO latency could stall the search page mount for several
/// seconds. We now collect the encoded project names first and read the
/// metadata files in parallel via rayon — disk IO scales with the
/// available rayon thread count, which usually means a 4-8x speedup on
/// SSDs and even more on NVMe.
pub fn get_all_cross_project_tags(source: &str) -> HashMap<String, Vec<String>> {
    match source {
        "claude" => {
            let projects_dir = match get_projects_dir() {
                Some(d) if d.exists() => d,
                _ => return HashMap::new(),
            };

            let encoded_names: Vec<String> = match fs::read_dir(&projects_dir) {
                Ok(entries) => entries
                    .flatten()
                    .filter_map(|entry| {
                        let path = entry.path();
                        if !path.is_dir() {
                            return None;
                        }
                        path.file_name()
                            .and_then(|n| n.to_str())
                            .map(str::to_string)
                    })
                    .collect(),
                Err(_) => return HashMap::new(),
            };

            encoded_names
                .into_par_iter()
                .filter_map(|encoded_name| {
                    let tags = get_all_tags("claude", &encoded_name);
                    if tags.is_empty() {
                        None
                    } else {
                        Some((encoded_name, tags))
                    }
                })
                .collect()
        }
        "codex" => {
            // Codex stores all session metadata in a single file, so there's
            // nothing to parallelize on this branch — one read either way.
            let tags = get_all_tags("codex", "");
            let mut result = HashMap::new();
            if !tags.is_empty() {
                result.insert(String::new(), tags);
            }
            result
        }
        _ => HashMap::new(),
    }
}
