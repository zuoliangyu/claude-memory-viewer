use std::fs;
use std::path::Path;
use std::process::Command;

use session_core::models::session::{SessionsIndex, SessionsIndexFileEntry};
use session_core::parser::jsonl as claude_parser;

#[tauri::command]
pub fn resume_session(
    source: String,
    session_id: String,
    project_path: String,
    file_path: Option<String>,
) -> Result<(), String> {
    // Try to derive the correct project path from the session file location
    let project_path = resolve_project_path(&source, &project_path, file_path.as_deref());

    if !Path::new(&project_path).exists() {
        return Err(format!("项目路径不存在: {}", project_path));
    }

    // For Claude sessions, ensure the session is in sessions-index.json
    // so that `claude --resume` can find it
    if source == "claude" {
        if let Some(fp) = &file_path {
            let fp = normalize_path(fp);
            ensure_session_in_index(&session_id, &fp, &project_path);
        }
    }

    let cli_cmd = match source.as_str() {
        "claude" => format!("claude --resume {}", session_id),
        "codex" => format!("codex resume {}", session_id),
        _ => return Err(format!("Unknown source: {}", source)),
    };

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        Command::new("cmd")
            .args(["/c", "start", "", "/d", &project_path, "cmd", "/k", &cli_cmd])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "tell application \"Terminal\" to do script \"cd '{}' && {}\"",
            project_path, cli_cmd
        );
        Command::new("osascript")
            .args(["-e", &script])
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;

        let cmd_str = format!("cd '{}' && {}", project_path, cli_cmd);

        let xfce_arg = format!("bash -c '{}'", cmd_str);
        let xterm_arg = format!("bash -c '{}'", cmd_str);
        let terminals: [(&str, &[&str]); 4] = [
            ("gnome-terminal", &["--", "bash", "-c", &cmd_str]),
            ("konsole", &["-e", "bash", "-c", &cmd_str]),
            ("xfce4-terminal", &["-e", &xfce_arg]),
            ("xterm", &["-e", &xterm_arg]),
        ];

        let mut launched = false;
        for (terminal, args) in &terminals {
            if Command::new(terminal)
                .args(*args)
                .process_group(0)
                .spawn()
                .is_ok()
            {
                launched = true;
                break;
            }
        }

        if !launched {
            return Err("No supported terminal emulator found".to_string());
        }
    }

    Ok(())
}

/// Resolve the correct project path for resuming a session.
/// Priority: sessions-index.json original_path > provided project_path
fn resolve_project_path(source: &str, project_path: &str, file_path: Option<&str>) -> String {
    if source == "claude" {
        if let Some(fp) = file_path {
            let fp = normalize_path(fp);
            if let Some(parent) = Path::new(&fp).parent() {
                let index_path = parent.join("sessions-index.json");
                if index_path.exists() {
                    if let Ok(content) = fs::read_to_string(&index_path) {
                        if let Ok(index) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(original) =
                                index.get("originalPath").and_then(|v| v.as_str())
                            {
                                let resolved = normalize_path(original);
                                if Path::new(&resolved).exists() {
                                    return resolved;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    normalize_path(project_path)
}

/// Ensure a session entry exists in sessions-index.json so that
/// `claude --resume {id}` can discover it. Orphan sessions (e.g. from
/// Ctrl+C exits) exist on disk but are missing from the index.
fn ensure_session_in_index(session_id: &str, file_path: &str, project_path: &str) {
    let session_file = Path::new(file_path);
    let parent = match session_file.parent() {
        Some(p) => p,
        None => return,
    };

    let index_path = parent.join("sessions-index.json");

    // Read existing index or create a new one
    let mut index: SessionsIndex = if index_path.exists() {
        match fs::read_to_string(&index_path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
        {
            Some(idx) => idx,
            None => return, // Can't parse existing index, don't risk corrupting it
        }
    } else {
        SessionsIndex {
            version: Some(1),
            entries: Vec::new(),
            original_path: Some(project_path.to_string()),
        }
    };

    // Already in index — nothing to do
    if index.entries.iter().any(|e| e.session_id == session_id) {
        return;
    }

    // Build an entry from the JSONL file metadata
    let first_prompt = claude_parser::extract_first_prompt(session_file);
    let metadata = claude_parser::extract_session_metadata(session_file);
    let (_, git_branch, cwd) = metadata.unwrap_or((String::new(), None, None));

    let file_meta = fs::metadata(session_file).ok();
    let mtime = file_meta.as_ref().and_then(|m| {
        m.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
    });
    let modified = file_meta.as_ref().and_then(|m| {
        m.modified().ok().map(|t| {
            let d = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
            chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default()
        })
    });
    let created = file_meta.as_ref().and_then(|m| {
        m.created().ok().map(|t| {
            let d = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
            chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default()
        })
    });

    let message_count = count_user_assistant(session_file);

    index.entries.push(SessionsIndexFileEntry {
        session_id: session_id.to_string(),
        full_path: Some(file_path.to_string()),
        file_mtime: mtime,
        first_prompt,
        message_count: Some(message_count),
        created,
        modified,
        git_branch,
        project_path: cwd.or_else(|| Some(project_path.to_string())),
        is_sidechain: Some(false),
    });

    // Write back
    if let Ok(json) = serde_json::to_string_pretty(&index) {
        let _ = fs::write(&index_path, json);
    }
}

fn count_user_assistant(path: &Path) -> u32 {
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

fn normalize_path(path: &str) -> String {
    if cfg!(windows) {
        path.replace('/', "\\")
    } else {
        path.replace('\\', "/")
    }
}
