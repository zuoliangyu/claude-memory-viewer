use lru::LruCache;
use parking_lot::Mutex;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use crate::models::message::{DisplayMessage, PaginatedMessages};

const MESSAGE_CACHE_CAPACITY: usize = 20;
const WARM_TAIL_PAGES: usize = 4;

#[derive(Debug, Clone)]
pub struct CachedMessages {
    modified_key: u64,
    total: usize,
    range_start: usize,
    messages: Vec<DisplayMessage>,
    is_complete: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct PageBounds {
    pub start: usize,
    pub end: usize,
    pub has_more: bool,
}

impl CachedMessages {
    fn full(modified_key: u64, messages: Vec<DisplayMessage>) -> Self {
        let total = messages.len();
        Self {
            modified_key,
            total,
            range_start: 0,
            messages,
            is_complete: true,
        }
    }

    fn partial(
        modified_key: u64,
        total: usize,
        range_start: usize,
        messages: Vec<DisplayMessage>,
    ) -> Self {
        let is_complete = range_start == 0 && messages.len() == total;
        Self {
            modified_key,
            total,
            range_start,
            messages,
            is_complete,
        }
    }

    fn to_page(&self, page: usize, page_size: usize, from_end: bool) -> Option<PaginatedMessages> {
        paginate_from_range(
            &self.messages,
            self.total,
            page,
            page_size,
            from_end,
            self.range_start,
        )
    }

    fn should_replace(&self, incoming: &Self) -> bool {
        if self.modified_key != incoming.modified_key {
            return true;
        }
        if incoming.is_complete && !self.is_complete {
            return true;
        }
        if self.is_complete {
            return false;
        }

        let self_end = self.range_start.saturating_add(self.messages.len());
        let incoming_end = incoming.range_start.saturating_add(incoming.messages.len());
        incoming.range_start <= self.range_start
            && incoming_end >= self_end
            && (incoming.range_start < self.range_start || incoming_end > self_end)
    }
}

fn global_message_cache() -> &'static Mutex<LruCache<String, CachedMessages>> {
    static MESSAGE_CACHE: OnceLock<Mutex<LruCache<String, CachedMessages>>> = OnceLock::new();
    MESSAGE_CACHE.get_or_init(|| {
        Mutex::new(LruCache::new(
            NonZeroUsize::new(MESSAGE_CACHE_CAPACITY).unwrap(),
        ))
    })
}

fn cache_key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub fn clear_message_cache() {
    global_message_cache().lock().clear();
}

pub fn clear_message_cache_for_path(path: &Path) {
    let key = cache_key(path);
    let _ = global_message_cache().lock().pop(&key);
}

pub fn file_modified_key(path: &Path) -> Result<u64, String> {
    let modified = std::fs::metadata(path)
        .map_err(|e| format!("Failed to read metadata: {}", e))?
        .modified()
        .map_err(|e| format!("Failed to read modified time: {}", e))?;
    let duration = modified.duration_since(UNIX_EPOCH).unwrap_or_default();
    Ok(duration
        .as_secs()
        .saturating_mul(1_000_000_000)
        .saturating_add(duration.subsec_nanos() as u64))
}

pub fn page_bounds(total: usize, page: usize, page_size: usize, from_end: bool) -> PageBounds {
    if from_end {
        let end = total.saturating_sub(page.saturating_mul(page_size));
        let start = end.saturating_sub(page_size);
        PageBounds {
            start,
            end,
            has_more: start > 0,
        }
    } else {
        let start = page.saturating_mul(page_size);
        let end = start.saturating_add(page_size).min(total);
        PageBounds {
            start,
            end,
            has_more: end < total,
        }
    }
}

pub fn tail_window_len(page: usize, page_size: usize) -> usize {
    let required = page.saturating_add(1).saturating_mul(page_size);
    let warm = page_size.saturating_mul(WARM_TAIL_PAGES);
    required.max(warm)
}

