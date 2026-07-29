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
        RecyclebinManifest { version: 1, items: vec![] }
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
    fs::write(&tmp_path, &json)
        .map_err(|e| format!("Failed to write manifest tmp: {}", e))?;
    fs::rename(&tmp_path, &path)
        .map_err(|e| format!("Failed to rename manifest: {}", e))?;
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
        moved_at: chrono::Utc::now().to_rfc3339(),
    };

    let mut manifest = load_manifest();
    manifest.items.push(item);
    save_manifest(&manifest)?;

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
    let pos = manifest.items.iter().position(|i| i.id == id)
        .ok_or_else(|| format!("Item not found: {}", id))?;
    let item = manifest.items[pos].clone();

    let items_dir = get_recyclebin_items_dir()
        .ok_or_else(|| "Cannot determine recyclebin items path".to_string())?;
    let stored_path = items_dir.join(&item.stored_name);

    if !stored_path.exists() {
        return Err(format!("Stored file not found: {:?}", stored_path));
    }

    let original = std::path::Path::new(&item.original_path);
    if let Some(parent) = original.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent dir: {}", e))?;
    }

    if original.exists() {
        return Err(format!("Destination already exists: {:?}", original));
    }

    // 跨卷的 fs::rename 会失败，回退到 copy + remove
    if let Err(rename_err) = fs::rename(&stored_path, original) {
        if cross_device_error(&rename_err) {
            copy_path(&stored_path, original).map_err(|e| {
                format!("Failed to restore item across volumes: {}", e)
            })?;
            remove_path(&stored_path).map_err(|e| {
                format!("Restored, but failed to clean recyclebin entry: {}", e)
            })?;
        } else {
            return Err(format!("Failed to restore item: {}", rename_err));
        }
    }

    manifest.items.remove(pos);
    save_manifest(&manifest)?;

    // 失效 sessions 缓存，让前端列表显示恢复的条目
    match item.source.as_str() {
        "claude" => crate::provider::claude::invalidate_cache(),
        "codex" => crate::provider::codex::invalidate_sessions_cache(),
        "grok" => crate::provider::grok::invalidate_sessions_cache(),
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
    let pos = manifest.items.iter().position(|i| i.id == id)
        .ok_or_else(|| format!("Item not found: {}", id))?;
    let item = manifest.items[pos].clone();

    let items_dir = get_recyclebin_items_dir()
        .ok_or_else(|| "Cannot determine recyclebin items path".to_string())?;
    let stored_path = items_dir.join(&item.stored_name);

    if stored_path.exists() {
        if stored_path.is_dir() {
            fs::remove_dir_all(&stored_path)
                .map_err(|e| format!("Failed to delete stored dir: {}", e))?;
        } else {
            fs::remove_file(&stored_path)
                .map_err(|e| format!("Failed to delete stored file: {}", e))?;
        }
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
    }

    save_manifest(&RecyclebinManifest::default())?;
    Ok(count)
}
