use std::path::{Path, PathBuf};

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

/// Simulate Claude Code's path encoding: non-ASCII-alphanumeric characters become `-`
fn encode_path_segment(segment: &str) -> String {
    segment
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Encode a full filesystem path to Claude's project directory name
/// (`~/.claude/projects/<encoded>`). Same rule as `encode_path_segment` —
/// every non-ASCII-alphanumeric char becomes `-`.
pub fn encode_project_path(path: &str) -> String {
    encode_path_segment(path)
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

/// Result of validated path decoding
pub struct DecodedPath {
    /// The decoded display path (real filesystem path if matched, basic decode otherwise)
    pub display_path: String,
    /// Whether the decoded path actually exists on the filesystem
    pub path_exists: bool,
}

/// Decode an encoded project path with filesystem validation.
///
/// Walks the encoded segments, matching each against actual directory entries
/// by re-encoding them and comparing. This recovers the original path including
/// non-ASCII characters (Chinese, etc.), dots, and other characters that are
/// all encoded as `-` by Claude Code.
///
/// Falls back to basic `decode_project_path` if filesystem matching fails.
pub fn decode_project_path_validated(encoded: &str) -> DecodedPath {
    // Split the encoded name by `-` (the universal separator)
    // But first, handle drive prefix on Windows (e.g., "C--Users-foo")
    // and Unix root (leading `-` means `/`)

    if cfg!(windows) {
        decode_validated_windows(encoded)
    } else {
        decode_validated_unix(encoded)
    }
}

fn decode_validated_windows(encoded: &str) -> DecodedPath {
    // Windows: "C--Users-project" → "C:\Users\project"
    // The `--` after drive letter encodes `:\`
    // But within path segments, `-` could be a real `-` or a separator

    // Extract drive prefix: first char + "--"
    if encoded.len() < 3 || !encoded.as_bytes()[0].is_ascii_alphabetic() {
        let basic = decode_project_path(encoded);
        let exists = Path::new(&basic).exists();
        return DecodedPath {
            display_path: basic,
            path_exists: exists,
        };
    }

    // Check for drive pattern: "X-..." where X is a letter
    let drive_letter = encoded.chars().next().unwrap();
    let after_drive = &encoded[1..];

    // Windows paths encode "X:\" as "X--" (`:` → `-`, `\` → `-`).
    // We must skip BOTH dashes to arrive at the first real path segment.
    // A bare "X-" (colon only, no backslash) is technically valid but rare.
    let (root, remaining) = if let Some(stripped) = after_drive.strip_prefix("--") {
        // Normal case: "C--Users-..." → root "C:\", remaining "Users-..."
        (format!("{}:\\", drive_letter), stripped)
    } else if let Some(stripped) = after_drive.strip_prefix('-') {
        // Edge case: "C-something" → treat as "C:\" still
        (format!("{}:\\", drive_letter), stripped)
    } else {
        let basic = decode_project_path(encoded);
        let exists = Path::new(&basic).exists();
        return DecodedPath {
            display_path: basic,
            path_exists: exists,
        };
    };

    let root_path = PathBuf::from(&root);

    if !root_path.exists() {
        // Drive not present (e.g. path from another machine).
        // Use the encoded name to extract a meaningful short name for display.
        let meaningful = short_name_from_encoded(encoded);
        let display = format!("{}\\...\\{}", root.trim_end_matches('\\'), meaningful);
        return DecodedPath {
            display_path: display,
            path_exists: false,
        };
    }

    // `remaining` now starts cleanly at the first path segment (e.g. "Users-...")
    match resolve_segments(&root_path, remaining) {
        Some(resolved) => {
            let display = resolved.to_string_lossy().to_string();
            DecodedPath {
                path_exists: true,
                display_path: display,
            }
        }
        None => {
            // Partial match: walk as far as possible on disk, append remaining
            // encoded tokens as a single hyphenated component for a better short name.
            let partial = resolve_segments_partial(&root_path, remaining);
            DecodedPath {
                path_exists: false,
                display_path: partial.to_string_lossy().to_string(),
            }
        }
    }
}

fn decode_validated_unix(encoded: &str) -> DecodedPath {
    // Unix: "-home-user-project" → "/home/user/project"
    // Leading `-` encodes root `/`

    let root_path = PathBuf::from("/");
    let remaining = encoded.trim_start_matches('-');

    if remaining.is_empty() {
        return DecodedPath {
            display_path: "/".to_string(),
            path_exists: true,
        };
    }

    match resolve_segments(&root_path, remaining) {
        Some(resolved) => {
            let display = resolved.to_string_lossy().to_string();
            DecodedPath {
                path_exists: true,
                display_path: display,
            }
        }
        None => {
            // Partial match: walk as far as possible on disk, append remaining
            // encoded tokens as a single hyphenated component for a better short name.
            let partial = resolve_segments_partial(&root_path, remaining);
            DecodedPath {
                path_exists: false,
                display_path: partial.to_string_lossy().to_string(),
            }
        }
    }
}

/// Core segment resolver: given a base directory and the remaining encoded string,
/// try to match real filesystem entries by greedily consuming encoded segments.
///
/// Strategy: try progressively longer segments (joining with `-`) and check if
/// any child of the current directory encodes to that segment.
fn resolve_segments(base: &Path, remaining: &str) -> Option<PathBuf> {
    if remaining.is_empty() {
        return Some(base.to_path_buf());
    }

    let parts: Vec<&str> = remaining.split('-').collect();
    resolve_segments_recursive(base, &parts, 0)
}

/// Partial variant: matches as many segments as possible from the filesystem.
/// When a level cannot be matched (directory missing or moved), the remaining
/// encoded tokens are joined with `-` and appended as a single path component.
///
/// This gives a much better short name than the naive `-` → separator fallback.
/// Example: if `liaoyuan_web_wrokspace` exists but `liaoyuan_materials` does not,
/// returns `.../liaoyuan_web_wrokspace/liaoyuan-materials` instead of `.../materials`.
fn resolve_segments_partial(base: &Path, remaining: &str) -> PathBuf {
    if remaining.is_empty() {
        return base.to_path_buf();
    }

    let parts: Vec<&str> = remaining.split('-').collect();
    resolve_segments_partial_recursive(base, &parts, 0)
}

fn resolve_segments_partial_recursive(current: &Path, parts: &[&str], start: usize) -> PathBuf {
    if start >= parts.len() {
        return current.to_path_buf();
    }

    let dir_entries: Vec<(String, String)> = match std::fs::read_dir(current) {
        Ok(rd) => rd
            .flatten()
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let encoded = encode_path_segment(&name);
                (name, encoded)
            })
            .collect(),
        Err(_) => {
            // Can't read directory; append all remaining tokens as one component.
            return current.join(parts[start..].join("-"));
        }
    };

    let max_consume = parts.len() - start;

    for count in 1..=max_consume {
        let candidate_encoded: String = parts[start..start + count].join("-");
        if candidate_encoded.is_empty() {
            continue;
        }

        for (real_name, entry_encoded) in &dir_entries {
            if *entry_encoded == candidate_encoded {
                let child = current.join(real_name);
                if start + count >= parts.len() {
                    return child;
                }
                if child.is_dir() {
                    return resolve_segments_partial_recursive(&child, parts, start + count);
                }
            }
        }

        // Unix `--` → `.` heuristic
        if !cfg!(windows) && count >= 2 && parts[start].is_empty() {
            let dot_candidate = format!(".{}", parts[start + 1..start + count].join("-"));
            let dot_encoded = encode_path_segment(&dot_candidate);
            for (real_name, entry_encoded) in &dir_entries {
                if *entry_encoded == dot_encoded {
                    let child = current.join(real_name);
                    if start + count >= parts.len() {
                        return child;
                    }
                    if child.is_dir() {
                        return resolve_segments_partial_recursive(&child, parts, start + count);
                    }
                }
            }
        }
    }

    // No match at this level: append remaining tokens as a single hyphenated component.
    current.join(parts[start..].join("-"))
}

