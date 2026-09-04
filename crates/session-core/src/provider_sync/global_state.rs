use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

pub fn global_state_path(codex_home: &Path) -> PathBuf {
    codex_home.join(".codex-global-state.json")
}

pub fn normalize_paths(path: &Path) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut v: Value =
        serde_json::from_str(&text).map_err(|e| format!("parse global state: {}", e))?;
    let mut changed = false;

    for key in [
        "electron-saved-workspace-roots",
        "project-order",
        "active-workspace-roots",
    ] {
        if let Some(arr) = v.get_mut(key).and_then(|x| x.as_array_mut()) {
            for item in arr.iter_mut() {
                if let Some(s) = item.as_str() {
                    let normalized = normalize_path_string(s);
                    if normalized != s {
                        *item = Value::String(normalized);
                        changed = true;
                    }
                }
            }
        }
    }

    if let Some(obj) = v
        .get_mut("electron-workspace-root-labels")
        .and_then(|x| x.as_object_mut())
    {
        rekey_object(obj, &mut changed);
    }

    if let Some(per_path) = v
        .get_mut("open-in-target-preferences")
        .and_then(|x| x.get_mut("perPath"))
        .and_then(|x| x.as_object_mut())
    {
        rekey_object(per_path, &mut changed);
    }

    if changed {
        let serialized = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
        fs::write(path, serialized).map_err(|e| e.to_string())?;
    }
    Ok(changed)
}

fn rekey_object(obj: &mut serde_json::Map<String, Value>, changed: &mut bool) {
    let keys: Vec<String> = obj.keys().cloned().collect();
    for k in keys {
        let normalized = normalize_path_string(&k);
        if normalized != k {
            if let Some(val) = obj.remove(&k) {
                obj.insert(normalized, val);
                *changed = true;
            }
        }
    }
}

pub fn normalize_path_string(s: &str) -> String {
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{}", rest)
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        s.to_string()
    }
}
