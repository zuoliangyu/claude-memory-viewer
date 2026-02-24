use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use session_core::parser::path_encoder::get_projects_dir;
use session_core::provider::codex;

/// Minimum interval between emitting fs-change events to the frontend.
const DEBOUNCE_DURATION: Duration = Duration::from_millis(300);

/// Start watching both Claude and Codex directories for changes.
/// Emits "fs-change" events to the frontend when files are modified.
/// Events are debounced to avoid flooding the frontend during batch operations.
pub fn start_watcher(app_handle: AppHandle) -> Result<(), String> {
    let claude_dir = get_projects_dir();
    let codex_dir = codex::get_sessions_dir();

    // At least one directory must exist
    if claude_dir.as_ref().map(|d| d.exists()).unwrap_or(false)
        || codex_dir.as_ref().map(|d| d.exists()).unwrap_or(false)
    {
        // ok, proceed
    } else {
        return Err("Neither Claude nor Codex directory exists".to_string());
    }

    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

        let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Failed to create watcher: {}", e);
                return;
            }
        };

        // Watch Claude projects directory
        if let Some(ref dir) = claude_dir {
            if dir.exists() {
                if let Err(e) = watcher.watch(dir, RecursiveMode::Recursive) {
                    eprintln!("Failed to watch Claude directory: {}", e);
                }
            }
        }

        // Watch Codex sessions directory
        if let Some(ref dir) = codex_dir {
            if dir.exists() {
                if let Err(e) = watcher.watch(dir, RecursiveMode::Recursive) {
                    eprintln!("Failed to watch Codex directory: {}", e);
                }
            }
        }

        let mut last_emit = Instant::now() - DEBOUNCE_DURATION;

        for event in rx {
            match event {
                Ok(event) => {
                    let relevant = event.paths.iter().any(|p| {
                        p.extension()
                            .map(|e| e == "jsonl" || e == "json")
                            .unwrap_or(false)
                    });

                    if relevant && last_emit.elapsed() >= DEBOUNCE_DURATION {
                        let paths: Vec<String> = event
                            .paths
                            .iter()
                            .map(|p| p.to_string_lossy().to_string())
                            .collect();

                        let _ = app_handle.emit("fs-change", paths);
                        last_emit = Instant::now();
                    }
                }
                Err(e) => {
                    eprintln!("Watch error: {}", e);
                }
            }
        }
    });

    Ok(())
}