fn resolve_segments_recursive(current: &Path, parts: &[&str], start: usize) -> Option<PathBuf> {
    if start >= parts.len() {
        return Some(current.to_path_buf());
    }

    // Read directory entries (cached per call — we iterate children once)
    let dir_entries: Vec<(String, String)> = match std::fs::read_dir(current) {
        Ok(rd) => rd
            .flatten()
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let encoded = encode_path_segment(&name);
                (name, encoded)
            })
            .collect(),
        Err(_) => return None,
    };

    // Try consuming 1, 2, 3, ... parts as a single segment name
    // (because a real directory name might contain `-` which also encodes to `-`)
    let max_consume = parts.len() - start;

    // Also try `--` → `.` heuristic for hidden directories (Unix)
    // When we see an empty part (from splitting `--`), it might be a `.` prefix

    for count in 1..=max_consume {
        let candidate_encoded: String = parts[start..start + count].join("-");

        // Skip empty candidates
        if candidate_encoded.is_empty() {
            continue;
        }

        // Check: does any directory entry encode to this candidate?
        for (real_name, entry_encoded) in &dir_entries {
            if *entry_encoded == candidate_encoded {
                let child = current.join(real_name);
                if start + count >= parts.len() {
                    // We consumed all parts
                    return Some(child);
                }
                // Only recurse into directories
                if child.is_dir() {
                    if let Some(result) = resolve_segments_recursive(&child, parts, start + count) {
                        return Some(result);
                    }
                }
                // If not a directory or recursion failed, try consuming more parts
            }
        }

        // Unix `--` → `.` heuristic: if candidate starts with empty segment,
        // try matching with a `.` prefix (hidden directories like .claude, .config)
        if !cfg!(windows) && count >= 2 && parts[start].is_empty() {
            // The `--` split as ["", "rest"] → try ".rest"
            let dot_candidate = format!(".{}", parts[start + 1..start + count].join("-"));
            let dot_encoded = encode_path_segment(&dot_candidate);
            for (real_name, entry_encoded) in &dir_entries {
                if *entry_encoded == dot_encoded {
                    let child = current.join(real_name);
                    if start + count >= parts.len() {
                        return Some(child);
                    }
                    if child.is_dir() {
                        if let Some(result) =
                            resolve_segments_recursive(&child, parts, start + count)
                        {
                            return Some(result);
                        }
                    }
                }
            }
        }
    }

    None
}

