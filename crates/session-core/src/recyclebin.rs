use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::app_dir::{get_recyclebin_items_dir, get_recyclebin_manifest_path};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecycledItem {
    pub id: String,
    pub item_type: String,
    pub reason: String,
    pub source: String,
    pub project_id: String,
    pub session_title: Option<String>,
    pub project_name: Option<String>,
    pub original_path: String,
    pub stored_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub companion_original_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub companion_stored_name: Option<String>,
    pub moved_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecyclebinManifest {
    pub version: u32,
    pub items: Vec<RecycledItem>,
}

impl Default for RecyclebinManifest {
    fn default() -> Self {
        RecyclebinManifest {
            version: 1,
            items: vec![],
        }
    }
}

fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}", ts)
}

pub fn load_manifest() -> RecyclebinManifest {
    let path = match get_recyclebin_manifest_path() {
        Some(p) => p,
        None => return RecyclebinManifest::default(),
    };
    if !path.exists() {
        return RecyclebinManifest::default();
    }
    let data = match fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return RecyclebinManifest::default(),
    };
    serde_json::from_str(&data).unwrap_or_default()
}

pub fn save_manifest(manifest: &RecyclebinManifest) -> Result<(), String> {
    let path = get_recyclebin_manifest_path()
        .ok_or_else(|| "Cannot determine recyclebin path".to_string())?;

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create recyclebin dir: {}", e))?;
    }

    let json = serde_json::to_string_pretty(manifest)
        .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, &json).map_err(|e| format!("Failed to write manifest tmp: {}", e))?;
    fs::rename(&tmp_path, &path).map_err(|e| format!("Failed to rename manifest: {}", e))?;
    Ok(())
}

/// 移动文件或目录到回收站 items/ 目录，追加 manifest，返回生成的 id。
pub fn move_to_recyclebin(
    original_path: &std::path::Path,
    item_type: &str,
    reason: &str,
    source: &str,
    project_id: &str,
    session_title: Option<String>,
    project_name: Option<String>,
) -> Result<String, String> {
    let items_dir = get_recyclebin_items_dir()
        .ok_or_else(|| "Cannot determine recyclebin items path".to_string())?;
    fs::create_dir_all(&items_dir)
        .map_err(|e| format!("Failed to create recyclebin items dir: {}", e))?;

    let id = generate_id();

    // 计算 stored_name：目录用 id/，文件用 id.ext
    let stored_name = if original_path.is_dir() {
        id.clone()
    } else {
        match original_path.extension().and_then(|e| e.to_str()) {
            Some(ext) => format!("{}.{}", id, ext),
            None => id.clone(),
        }
    };

    let target: PathBuf = items_dir.join(&stored_name);

    // 目标已存在则报错（理论上 id 纳秒级不会重复）
    if target.exists() {
        return Err(format!("Target already exists: {:?}", target));
    }

    fs::rename(original_path, &target)
        .map_err(|e| format!("Failed to move to recyclebin: {}", e))?;

    let item = RecycledItem {
        id: id.clone(),
        item_type: item_type.to_string(),
        reason: reason.to_string(),
        source: source.to_string(),
        project_id: project_id.to_string(),
        session_title,
        project_name,
        original_path: original_path.to_string_lossy().to_string(),
        stored_name,
        companion_original_path: None,
        companion_stored_name: None,
        moved_at: chrono::Utc::now().to_rfc3339(),
    };

    let mut manifest = load_manifest();
    manifest.items.push(item);
    save_manifest(&manifest)?;

    Ok(id)
}

fn session_artifact_dir(session_path: &std::path::Path) -> Result<Option<PathBuf>, String> {
    let artifact_path = session_path.with_extension("");
    match fs::symlink_metadata(&artifact_path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "Failed to inspect session artifact directory: {error}"
        )),
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err("OMP session artifact directory must not be a symbolic link".to_string())
        }
        Ok(metadata) if metadata.is_dir() => Ok(Some(artifact_path)),
        Ok(_) => Err("OMP session artifact path is not a directory".to_string()),
    }
}

