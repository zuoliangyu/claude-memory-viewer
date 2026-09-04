use std::path::Path;

use session_core::export::{render_session, ExportFormat};

/// 渲染单个会话为指定格式的字符串。文件名由前端决定，这里只返回内容。
#[tauri::command]
pub fn export_session(source: String, file_path: String, format: String) -> Result<String, String> {
    let fmt = ExportFormat::parse(&format)?;
    render_session(&source, &file_path, fmt)
}

/// 把导出内容写入用户通过保存/选目录对话框选定的路径。
///
/// 轻量守卫：只允许写 `.json` / `.md` / `.html` 后缀（导出场景），且父目录必须
/// 已存在，避免被当成任意文件写入接口。
#[tauri::command]
pub fn write_export_file(path: String, content: String) -> Result<(), String> {
    let p = Path::new(&path);

    let ext_ok = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_ascii_lowercase().as_str(), "json" | "md" | "html"))
        .unwrap_or(false);
    if !ext_ok {
        return Err("Export file must end with .json, .md or .html".to_string());
    }

    match p.parent() {
        Some(parent) if parent.as_os_str().is_empty() || parent.is_dir() => {}
        Some(parent) => {
            return Err(format!(
                "Target directory does not exist: {}",
                parent.display()
            ))
        }
        None => return Err("Invalid export path".to_string()),
    }

    std::fs::write(p, content.as_bytes()).map_err(|e| format!("Failed to write export file: {}", e))
}