/// Extract the last path segment as a short name.
///
/// For fully-resolved real paths this is just the last component.
/// For partially-resolved or naive-decoded paths the last component may be
/// a hyphenated encoded segment like "liaoyuan-materials" — that is already
/// better than splitting on `-` further, so we keep it as-is.
pub fn short_name_from_path(path: &str) -> String {
    let path = path.trim_end_matches(['/', '\\']);
    if let Some(pos) = path.rfind(['/', '\\']) {
        path[pos + 1..].to_string()
    } else {
        path.to_string()
    }
}

/// Derive a best-effort short name directly from an encoded project directory name,
/// used when no `originalPath` is available AND filesystem matching completely fails
/// (e.g. the path lives on another machine with no accessible root drive).
///
/// Strips well-known leading components (drive letter, Users/{name}, Desktop/Documents…)
/// and returns the last 1–2 meaningful hyphenated segments.
pub fn short_name_from_encoded(encoded: &str) -> String {
    // Split on `-` to get raw tokens (empty tokens from leading/doubled dashes are removed)
    let tokens: Vec<&str> = encoded.split('-').filter(|s| !s.is_empty()).collect();
    if tokens.is_empty() {
        return encoded.to_string();
    }

    // Known "boring" path component patterns to skip from the left:
    // - single-letter drive (Windows: C, D, …)
    // - "Users" / "home"
    // - common system dirs
    const SKIP: &[&str] = &[
        "Users",
        "home",
        "Desktop",
        "Documents",
        "Downloads",
        "OneDrive",
        "Dropbox",
        "iCloud",
        "projects",
        "workspace",
        "workspaces",
        "dev",
        "code",
        "src",
    ];

    let mut start = 0;

    // Skip drive letter (single ASCII alpha token)
    if tokens[start].len() == 1 && tokens[start].chars().all(|c| c.is_ascii_alphabetic()) {
        start += 1;
    }

    // Skip "Users" or "home" and the following username token
    if start < tokens.len()
        && (tokens[start].eq_ignore_ascii_case("Users")
            || tokens[start].eq_ignore_ascii_case("home"))
    {
        start += 1; // skip "Users"/"home"
        if start < tokens.len() {
            start += 1; // skip username
        }
    }

    // Skip other known boring leading tokens
    while start < tokens.len() && SKIP.iter().any(|s| tokens[start].eq_ignore_ascii_case(s)) {
        start += 1;
    }

    let meaningful: &[&str] = if start < tokens.len() {
        &tokens[start..]
    } else {
        // Fell through everything; use last 2 tokens
        let tail = tokens.len().saturating_sub(2);
        &tokens[tail..]
    };

    if meaningful.is_empty() {
        return tokens.last().copied().unwrap_or(encoded).to_string();
    }

    // Return only the last "segment group" — look for the last boundary that
    // looks like two consecutive short common tokens (heuristic: len <= 3 suggests
    // a separator-like word such as a very short abbreviation). Otherwise just
    // take all meaningful tokens joined.
    meaningful.join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_path_segment() {
        assert_eq!(encode_path_segment("hello"), "hello");
        assert_eq!(encode_path_segment("hello world"), "hello-world");
        assert_eq!(encode_path_segment("my.project"), "my-project");
        assert_eq!(encode_path_segment(".claude"), "-claude");
        assert_eq!(encode_path_segment("中文目录"), "----");
        assert_eq!(encode_path_segment("test-name"), "test-name");
    }

    #[test]
    fn test_basic_decode_unchanged() {
        // Basic decode should still work the same
        if cfg!(windows) {
            assert_eq!(decode_project_path("C-Users-test"), "C:\\Users\\test");
        } else {
            assert_eq!(decode_project_path("-home-user-test"), "/home/user/test");
        }
    }

    #[test]
    fn test_windows_double_dash_prefix() {
        // "C--Users-..." encodes "C:\Users\..."
        // The resolved remaining after stripping "C--" must be "Users-..."
        // (two dashes for `:` and `\`, not one).
        // We verify via decode_project_path since filesystem may not match in tests.
        let encoded = "C--Users-zuolan-Desktop-liaoyuan-web-wrokspace-liaoyuan-materials";
        // Basic decode: all `-` → `\`
        let basic = decode_project_path(encoded);
        assert_eq!(
            basic,
            "C:\\\\Users\\zuolan\\Desktop\\liaoyuan\\web\\wrokspace\\liaoyuan\\materials"
        );
        // The validated decoder should NOT show just "materials" as the short name.
        // (Full filesystem match or partial — either way last component ≠ "materials").
        let decoded = decode_project_path_validated(encoded);
        let short = short_name_from_path(&decoded.display_path);
        assert_ne!(
            short, "materials",
            "short name should not be the naive last token"
        );
    }
}
