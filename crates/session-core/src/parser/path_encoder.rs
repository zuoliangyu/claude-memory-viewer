use std::path::PathBuf;

/// Get the Claude home directory (~/.claude)
pub fn get_claude_home() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude"))
}

/// Get the Claude projects directory (~/.claude/projects)
pub fn get_projects_dir() -> Option<PathBuf> {
    get_claude_home().map(|h| h.join("projects"))
}

/// Get the stats cache file path (~/.claude/stats-cache.json)
pub fn get_stats_cache_path() -> Option<PathBuf> {
    get_claude_home().map(|h| h.join("stats-cache.json"))
}

/// Decode an encoded project directory name back to a path (best-effort fallback)
/// Prefer using originalPath from sessions-index.json when available
pub fn decode_project_path(encoded: &str) -> String {
    if cfg!(windows) {
        if encoded.len() >= 2 && encoded.chars().nth(1) == Some('-') {
            let drive = &encoded[0..1];
            let rest = &encoded[2..];
            let path_part = rest.replace('-', "\\");
            format!("{}:{}", drive, path_part)
        } else {
            encoded.replace('-', "\\")
        }
    } else {
        encoded.replace('-', "/")
    }
}

/// Extract the last path segment as a short name
pub fn short_name_from_path(path: &str) -> String {
    let path = path.trim_end_matches(['/', '\\']);
    if let Some(pos) = path.rfind(['/', '\\']) {
        path[pos + 1..].to_string()
    } else {
        path.to_string()
    }
}
