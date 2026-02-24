use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::Arc;
use tokio::sync::broadcast;

use session_core::parser::path_encoder::get_projects_dir;
use session_core::provider::codex;

/// Shared broadcast sender for file change events
pub type FsChangeTx = Arc<broadcast::Sender<Vec<String>>>;

/// Create the broadcast channel and start the file watcher
pub fn start_file_watcher() -> FsChangeTx {
    let (tx, _) = broadcast::channel::<Vec<String>>(64);
    let tx = Arc::new(tx);
    let tx_clone = tx.clone();

    std::thread::spawn(move || {
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

        for event in notify_rx {
            match event {
                Ok(event) => {
                    let relevant = event.paths.iter().any(|p| {
                        p.extension()
                            .map(|e| e == "jsonl" || e == "json")
                            .unwrap_or(false)
                    });

                    if relevant {
                        let paths: Vec<String> = event
                            .paths
                            .iter()
                            .map(|p| p.to_string_lossy().to_string())
                            .collect();

                        let _ = tx_clone.send(paths);
                    }
                }
                Err(e) => {
                    tracing::warn!("Watch error: {}", e);
                }
            }
        }
    });

    tx
}

/// WebSocket handler for file change events
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    axum::extract::State(tx): axum::extract::State<FsChangeTx>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, tx))
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