/// Move an OMP session JSONL and its sibling artifact directory as one
/// recoverable recycle-bin item. The artifact directory is optional because
/// plain text-only sessions do not create one.
pub fn move_omp_session_to_recyclebin(
    session_path: &std::path::Path,
    project_id: &str,
    session_title: Option<String>,
    project_name: Option<String>,
) -> Result<String, String> {
    if !session_path.is_file() {
        return Err("OMP session file not found".to_string());
    }
    let artifact_path = session_artifact_dir(session_path)?;
    let items_dir = get_recyclebin_items_dir()
        .ok_or_else(|| "Cannot determine recyclebin items path".to_string())?;
    fs::create_dir_all(&items_dir)
        .map_err(|error| format!("Failed to create recyclebin items dir: {error}"))?;

    let id = generate_id();
    let stored_name = format!("{id}.jsonl");
    let stored_path = items_dir.join(&stored_name);
    let companion_stored_name = artifact_path.as_ref().map(|_| format!("{id}.artifacts"));
    let companion_stored_path = companion_stored_name
        .as_ref()
        .map(|name| items_dir.join(name));
    if stored_path.exists()
        || companion_stored_path
            .as_ref()
            .is_some_and(|path| path.exists())
    {
        return Err("Recyclebin target already exists".to_string());
    }

    fs::rename(session_path, &stored_path)
        .map_err(|error| format!("Failed to move OMP session to recyclebin: {error}"))?;
    if let (Some(artifact_path), Some(companion_stored_path)) =
        (artifact_path.as_ref(), companion_stored_path.as_ref())
    {
        if let Err(error) = fs::rename(artifact_path, companion_stored_path) {
            let rollback = fs::rename(&stored_path, session_path);
            return match rollback {
                Ok(()) => Err(format!("Failed to move OMP session artifacts to recyclebin: {error}")),
                Err(rollback_error) => Err(format!(
                    "Failed to move OMP session artifacts to recyclebin: {error}; failed to restore session file: {rollback_error}"
                )),
            };
        }
    }

    let item = RecycledItem {
        id: id.clone(),
        item_type: "session".to_string(),
        reason: "ManualDelete".to_string(),
        source: "omp".to_string(),
        project_id: project_id.to_string(),
        session_title,
        project_name,
        original_path: session_path.to_string_lossy().to_string(),
        stored_name,
        companion_original_path: artifact_path
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        companion_stored_name,
        moved_at: chrono::Utc::now().to_rfc3339(),
    };
    let mut manifest = load_manifest();
    manifest.items.push(item);
    if let Err(error) = save_manifest(&manifest) {
        let mut rollback_errors = Vec::new();
        if let (Some(artifact_path), Some(companion_stored_path)) =
            (artifact_path.as_ref(), companion_stored_path.as_ref())
        {
            if let Err(rollback_error) = fs::rename(companion_stored_path, artifact_path) {
                rollback_errors.push(rollback_error.to_string());
            }
        }
        if let Err(rollback_error) = fs::rename(&stored_path, session_path) {
            rollback_errors.push(rollback_error.to_string());
        }
        if rollback_errors.is_empty() {
            return Err(format!("Failed to record OMP recyclebin item: {error}"));
        }
        return Err(format!(
            "Failed to record OMP recyclebin item: {error}; rollback failed: {}",
            rollback_errors.join("; ")
        ));
    }

    Ok(id)
}

/// 列出所有回收站条目，按 movedAt 倒序排列。
pub fn list_items() -> Vec<RecycledItem> {
    let mut items = load_manifest().items;
    items.sort_by(|a, b| b.moved_at.cmp(&a.moved_at));
    items
}

/// 将条目还原到 original_path，自动创建父目录。
/// 还原成功后失效对应数据源的 sessions 缓存，避免 UI 不刷新。
pub fn restore_item(id: &str) -> Result<(), String> {
    let mut manifest = load_manifest();
    let pos = manifest
        .items
        .iter()
        .position(|item| item.id == id)
        .ok_or_else(|| format!("Item not found: {id}"))?;
    let item = manifest.items[pos].clone();
    let companion = match (&item.companion_original_path, &item.companion_stored_name) {
        (None, None) => None,
        (Some(original), Some(stored)) => Some((PathBuf::from(original), stored)),
        _ => return Err("Recyclebin companion metadata is incomplete".to_string()),
    };

    let items_dir = get_recyclebin_items_dir()
        .ok_or_else(|| "Cannot determine recyclebin items path".to_string())?;
    let stored_path = items_dir.join(&item.stored_name);
    if !stored_path.exists() {
        return Err(format!("Stored file not found: {stored_path:?}"));
    }
    let original = PathBuf::from(&item.original_path);
    if original.exists() {
        return Err(format!("Destination already exists: {original:?}"));
    }
    if let Some((companion_original, companion_stored_name)) = &companion {
        if companion_original.exists() {
            return Err(format!(
                "Companion destination already exists: {companion_original:?}"
            ));
        }
        let companion_stored_path = items_dir.join(companion_stored_name);
        if !companion_stored_path.exists() {
            return Err(format!(
                "Stored companion not found: {companion_stored_path:?}"
            ));
        }
    }
    if let Some(parent) = original.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create parent dir: {error}"))?;
    }
    if let Some((companion_original, _)) = &companion {
        if let Some(parent) = companion_original.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create companion parent dir: {error}"))?;
        }
    }

    restore_path(&stored_path, &original)?;
    if let Some((companion_original, companion_stored_name)) = &companion {
        let companion_stored_path = items_dir.join(companion_stored_name);
        if let Err(error) = restore_path(&companion_stored_path, companion_original) {
            let rollback = restore_path(&original, &stored_path);
            return match rollback {
                Ok(()) => Err(format!("Failed to restore companion artifact: {error}")),
                Err(rollback_error) => Err(format!(
                    "Failed to restore companion artifact: {error}; failed to return session file to recyclebin: {rollback_error}"
                )),
            };
        }
    }

    manifest.items.remove(pos);
    save_manifest(&manifest)?;

    match item.source.as_str() {
        "claude" => crate::provider::claude::invalidate_cache(),
        "codex" => crate::provider::codex::invalidate_sessions_cache(),
        "grok" => crate::provider::grok::invalidate_sessions_cache(),
        "omp" => crate::provider::omp::invalidate_sessions_cache(),
        _ => {}
    }
    Ok(())
}

