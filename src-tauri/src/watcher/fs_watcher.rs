use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use session_core::parser::path_encoder::get_projects_dir;
use session_core::provider::{claude, codex, grok, omp};
use session_core::watcher_batch::collect_until_quiet;

/// Minimum interval between emitting fs-change events to the frontend.
const DEBOUNCE_DURATION: Duration = Duration::from_millis(300);
const MAX_BATCH_DURATION: Duration = Duration::from_secs(1);

/// Start watching Claude, Codex and Grok session directories for changes.
/// Emits "fs-change" events to the frontend when files are modified.
/// Events are debounced to avoid flooding the frontend during batch operations.
pub fn start_watcher(app_handle: AppHandle) -> Result<(), String> {
    let claude_dir = get_projects_dir();
    let codex_dir = codex::get_sessions_dir();
    let grok_dir = grok::get_sessions_dir();
    let omp_dir = omp::get_sessions_dir();

    // At least one directory must exist
    if claude_dir.as_ref().map(|dir| dir.exists()).unwrap_or(false)
        || codex_dir.as_ref().map(|dir| dir.exists()).unwrap_or(false)
        || grok_dir.as_ref().map(|dir| dir.exists()).unwrap_or(false)
        || omp_dir.as_ref().map(|dir| dir.exists()).unwrap_or(false)
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
            if dir.exists() {
                let _ = watcher.watch(dir, RecursiveMode::Recursive);
            }
        }
        if let Some(ref dir) = omp_dir {
            if dir.exists() {
                let _ = watcher.watch(dir, RecursiveMode::Recursive);
            }
        }

        while let Ok(first) = rx.recv() {
            let events =
                collect_until_quiet(first, DEBOUNCE_DURATION, MAX_BATCH_DURATION, |timeout| {
                    rx.recv_timeout(timeout)
                });
            let mut changed = HashSet::new();
            for event in events {
                match event {
                    Ok(event) => {
                        changed.extend(event.paths.into_iter().filter(|path| {
                            let is_meta = path
                                .file_name()
                                .map(|name| name == ".session-viewer-meta.json")
                                .unwrap_or(false);
                            !is_meta
                                && path
                                    .extension()
                                    .map(|ext| ext == "jsonl" || ext == "json")
                                    .unwrap_or(false)
                        }));
                    }
                    Err(error) => eprintln!("Watch error: {error}"),
                }
            }

            if !changed.is_empty() {
                let paths: Vec<PathBuf> = changed.into_iter().collect();
                let is_claude_change = claude_dir
                    .as_ref()
                    .map(|dir| paths.iter().any(|path| path.starts_with(dir)))
                    .unwrap_or(false);
                let is_codex_change = codex_dir
                    .as_ref()
                    .map(|dir| paths.iter().any(|path| path.starts_with(dir)))
                    .unwrap_or(false);
                let is_grok_change = grok_dir
                    .as_ref()
                    .map(|dir| paths.iter().any(|path| path.starts_with(dir)))
                    .unwrap_or(false);
                let is_omp_change = omp_dir
                    .as_ref()
                    .map(|dir| paths.iter().any(|path| path.starts_with(dir)))
                    .unwrap_or(false);

                // Hand each provider only the paths under its own
                // directory, so it can surgically update just the
                // affected projects/files instead of wiping everything.
                if is_claude_change {
                    if let Some(ref dir) = claude_dir {
                        let provider_paths: Vec<PathBuf> = paths
                            .iter()
                            .filter(|p| p.starts_with(dir))
                            .cloned()
                            .collect();
                        if !provider_paths.is_empty() {
                            claude::invalidate_paths(&provider_paths);
                        }
                    }
                }
                if is_codex_change {
                    if let Some(ref dir) = codex_dir {
                        let provider_paths: Vec<PathBuf> = paths
                            .iter()
                            .filter(|p| p.starts_with(dir))
                            .cloned()
                            .collect();
                        if !provider_paths.is_empty() {
                            codex::invalidate_paths(&provider_paths);
                        }
                    }
                }
                if is_grok_change {
                    if let Some(ref dir) = grok_dir {
                        let provider_paths: Vec<PathBuf> = paths
                            .iter()
                            .filter(|p| p.starts_with(dir))
                            .cloned()
                            .collect();
                        if !provider_paths.is_empty() {
                            grok::invalidate_paths(&provider_paths);
                        }
                    }
                }
                if is_omp_change {
                    if let Some(ref dir) = omp_dir {
                        let provider_paths: Vec<PathBuf> = paths
                            .iter()
                            .filter(|path| path.starts_with(dir))
                            .cloned()
                            .collect();
                        if !provider_paths.is_empty() {
                            omp::invalidate_paths(&provider_paths);
                        }
                    }
                }

                let paths: Vec<String> = paths
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();

                let _ = app_handle.emit("fs-change", paths);
            }
        }
    });

    Ok(())
}