pub fn paginate_from_range(
    messages: &[DisplayMessage],
    total: usize,
    page: usize,
    page_size: usize,
    from_end: bool,
    range_start: usize,
) -> Option<PaginatedMessages> {
    let bounds = page_bounds(total, page, page_size, from_end);
    if bounds.end < bounds.start {
        return None;
    }
    if !messages.is_empty() && bounds.end > bounds.start {
        let range_end = range_start.saturating_add(messages.len());
        if bounds.start < range_start || bounds.end > range_end {
            return None;
        }
    } else if bounds.end > 0 && messages.is_empty() {
        return None;
    }

    let local_start = bounds.start.saturating_sub(range_start);
    let local_end = bounds.end.saturating_sub(range_start);

    Some(PaginatedMessages {
        messages: messages[local_start..local_end].to_vec(),
        total,
        page,
        page_size,
        has_more: bounds.has_more,
    })
}

fn get_cache_entry(path: &Path) -> Result<Option<CachedMessages>, String> {
    let key = cache_key(path);
    let modified_key = file_modified_key(path)?;
    let mut cache = global_message_cache().lock();

    if let Some(entry) = cache.get(&key).cloned() {
        if entry.modified_key == modified_key {
            return Ok(Some(entry));
        }
    }

    let _ = cache.pop(&key);
    Ok(None)
}

fn put_cache_entry(path: &Path, entry: CachedMessages) -> Result<(), String> {
    let key = cache_key(path);
    let mut cache = global_message_cache().lock();

    if let Some(existing) = cache.get(&key).cloned() {
        if !existing.should_replace(&entry) {
            return Ok(());
        }
    }

    cache.put(key, entry);
    Ok(())
}

pub fn get_cached_full_messages(path: &Path) -> Result<Option<Vec<DisplayMessage>>, String> {
    Ok(get_cache_entry(path)?.and_then(|entry| entry.is_complete.then_some(entry.messages)))
}

/// Try to satisfy a range request `[start, end)` directly from the cache.
/// Returns `(slice, total)` if the cached entry's range fully contains the
/// requested window. The slice is in absolute index order (i.e. the first
/// element corresponds to message at index `start`).
pub fn get_cached_range(
    path: &Path,
    start: usize,
    end: usize,
) -> Result<Option<(Vec<DisplayMessage>, usize)>, String> {
    if end <= start {
        return Ok(None);
    }
    let entry = match get_cache_entry(path)? {
        Some(e) => e,
        None => return Ok(None),
    };

    let cached_end = entry.range_start.saturating_add(entry.messages.len());
    if start < entry.range_start || end > cached_end {
        return Ok(None);
    }

    let local_start = start - entry.range_start;
    let local_end = end - entry.range_start;
    if local_end > entry.messages.len() {
        return Ok(None);
    }
    let slice = entry.messages[local_start..local_end].to_vec();
    Ok(Some((slice, entry.total)))
}

pub fn get_cached_page(
    path: &Path,
    page: usize,
    page_size: usize,
    from_end: bool,
) -> Result<Option<PaginatedMessages>, String> {
    Ok(get_cache_entry(path)?.and_then(|entry| entry.to_page(page, page_size, from_end)))
}

pub fn store_full_messages(path: &Path, messages: &[DisplayMessage]) -> Result<(), String> {
    let modified_key = file_modified_key(path)?;
    put_cache_entry(path, CachedMessages::full(modified_key, messages.to_vec()))
}

pub fn store_partial_messages(
    path: &Path,
    total: usize,
    range_start: usize,
    messages: &[DisplayMessage],
) -> Result<(), String> {
    let modified_key = file_modified_key(path)?;
    put_cache_entry(
        path,
        CachedMessages::partial(modified_key, total, range_start, messages.to_vec()),
    )
}

/// Application state shared across commands.
/// The underlying cache is global so Tauri and Web can reuse the same logic.
#[allow(dead_code)]
pub struct AppState {
    pub message_cache: &'static Mutex<LruCache<String, CachedMessages>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            message_cache: global_message_cache(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
