use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarksFile {
    pub version: u32,
    pub bookmarks: Vec<Bookmark>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub id: String,
    pub source: String,
    pub project_id: String,
    pub session_id: String,
    pub file_path: String,
    pub message_id: Option<String>,
    pub preview: String,
    pub session_title: String,
    pub project_name: String,
    pub created_at: String,
}

fn bookmarks_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
    Ok(home.join(".session-viewer-bookmarks.json"))
}

pub fn load_bookmarks() -> BookmarksFile {
    let path = match bookmarks_path() {
        Ok(p) => p,
        Err(_) => {
            return BookmarksFile {
                version: 1,
                bookmarks: vec![],
            }
        }
    };
    if !path.exists() {
        return BookmarksFile {
            version: 1,
            bookmarks: vec![],
        };
    }
    let data = match fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => {
            return BookmarksFile {
                version: 1,
                bookmarks: vec![],
            }
        }
    };
    serde_json::from_str(&data).unwrap_or(BookmarksFile {
        version: 1,
        bookmarks: vec![],
    })
}

fn save_bookmarks(file: &BookmarksFile) -> Result<(), String> {
    let path = bookmarks_path()?;
    let json = serde_json::to_string_pretty(file)
        .map_err(|e| format!("Failed to serialize bookmarks: {}", e))?;

    // Atomic write: write to tmp then rename
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, &json).map_err(|e| format!("Failed to write bookmarks tmp: {}", e))?;
    fs::rename(&tmp_path, &path).map_err(|e| format!("Failed to rename bookmarks file: {}", e))?;
    Ok(())
}

fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}", ts)
}

pub fn add_bookmark(bookmark: Bookmark) -> Result<Bookmark, String> {
    let mut file = load_bookmarks();

    // Deduplicate: same session + message_id
    let exists = file.bookmarks.iter().any(|b| {
        b.source == bookmark.source
            && b.session_id == bookmark.session_id
            && b.message_id == bookmark.message_id
    });
    if exists {
        return Err("Bookmark already exists".to_string());
    }

    let mut bm = bookmark;
    if bm.id.is_empty() {
        bm.id = generate_id();
    }
    if bm.created_at.is_empty() {
        bm.created_at = chrono::Utc::now().to_rfc3339();
    }

    file.bookmarks.push(bm.clone());
    save_bookmarks(&file)?;
    Ok(bm)
}

pub fn remove_bookmark(id: &str) -> Result<(), String> {
    let mut file = load_bookmarks();
    let len_before = file.bookmarks.len();
    file.bookmarks.retain(|b| b.id != id);
    if file.bookmarks.len() == len_before {
        return Err("Bookmark not found".to_string());
    }
    save_bookmarks(&file)?;
    Ok(())
}

pub fn list_bookmarks(source: Option<&str>) -> Vec<Bookmark> {
    let file = load_bookmarks();
    match source {
        Some(s) => file
            .bookmarks
            .into_iter()
            .filter(|b| b.source == s)
            .collect(),
        None => file.bookmarks,
    }
}