fn cross_device_error(err: &std::io::Error) -> bool {
    // EXDEV on Unix; Windows uses a different error string
    matches!(err.raw_os_error(), Some(18))
        || err.to_string().to_lowercase().contains("different device")
        || err.to_string().to_lowercase().contains("different volume")
}

fn restore_path(source: &std::path::Path, destination: &std::path::Path) -> Result<(), String> {
    if let Err(rename_error) = fs::rename(source, destination) {
        if !cross_device_error(&rename_error) {
            return Err(format!("Failed to restore item: {rename_error}"));
        }
        copy_path(source, destination)
            .map_err(|error| format!("Failed to restore item across volumes: {error}"))?;
        remove_path(source)
            .map_err(|error| format!("Restored, but failed to clean recyclebin entry: {error}"))?;
    }
    Ok(())
}

fn copy_path(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if src.is_dir() {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let to = dst.join(entry.file_name());
            copy_path(&entry.path(), &to)?;
        }
        Ok(())
    } else {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(src, dst).map(|_| ())
    }
}

fn remove_path(p: &std::path::Path) -> std::io::Result<()> {
    if p.is_dir() {
        fs::remove_dir_all(p)
    } else {
        fs::remove_file(p)
    }
}

/// 永久删除条目（从 items/ 删文件 + manifest 移除）。
pub fn permanently_delete_item(id: &str) -> Result<(), String> {
    let mut manifest = load_manifest();
    let pos = manifest
        .items
        .iter()
        .position(|item| item.id == id)
        .ok_or_else(|| format!("Item not found: {id}"))?;
    let item = manifest.items[pos].clone();
    let items_dir = get_recyclebin_items_dir()
        .ok_or_else(|| "Cannot determine recyclebin items path".to_string())?;
    if let Some(companion_stored_name) = &item.companion_stored_name {
        let companion_stored_path = items_dir.join(companion_stored_name);
        if companion_stored_path.exists() {
            remove_path(&companion_stored_path)
                .map_err(|error| format!("Failed to delete stored companion: {error}"))?;
        }
    }
    let stored_path = items_dir.join(&item.stored_name);
    if stored_path.exists() {
        remove_path(&stored_path)
            .map_err(|error| format!("Failed to delete stored item: {error}"))?;
    }

    manifest.items.remove(pos);
    save_manifest(&manifest)?;
    Ok(())
}

/// 清空回收站所有条目，返回删除数量。
pub fn empty_recyclebin() -> Result<usize, String> {
    let manifest = load_manifest();
    let count = manifest.items.len();
    if count == 0 {
        return Ok(0);
    }

    let items_dir = get_recyclebin_items_dir()
        .ok_or_else(|| "Cannot determine recyclebin items path".to_string())?;

    for item in &manifest.items {
        let stored_path = items_dir.join(&item.stored_name);
        if stored_path.exists() {
            if stored_path.is_dir() {
                let _ = fs::remove_dir_all(&stored_path);
            } else {
                let _ = fs::remove_file(&stored_path);
            }
        }
        if let Some(companion_stored_name) = &item.companion_stored_name {
            let companion_stored_path = items_dir.join(companion_stored_name);
            if companion_stored_path.exists() {
                let _ = remove_path(&companion_stored_path);
            }
        }
    }

    save_manifest(&RecyclebinManifest::default())?;
    Ok(count)
}
