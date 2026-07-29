use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use session_core::parser::path_encoder::get_projects_dir;
use session_core::provider::{claude, codex, grok};

/// Minimum interval between emitting fs-change events to the frontend.
const DEBOUNCE_DURATION: Duration = Duration::from_millis(300);

/// Start watching both Claude and Codex directories for changes.
/// Emits "fs-change" events to the frontend when files are modified.
/// Events are debounced to avoid flooding the frontend during batch operations.
pub fn start_watcher(app_handle: AppHandle) -> Result<(), String> {
    let claude_dir = get_projects_dir();
    let codex_dir = codex::get_sessions_dir();
    let grok_dir = grok::get_sessions_dir();

    // At least one directory must exist
    if claude_dir.as_ref().map(|d| d.exists()).unwrap_or(false)
        || codex_dir.as_ref().map(|d| d.exists()).unwrap_or(false)
        || grok_dir.as_ref().map(|d| d.exists()).unwrap_or(false)
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
        if let Some(ref dir) = grok_dir {
            if dir.exists() { let _ = watcher.watch(dir, RecursiveMode::Recursive); }
        }

        let mut last_emit = Instant::now() - DEBOUNCE_DURATION;

        for event in rx {
            match event {
                Ok(event) => {
                    let relevant = event.paths.iter().any(|p| {
                        let is_meta = p
                            .file_name()
                            .map(|n| n == ".session-viewer-meta.json")
                            .unwrap_or(false);
                        !is_meta
                            && p.extension()
                                .map(|e| e == "jsonl" || e == "json")
                                .unwrap_or(false)
                    });

                    if relevant && last_emit.elapsed() >= DEBOUNCE_DURATION {
                        let is_claude_change = claude_dir.as_ref().map(|dir| {
                            event.paths.iter().any(|path| path.starts_with(dir))
                        }).unwrap_or(false);
                        let is_codex_change = codex_dir.as_ref().map(|dir| {
                            event.paths.iter().any(|path| path.starts_with(dir))
                        }).unwrap_or(false);
                        let is_grok_change = grok_dir.as_ref().map(|dir| event.paths.iter().any(|path| path.starts_with(dir))).unwrap_or(false);

                        // Hand each provider only the paths under its own
                        // directory, so it can surgically update just the
                        // affected projects/files instead of wiping everything.
                        if is_claude_change {
                            if let Some(ref dir) = claude_dir {
                                let paths: Vec<PathBuf> = event
                                    .paths
                                    .iter()
                                    .filter(|p| p.starts_with(dir))
                                    .cloned()
                                    .collect();
                                if !paths.is_empty() {
                                    claude::invalidate_paths(&paths);
                                }
                            }
                        }
                        if is_codex_change {
                            if let Some(ref dir) = codex_dir {
                                let paths: Vec<PathBuf> = event
                                    .paths
                                    .iter()
                                    .filter(|p| p.starts_with(dir))
                                    .cloned()
                                    .collect();
                                if !paths.is_empty() {
                                    codex::invalidate_paths(&paths);
                                }
                            }
                        }
                        if is_grok_change { grok::invalidate_sessions_cache(); }

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
