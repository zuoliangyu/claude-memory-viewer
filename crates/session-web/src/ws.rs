use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::Response;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

use crate::{require_ws_auth, AppToken, WsTicketStore};

/// Minimum interval between sending file change events.
/// Docker volume mounts can produce frequent inotify events,
/// so use a longer debounce to avoid flooding clients.
const DEBOUNCE_DURATION: Duration = Duration::from_millis(1000);

use session_core::parser::path_encoder::get_projects_dir;
use session_core::provider::{claude, codex, grok};

/// Shared broadcast sender for file change events
pub type FsChangeTx = Arc<broadcast::Sender<Vec<String>>>;

/// Create the broadcast channel and start the file watcher. The watcher
/// runs in a supervisor loop: if `notify` fails to start or its event
/// channel closes for any reason, we sleep briefly and respawn so a
/// transient failure (network share unmounting, OS resource exhaustion,
/// etc.) doesn't silently leave the server without change notifications
/// for the rest of its lifetime.
pub fn start_file_watcher() -> FsChangeTx {
    let (tx, _) = broadcast::channel::<Vec<String>>(64);
    let tx = Arc::new(tx);
    let tx_clone = tx.clone();

    std::thread::spawn(move || {
        loop {
            run_file_watcher_once(&tx_clone);
            // The inner function only returns when the watcher dies. Pause
            // before respawning so a chronic failure can't pin a CPU core.
            tracing::warn!("file watcher exited; respawning in 5s");
            std::thread::sleep(Duration::from_secs(5));
        }
    });

    tx
}

/// One pass of the file watcher loop. Returns when the watcher fails or its
/// event stream ends.
fn run_file_watcher_once(tx_clone: &broadcast::Sender<Vec<String>>) {
    let (notify_tx, notify_rx) = std::sync::mpsc::channel();

    let mut watcher = match RecommendedWatcher::new(notify_tx, Config::default()) {
        Ok(w) => w,
        Err(e) => {
            tracing::error!("Failed to create file watcher: {}", e);
            return;
        }
    };

    // Watch Claude projects directory
    if let Some(dir) = get_projects_dir() {
        if dir.exists() {
            if let Err(e) = watcher.watch(&dir, RecursiveMode::Recursive) {
                tracing::warn!("Failed to watch Claude directory: {}", e);
            }
        }
    }

    // Watch Codex sessions directory
    if let Some(dir) = codex::get_sessions_dir() {
        if dir.exists() {
            if let Err(e) = watcher.watch(&dir, RecursiveMode::Recursive) {
                tracing::warn!("Failed to watch Codex directory: {}", e);
            }
        }
    }
    if let Some(dir) = grok::get_sessions_dir() {
        if dir.exists() { let _ = watcher.watch(&dir, RecursiveMode::Recursive); }
    }

    let mut last_emit = Instant::now() - DEBOUNCE_DURATION;

    for event in notify_rx {
        match event {
            Ok(event) => {
                let relevant = event.paths.iter().any(|p| {
                    p.extension()
                        .map(|e| e == "jsonl" || e == "json")
                        .unwrap_or(false)
                });

                if relevant && last_emit.elapsed() >= DEBOUNCE_DURATION {
                    // Surgically update only the affected projects/files (same
                    // path-partitioned approach as the Tauri watcher) instead of
                    // wiping the whole cache on every change.
                    if let Some(dir) = get_projects_dir() {
                        let claude_paths: Vec<std::path::PathBuf> = event
                            .paths
                            .iter()
                            .filter(|p| p.starts_with(&dir))
                            .cloned()
                            .collect();
                        if !claude_paths.is_empty() {
                            claude::invalidate_paths(&claude_paths);
                        }
                    }
                    if let Some(dir) = codex::get_sessions_dir() {
                        let codex_paths: Vec<std::path::PathBuf> = event
                            .paths
                            .iter()
                            .filter(|p| p.starts_with(&dir))
                            .cloned()
                            .collect();
                        if !codex_paths.is_empty() {
                            codex::invalidate_paths(&codex_paths);
                        }
                    }
                    if let Some(dir) = grok::get_sessions_dir() {
                        if event.paths.iter().any(|path| path.starts_with(&dir)) { grok::invalidate_sessions_cache(); }
                    }

                    let paths: Vec<String> = event
                        .paths
                        .iter()
                        .map(|p| p.to_string_lossy().to_string())
                        .collect();

                    let _ = tx_clone.send(paths);
                    last_emit = Instant::now();
                }
            }
            Err(e) => {
                tracing::warn!("Watch error: {}", e);
            }
        }
    }
}

/// WebSocket handler for file change events
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    axum::extract::State(tx): axum::extract::State<FsChangeTx>,
    axum::extract::Extension(app_token): axum::extract::Extension<AppToken>,
    axum::extract::Extension(tickets): axum::extract::Extension<WsTicketStore>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(query): axum::extract::Query<WsAuthQuery>,
) -> Result<Response, StatusCode> {
    require_ws_auth(&headers, query.ticket.as_deref(), &app_token, &tickets)?;
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, tx)))
}

#[derive(serde::Deserialize)]
pub struct WsAuthQuery {
    /// Single-use ticket from POST /api/auth/ws-ticket.
    pub ticket: Option<String>,
}

async fn handle_socket(mut socket: WebSocket, tx: FsChangeTx) {
    let mut rx = tx.subscribe();

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(paths) => {
                        let json = serde_json::json!({
                            "type": "fs-change",
                            "paths": paths,
                        });
                        if socket.send(Message::Text(json.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {} // ignore other messages
                }
            }
        }
    }
}
