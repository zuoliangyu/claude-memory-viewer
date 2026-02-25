use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use serde::Deserialize;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use session_core::cli;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatRequest {
    action: String,         // "start" | "continue" | "cancel"
    source: Option<String>, // "claude" | "codex"
    project_path: Option<String>,
    prompt: Option<String>,
    model: Option<String>,
    session_id: Option<String>,
    skip_permissions: Option<bool>,
}

pub async fn chat_ws_handler(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(handle_chat_socket)
}

async fn handle_chat_socket(mut socket: WebSocket) {
    // Channel for sending messages back to the client
    let (tx, mut rx) = mpsc::channel::<String>(100);

    // Track the current child process PID for cancellation
    let cancel_tx = tokio::sync::watch::channel(false);
    let cancel_sender = cancel_tx.0;

    loop {
        tokio::select! {
            // Forward queued messages to the WebSocket
            Some(msg) = rx.recv() => {
                if socket.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }

            // Receive messages from the client
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let request: ChatRequest = match serde_json::from_str(&text) {
                            Ok(r) => r,
                            Err(e) => {
                                let err_msg = serde_json::json!({
                                    "type": "error",
                                    "data": format!("Invalid request: {}", e)
                                }).to_string();
                                let _ = socket.send(Message::Text(err_msg.into())).await;
                                continue;
                            }
                        };

                        match request.action.as_str() {
                            "start" | "continue" => {
                                let source = request.source.unwrap_or_else(|| "claude".to_string());
                                let project_path = request.project_path.unwrap_or_default();
                                let prompt = request.prompt.unwrap_or_default();
                                let model = request.model.unwrap_or_default();
                                let skip_permissions = request.skip_permissions.unwrap_or(false);
                                let resume_id = if request.action == "continue" {
                                    request.session_id.clone()
                                } else {
                                    None
                                };

                                let session_id = request.session_id
                                    .filter(|_| request.action == "continue")
                                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                                // Send session_id to client
                                let sid_msg = serde_json::json!({
                                    "type": "session_id",
                                    "data": &session_id
                                }).to_string();
                                let _ = socket.send(Message::Text(sid_msg.into())).await;

                                // Spawn the CLI process
                                let tx_clone = tx.clone();
                                let mut cancel_rx = cancel_sender.subscribe();

                                tokio::spawn(async move {
                                    if let Err(e) = run_cli_process(
                                        &source,
                                        &project_path,
                                        &prompt,
                                        &model,
                                        skip_permissions,
                                        resume_id.as_deref(),
                                        tx_clone,
                                        &mut cancel_rx,
                                    ).await {
                                        // Error already sent via tx in run_cli_process
                                        tracing::error!("CLI process error: {}", e);
                                    }
                                });
                            }
                            "cancel" => {
                                let _ = cancel_sender.send(true);
                            }
                            _ => {
                                let err_msg = serde_json::json!({
                                    "type": "error",
                                    "data": format!("Unknown action: {}", request.action)
                                }).to_string();
                                let _ = socket.send(Message::Text(err_msg.into())).await;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_cli_process(
    source: &str,
    project_path: &str,
    prompt: &str,
    model: &str,
    skip_permissions: bool,
    resume_session_id: Option<&str>,
    tx: mpsc::Sender<String>,
    cancel_rx: &mut tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let cli_path = cli::find_cli(source)?;

    let mut cmd = Command::new(&cli_path);

    match source {
        "claude" => {
            if let Some(sid) = resume_session_id {
                cmd.arg("--resume").arg(sid);
            }
            cmd.arg("-p").arg(prompt);
            if !model.is_empty() {
                cmd.arg("--model").arg(model);
            }
            cmd.arg("--output-format").arg("stream-json");
            cmd.arg("--verbose");
            if skip_permissions {
                cmd.arg("--dangerously-skip-permissions");
            }
        }
        "codex" => {
            cmd.arg("exec").arg("--json");
            if !model.is_empty() {
                cmd.arg("-m").arg(model);
            }
            if let Some(sid) = resume_session_id {
                cmd.arg("--session").arg(sid);
            }
            cmd.arg(prompt);
        }
        _ => return Err(format!("Unknown source: {}", source)),
    }

    if !project_path.is_empty() {
        cmd.current_dir(project_path);
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn CLI: {}", e))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let tx_stdout = tx.clone();
    let stdout_task = tokio::spawn(async move {
        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let msg = serde_json::json!({
                    "type": "output",
                    "data": line
                })
                .to_string();
                if tx_stdout.send(msg).await.is_err() {
                    break;
                }
            }
        }
    });

    let tx_stderr = tx.clone();
    let stderr_task = tokio::spawn(async move {
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let msg = serde_json::json!({
                    "type": "error",
                    "data": line
                })
                .to_string();
                if tx_stderr.send(msg).await.is_err() {
                    break;
                }
            }
        }
    });

    // Wait for completion or cancellation
    tokio::select! {
        status = child.wait() => {
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            let success = status.map(|s| s.success()).unwrap_or(false);
            let complete_msg = serde_json::json!({
                "type": "complete",
                "success": success
            }).to_string();
            let _ = tx.send(complete_msg).await;
        }
        _ = cancel_rx.changed() => {
            let _ = child.kill().await;
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            let cancel_msg = serde_json::json!({
                "type": "complete",
                "success": false
            }).to_string();
            let _ = tx.send(cancel_msg).await;
        }
    }

    Ok(())
}
