use session_core::provider::claude;
use session_core::recyclebin::{self, RecycledItem};

#[tauri::command]
pub fn list_recycled_items() -> Result<Vec<RecycledItem>, String> {
    Ok(recyclebin::list_items())
}

#[tauri::command]
pub fn restore_recycled_item(id: String) -> Result<(), String> {
    recyclebin::restore_item(&id)
}

#[tauri::command]
pub fn permanently_delete_recycled_item(id: String) -> Result<(), String> {
    recyclebin::permanently_delete_item(&id)
}

#[tauri::command]
pub fn empty_recyclebin() -> Result<usize, String> {
    recyclebin::empty_recyclebin()
}

#[tauri::command]
pub fn cleanup_orphan_dirs(source: String) -> Result<usize, String> {
    match source.as_str() {
        "claude" => claude::cleanup_all_orphan_dirs(),
        _ => Err(format!(
            "Orphan dir cleanup not supported for source: {}",
            source
        )),
    }
}
