use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use filetime::{set_file_mtime, FileTime};
use serde_json::Value;

const META_SCAN_LINES: usize = 50;

pub struct RolloutMetaInfo {
    pub path: PathBuf,
    pub provider: String,
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub session_meta_line_idx: usize,
    pub original_line: String,
    pub mtime: SystemTime,
    pub has_encrypted_content: bool,
}

pub fn scan_session_dir(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !dir.exists() {
        return out;
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(read) = fs::read_dir(&current) else {
            continue;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name.starts_with("rollout-") {
                out.push(path);
            }
        }
    }
    out
}

pub fn read_meta(path: &Path, scan_encrypted: bool) -> Option<RolloutMetaInfo> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut idx = 0usize;
    let mut found: Option<(usize, String, Value)> = None;
    for line in reader.lines().take(META_SCAN_LINES) {
        let raw = match line {
            Ok(l) => l,
            Err(_) => {
                idx += 1;
                continue;
            }
        };
        let trimmed = raw.trim_start_matches('\u{feff}').trim();
        if trimmed.is_empty() {
            idx += 1;
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
            if v.get("type").and_then(|x| x.as_str()) == Some("session_meta") {
                found = Some((idx, raw, v));
                break;
            }
        }
        idx += 1;
    }
    let (line_idx, original_line, v) = found?;
    let payload = v.get("payload")?;
    let provider = payload
        .get("model_provider")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let session_id = payload.get("id").and_then(|x| x.as_str()).map(String::from);
    let cwd = payload
        .get("cwd")
        .and_then(|x| x.as_str())
        .map(String::from);
    let metadata = fs::metadata(path).ok()?;
    let mtime = metadata.modified().ok()?;
    let has_encrypted = if scan_encrypted {
        scan_for_encrypted_content(path)
    } else {
        false
    };

    Some(RolloutMetaInfo {
        path: path.to_path_buf(),
        provider,
        session_id,
        cwd,
        session_meta_line_idx: line_idx,
        original_line,
        mtime,
        has_encrypted_content: has_encrypted,
    })
}

fn scan_for_encrypted_content(path: &Path) -> bool {
    let needle = b"encrypted_content";
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut buf = vec![0u8; 64 * 1024];
    let keep = needle.len() - 1;
    let mut tail: Vec<u8> = Vec::with_capacity(keep);
    loop {
        let n = match file.read(&mut buf) {
            Ok(0) => return false,
            Ok(n) => n,
            Err(_) => return false,
        };
        let mut chunk = Vec::with_capacity(tail.len() + n);
        chunk.extend_from_slice(&tail);
        chunk.extend_from_slice(&buf[..n]);
        if chunk.windows(needle.len()).any(|w| w == needle) {
            return true;
        }
        if chunk.len() > keep {
            tail = chunk[chunk.len() - keep..].to_vec();
        } else {
            tail = chunk;
        }
    }
}

pub fn rewrite_session_meta_provider(
    path: &Path,
    line_idx: usize,
    original_line: &str,
    new_provider: &str,
    mtime: SystemTime,
) -> Result<String, String> {
    let raw_text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let original_text = raw_text.replace("\r\n", "\n");
    let trailing_newline = original_text.ends_with('\n');
    let mut lines: Vec<String> = original_text.split('\n').map(|s| s.to_string()).collect();
    if trailing_newline && lines.last().map(|s| s.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    if line_idx >= lines.len() {
        return Err(format!(
            "session_meta line index {} out of range (file has {} lines)",
            line_idx,
            lines.len()
        ));
    }
    if lines[line_idx] != original_line {
        return Err(format!(
            "rollout {} changed during sync; aborting rewrite",
            path.display()
        ));
    }
    let mut v: Value = serde_json::from_str(original_line.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("parse session_meta: {}", e))?;
    if let Some(payload) = v.get_mut("payload") {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert(
                "model_provider".into(),
                Value::String(new_provider.to_string()),
            );
        }
    }
    let new_line = serde_json::to_string(&v).map_err(|e| e.to_string())?;
    lines[line_idx] = new_line.clone();
    let mut content = lines.join("\n");
    if trailing_newline {
        content.push('\n');
    }

    let tmp = path.with_extension("jsonl.tmp");
    {
        let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        f.write_all(content.as_bytes()).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
    }
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("replace rollout file: {}", e));
    }
    let _ = set_file_mtime(path, FileTime::from_system_time(mtime));
    Ok(new_line)
}

/// Write a non-destructive clone of `src` to `dst`, rewriting only the
/// `session_meta` line so the copy carries a new `model_provider`, a new `id`
/// (and `session_id` when present). The conversation body is copied verbatim —
/// like the reference tool, this is a *shallow* clone: event rows deeper in the
/// file keep references to the original id, which is fine for viewing/resume.
/// `dst` is a brand-new path, so a plain write is enough (no atomic rename).
pub fn write_cloned_rollout(
    src: &Path,
    dst: &Path,
    line_idx: usize,
    original_line: &str,
    new_provider: &str,
    new_id: &str,
    mtime: SystemTime,
) -> Result<(), String> {
    let raw_text = fs::read_to_string(src).map_err(|e| e.to_string())?;
    let original_text = raw_text.replace("\r\n", "\n");
    let trailing_newline = original_text.ends_with('\n');
    let mut lines: Vec<String> = original_text.split('\n').map(|s| s.to_string()).collect();
    if trailing_newline && lines.last().map(|s| s.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    if line_idx >= lines.len() {
        return Err(format!(
            "session_meta line index {} out of range (file has {} lines)",
            line_idx,
            lines.len()
        ));
    }
    if lines[line_idx] != original_line {
        return Err(format!(
            "rollout {} changed during clone; aborting",
            src.display()
        ));
    }
    let mut v: Value = serde_json::from_str(original_line.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("parse session_meta: {}", e))?;
    if let Some(payload) = v.get_mut("payload").and_then(|p| p.as_object_mut()) {
        payload.insert(
            "model_provider".into(),
            Value::String(new_provider.to_string()),
        );
        payload.insert("id".into(), Value::String(new_id.to_string()));
        if payload.contains_key("session_id") {
            payload.insert("session_id".into(), Value::String(new_id.to_string()));
        }
    }
    let new_line = serde_json::to_string(&v).map_err(|e| e.to_string())?;
    lines[line_idx] = new_line;
    let mut content = lines.join("\n");
    if trailing_newline {
        content.push('\n');
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(dst, content).map_err(|e| format!("write clone {}: {}", dst.display(), e))?;
    let _ = set_file_mtime(dst, FileTime::from_system_time(mtime));
    Ok(())
}

pub fn restore_session_meta(
    path: &Path,
    line_idx: usize,
    original_line: &str,
    mtime_secs: i64,
) -> Result<(), String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let text = raw.replace("\r\n", "\n");
    let trailing = text.ends_with('\n');
    let mut lines: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();
    if trailing && lines.last().map(|s| s.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    if line_idx >= lines.len() {
        return Err("line index out of range".into());
    }
    lines[line_idx] = original_line.to_string();
    let mut content = lines.join("\n");
    if trailing {
        content.push('\n');
    }
    fs::write(path, content).map_err(|e| e.to_string())?;
    let _ = set_file_mtime(path, FileTime::from_unix_time(mtime_secs, 0));
    Ok(())
}
