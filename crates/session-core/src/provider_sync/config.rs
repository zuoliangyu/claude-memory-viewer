use std::fs;
use std::path::Path;

const DEFAULT_PROVIDER: &str = "openai";

pub fn read_current_provider(config_path: &Path) -> (String, bool) {
    let text = match fs::read_to_string(config_path) {
        Ok(s) => s,
        Err(_) => return (DEFAULT_PROVIDER.to_string(), true),
    };
    match parse_root_provider(&text) {
        Some(p) => (p, false),
        None => (DEFAULT_PROVIDER.to_string(), true),
    }
}

fn parse_root_provider(text: &str) -> Option<String> {
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            return None;
        }
        if let Some(rest) = line.strip_prefix("model_provider") {
            let rest = rest.trim_start();
            let Some(rest) = rest.strip_prefix('=') else {
                continue;
            };
            let value = strip_comment(rest).trim();
            let unquoted = value.trim_matches('"').trim_matches('\'').to_string();
            if !unquoted.is_empty() {
                return Some(unquoted);
            }
        }
    }
    None
}

fn strip_comment(s: &str) -> &str {
    let bytes = s.as_bytes();
    let mut in_double = false;
    let mut in_single = false;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'"' if !in_single => in_double = !in_double,
            b'\'' if !in_double => in_single = !in_single,
            b'#' if !in_double && !in_single => return &s[..i],
            _ => {}
        }
    }
    s
}

pub fn list_configured_providers(config_path: &Path) -> Vec<String> {
    let text = match fs::read_to_string(config_path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("[model_providers.") else {
            continue;
        };
        let Some(end) = rest.find(']') else { continue };
        let id = rest[..end].trim().trim_matches('"').to_string();
        if !id.is_empty() && !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

pub fn set_root_provider(config_path: &Path, new_provider: &str) -> Result<String, String> {
    let original = fs::read_to_string(config_path).unwrap_or_default();

    let mut lines: Vec<String> = if original.is_empty() {
        Vec::new()
    } else {
        original.split('\n').map(|s| s.to_string()).collect()
    };

    let mut replaced = false;
    let mut first_section_idx: Option<usize> = None;
    for (i, raw) in lines.iter().enumerate() {
        let line = strip_comment(raw).trim();
        if line.starts_with('[') {
            first_section_idx = Some(i);
            break;
        }
        if line.starts_with("model_provider") && line.contains('=') {
            lines[i] = format!("model_provider = \"{}\"", new_provider);
            replaced = true;
            break;
        }
    }

    if !replaced {
        let insert_at = first_section_idx.unwrap_or(lines.len());
        lines.insert(insert_at, format!("model_provider = \"{}\"", new_provider));
        if first_section_idx.is_some() {
            lines.insert(insert_at + 1, String::new());
        }
    }

    let new_text = lines.join("\n");
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(config_path, &new_text).map_err(|e| e.to_string())?;
    Ok(original)
}
