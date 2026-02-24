use lru::LruCache;
use parking_lot::Mutex;
use std::num::NonZeroUsize;

use crate::models::message::DisplayMessage;

/// Application state shared across commands
#[allow(dead_code)]
pub struct AppState {
    /// LRU cache for parsed session messages (key: "encodedName/sessionId")
    pub message_cache: Mutex<LruCache<String, Vec<DisplayMessage>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            message_cache: Mutex::new(LruCache::new(NonZeroUsize::new(20).unwrap())),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
