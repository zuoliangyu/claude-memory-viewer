use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, watch, Mutex};

use session_core::cli;
use session_core::cli_config::{self, ResolvedCliCredentials};
use session_core::codex_app_server::CodexAppServer;

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
    cli_path: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
}

pub async fn chat_ws_handler(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(handle_chat_socket)
}

fn canonicalize_existing_dir(path: &str) -> Result<PathBuf, String> {
    if path.trim().is_empty() {
        return Err("Project path is required".to_string());
    }

    let canonical = PathBuf::from(path)
        .canonicalize()
        .map_err(|e| format!("Failed to resolve project path: {}", e))?;

    if !canonical.is_dir() {
        return Err("Project path must be a directory".to_string());
    }

    Ok(canonical)
}

fn allowed_project_roots(source: &str) -> Result<Vec<PathBuf>, String> {
    let projects = if source == "codex" {
        session_core::provider::codex::get_projects()
    } else if source == "omp" {
        session_core::provider::omp::get_projects()
    } else {
        session_core::provider::claude::get_projects()
    }?;
    let mut roots = Vec::new();
    for project in projects {
        let display_path = project.display_path;
        if display_path.trim().is_empty() {
            continue;
        }

        let path = PathBuf::from(&display_path);
        if !path.exists() || !path.is_dir() {
            continue;
        }

        if let Ok(canonical) = path.canonicalize() {
            roots.push(canonical);
        }
    }

    roots.sort();
    roots.dedup();

    if roots.is_empty() {
        Err(format!(
            "No accessible {} project directories are available",
            source
        ))
    } else {
        Ok(roots)
    }
}

fn path_is_within_any_root(path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}

fn resolve_project_dir(source: &str, project_path: &str) -> Result<PathBuf, String> {
    let requested = canonicalize_existing_dir(project_path)?;
    let allowed_roots = allowed_project_roots(source)?;

    if path_is_within_any_root(&requested, &allowed_roots) {
        Ok(requested)
    } else {
        Err(format!(
            "Project path is outside the allowed {} project directories",
            source
        ))
    }
}

/// Per-session Codex turn state (thread_id + active turn_id) for cancel routing.
#[derive(Default, Clone)]
struct CodexTurnState {
    thread_id: Option<String>,
    turn_id: Option<String>,
    credentials: Option<ResolvedCliCredentials>,
}

type ClaudeCancelMap = Arc<Mutex<HashMap<String, watch::Sender<bool>>>>;
type CodexStateMap = Arc<Mutex<HashMap<String, CodexTurnState>>>;

async fn handle_chat_socket(mut socket: WebSocket) {
    // Channel for sending messages back to the client
    let (tx, mut rx) = mpsc::channel::<String>(100);

    // Per-session cancel watches for Claude tasks.
    let claude_cancels: ClaudeCancelMap = Arc::new(Mutex::new(HashMap::new()));
    // Per-session Codex turn state for cancel routing.
    let codex_states: CodexStateMap = Arc::new(Mutex::new(HashMap::new()));

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
                                let raw_source =
                                    request.source.unwrap_or_else(|| "claude".to_string());
                                let source = match cli::normalize_source(&raw_source) {
                                    Ok(source) => source.to_string(),
                                    Err(e) => {
                                        let err_msg = serde_json::json!({
                                            "type": "error",
                                            "data": e
                                        }).to_string();
                                        let _ = socket.send(Message::Text(err_msg.into())).await;
                                        continue;
                                    }
                                };
                                let project_path = request.project_path.unwrap_or_default();
                                let prompt = request.prompt.unwrap_or_default();
                                let model = request.model.unwrap_or_default();
                                let skip_permissions = request.skip_permissions.unwrap_or(false);
                                let cli_path = request.cli_path.unwrap_or_default();
                                let api_key = request.api_key.unwrap_or_default();
                                let base_url = request.base_url.unwrap_or_default();
                                let resume_id = if request.action == "continue" {
                                    request.session_id.clone()
                                } else {
                                    None
                                };

                                // The routing id is the client-supplied stream key
                                // (or a fresh one if absent). Every frame emitted
                                // for this turn carries `sessionId: routing_id` so
                                // the client can demux concurrent panes.
                                let routing_id = request.session_id
                                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                                // Echo session_id immediately so the caller's
                                // start/continue Promise can resolve. Codex will
                                // overwrite `data` with the real thread_id once
                                // thread/start (or thread/resume) returns.
                                if source != "codex" {
                                    let sid_msg = serde_json::json!({
                                        "type": "session_id",
                                        "data": &routing_id,
                                        "sessionId": &routing_id,
                                    }).to_string();
                                    let _ = socket.send(Message::Text(sid_msg.into())).await;
                                }

                                let tx_task = tx.clone();
                                let routing_for_task = routing_id.clone();

                                if source == "codex" {
                                    let codex_states_clone = codex_states.clone();
                                    tokio::spawn(async move {
                                        let outcome = run_codex_chat_via_app_server(
                                            routing_for_task.clone(),
                                            &project_path,
                                            &prompt,
                                            &model,
                                            resume_id,
                                            &api_key,
                                            &base_url,
                                            tx_task.clone(),
                                            codex_states_clone.clone(),
                                        ).await;
                                        if let Err(e) = outcome {
                                            tracing::error!("Codex app-server error: {}", e);
                                            let err_msg = serde_json::json!({
                                                "type": "error",
                                                "data": e,
                                                "sessionId": &routing_for_task,
                                            }).to_string();
                                            let _ = tx_task.send(err_msg).await;
                                            let complete_msg = serde_json::json!({
                                                "type": "complete",
                                                "success": false,
                                                "sessionId": &routing_for_task,
                                            }).to_string();
                                            let _ = tx_task.send(complete_msg).await;
                                        }
                                        // Always drop per-session state at the
                                        // end of the turn so it can't haunt the
                                        // next request on this socket.
                                        codex_states_clone.lock().await.remove(&routing_for_task);
                                    });
                                } else {
                                    // Allocate a per-session cancel watch so a
                                    // cancel on one pane never kills another.
                                    let (cancel_tx, mut cancel_rx) = watch::channel(false);
                                    claude_cancels
                                        .lock()
                                        .await
                                        .insert(routing_id.clone(), cancel_tx);
                                    let claude_cancels_clone = claude_cancels.clone();
                                    tokio::spawn(async move {
                                        let outcome = run_cli_process(
                                            &source,
                                            &project_path,
                                            &prompt,
                                            &model,
                                            skip_permissions,
                                            resume_id.as_deref(),
                                            &cli_path,
                                            &api_key,
                                            &base_url,
                                            routing_for_task.clone(),
                                            tx_task.clone(),
                                            &mut cancel_rx,
                                        ).await;
                                        if let Err(e) = outcome {
                                            tracing::error!("CLI process error: {}", e);
                                            let err_msg = serde_json::json!({
                                                "type": "error",
                                                "data": e,
                                                "sessionId": &routing_for_task,
                                            }).to_string();
                                            let _ = tx_task.send(err_msg).await;
                                            let complete_msg = serde_json::json!({
                                                "type": "complete",
                                                "success": false,
                                                "sessionId": &routing_for_task,
                                            }).to_string();
                                            let _ = tx_task.send(complete_msg).await;
                                        }
                                        claude_cancels_clone
                                            .lock()
                                            .await
                                            .remove(&routing_for_task);
                                    });
                                }
                            }
                            "cancel" => {
                                let target = match request.session_id.clone() {
                                    Some(id) if !id.is_empty() => id,
                                    _ => {
                                        tracing::warn!("cancel received without sessionId; ignoring");
                                        continue;
                                    }
                                };

                                // Codex cancel: per-session lookup → turn/interrupt.
                                let codex_snapshot = {
                                    let states = codex_states.lock().await;
                                    states.get(&target).cloned()
                                };
                                if let Some(state) = codex_snapshot {
                                    if let (Some(tid), Some(turn), Some(creds)) =
                                        (state.thread_id, state.turn_id, state.credentials)
                                    {
                                        let server = CodexAppServer::global();
                                        if let Err(e) =
                                            server.interrupt_turn(&creds, &tid, &turn).await
                                        {
                                            tracing::warn!("turn/interrupt failed: {}", e);
                                        }
                                    }
                                }

                                // Claude cancel: signal only the matching session.
                                let cancel_sender = {
                                    let map = claude_cancels.lock().await;
                                    map.get(&target).cloned()
                                };
                                if let Some(sender) = cancel_sender {
                                    let _ = sender.send(true);
                                }
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
    custom_cli_path: &str,
    api_key_override: &str,
    base_url_override: &str,
    routing_id: String,
    tx: mpsc::Sender<String>,
    cancel_rx: &mut tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let source = cli::normalize_source(source)?;
    if source == "codex" {
        return Err("Codex chat must go through run_codex_chat_via_app_server".to_string());
    }
    let project_dir = resolve_project_dir(source, project_path)?;
    if !custom_cli_path.trim().is_empty() {
        tracing::warn!("Ignoring client-supplied cli_path for web chat");
    }
    let cli_path = cli::find_cli(source)?;
    let credentials =
        cli_config::resolve_credentials(source, Some(api_key_override), Some(base_url_override))?;
    let mut cmd = Command::new(&cli_path);
    let resume_target = resume_session_id.map(|session_id| {
        if source == "omp" {
            session_core::provider::omp::find_session_file(project_path, session_id)
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_else(|| session_id.to_string())
        } else {
            session_id.to_string()
        }
    });
    if let Some(resume_target) = resume_target.as_deref() {
        cmd.arg("--resume").arg(resume_target);
    }
    cmd.arg("-p").arg(prompt);
    if !model.is_empty() {
        if source == "claude" {
            let cli_model = model.strip_suffix("-latest").unwrap_or(model);
            cmd.arg("--model").arg(cli_model);
        } else {
            cmd.arg("--model").arg(model);
        }
    }
    if source == "omp" {
        cmd.arg("--mode").arg("json");
        cmd.arg("--no-pty");
        if skip_permissions {
            cmd.arg("--auto-approve");
        }
    } else {
        cmd.arg("--output-format").arg("stream-json");
        cmd.arg("--include-partial-messages");
        cmd.arg("--verbose");
        if skip_permissions {
            cmd.arg("--dangerously-skip-permissions");
        }
    }

    // Web mode: no interactive terminal, close stdin so CLI doesn't hang
    // waiting for permission confirmation or other interactive prompts
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    compose_chat_path(&mut cmd, &cli_path)?;
    apply_provider_env(&mut cmd, source, &credentials);
    cmd.current_dir(project_dir);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn CLI: {}", e))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let tx_stdout = tx.clone();
    let routing_stdout = routing_id.clone();
    let stdout_task = tokio::spawn(async move {
        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let msg = serde_json::json!({
                    "type": "output",
                    "data": line,
                    "sessionId": &routing_stdout,
                })
                .to_string();
                if tx_stdout.send(msg).await.is_err() {
                    break;
                }
            }
        }
    });

    let tx_stderr = tx.clone();
    let routing_stderr = routing_id.clone();
    let stderr_task = tokio::spawn(async move {
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let msg = serde_json::json!({
                    "type": "error",
                    "data": line,
                    "sessionId": &routing_stderr,
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
                "success": success,
                "sessionId": &routing_id,
            }).to_string();
            let _ = tx.send(complete_msg).await;
        }
        _ = cancel_rx.changed() => {
            let _ = child.kill().await;
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            let cancel_msg = serde_json::json!({
                "type": "complete",
                "success": false,
                "sessionId": &routing_id,
            }).to_string();
            let _ = tx.send(cancel_msg).await;
        }
    }

    Ok(())
}

/// Run a Codex chat through the app-server. Sends events to `tx` in the same
/// envelope shape (`{"type":"output","data":"<json>","sessionId":<routing>}`)
/// used elsewhere on the socket. `routing_id` is the per-pane stream key the
/// client uses to demux concurrent turns.
#[allow(clippy::too_many_arguments)]
async fn run_codex_chat_via_app_server(
    routing_id: String,
    project_path: &str,
    prompt: &str,
    model: &str,
    resume_session_id: Option<String>,
    api_key_override: &str,
    base_url_override: &str,
    tx: mpsc::Sender<String>,
    states: CodexStateMap,
) -> Result<(), String> {
    let project_dir = resolve_project_dir("codex", project_path)?;
    let project_dir_str = project_dir.to_string_lossy().to_string();
    let credentials =
        cli_config::resolve_credentials("codex", Some(api_key_override), Some(base_url_override))?;

    let server = CodexAppServer::global();
    let model_opt = if model.is_empty() { None } else { Some(model) };

    // Open the thread.
    let (thread_id, history_turns) = if let Some(sid) = resume_session_id.as_deref() {
        let resp = server
            .resume_thread(&credentials, sid, &project_dir_str, model_opt)
            .await
            .map_err(|e| format!("codex thread/resume failed: {}", e))?;
        let id = resp
            .get("thread")
            .and_then(|t| t.get("id"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| sid.to_string());
        let turns = resp
            .get("thread")
            .and_then(|t| t.get("turns"))
            .cloned()
            .unwrap_or(Value::Null);
        (id, turns)
    } else {
        let resp = server
            .start_thread(&credentials, &project_dir_str, model_opt)
            .await
            .map_err(|e| format!("codex thread/start failed: {}", e))?;
        let id = resp
            .get("thread")
            .and_then(|t| t.get("id"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| "codex thread/start: missing thread.id".to_string())?;
        (id, Value::Null)
    };

    // Register per-session state for cancel routing.
    {
        let mut map = states.lock().await;
        let entry = map.entry(routing_id.clone()).or_default();
        entry.thread_id = Some(thread_id.clone());
        entry.credentials = Some(credentials.clone());
    }

    // Emit session_id. `data` is the real thread_id (used by the client for
    // resume / URL); `sessionId` is the stable routing key the client used
    // to subscribe.
    let sid_msg = serde_json::json!({
        "type": "session_id",
        "data": &thread_id,
        "sessionId": &routing_id,
    })
    .to_string();
    let _ = tx.send(sid_msg).await;

    // Emit history (resume only).
    if !history_turns.is_null() {
        let hist_msg = serde_json::json!({
            "type": "history",
            "data": history_turns,
            "sessionId": &routing_id,
        })
        .to_string();
        let _ = tx.send(hist_msg).await;
    }

    // Subscribe before kicking off the turn.
    let mut rx = server
        .subscribe(&credentials, &thread_id)
        .await
        .map_err(|e| format!("codex subscribe failed: {}", e))?;

    // Signal used to wake the streaming loop below if turn/start fails
    // before any notification arrives — otherwise it would block on
    // rx.recv() forever. Using `Notify` rather than a oneshot because the
    // oneshot Receiver also fires (with `Err(RecvError)`) when the Sender
    // drops without sending, which would prematurely abort every successful
    // turn. `Notify::notify_one` only fires on the explicit call.
    let abort = Arc::new(tokio::sync::Notify::new());

    // Fire turn/start in the background; events stream through `rx`.
    {
        let server = server.clone();
        let creds = credentials.clone();
        let tid = thread_id.clone();
        let prompt_owned = prompt.to_string();
        let model_owned = model.to_string();
        let cwd = project_dir_str.clone();
        let tx_err = tx.clone();
        let routing_for_err = routing_id.clone();
        let abort_for_err = abort.clone();
        tokio::spawn(async move {
            let m = if model_owned.is_empty() {
                None
            } else {
                Some(model_owned.as_str())
            };
            if let Err(e) = server
                .start_turn(&creds, &tid, &prompt_owned, m, Some(cwd.as_str()))
                .await
            {
                let err = serde_json::json!({
                    "type": "error",
                    "data": format!("turn/start failed: {}", e),
                    "sessionId": &routing_for_err,
                })
                .to_string();
                let _ = tx_err.send(err).await;
                abort_for_err.notify_one();
            }
            // Success path: drop the Arc; no notification fires and the
            // streaming loop continues until turn/completed.
        });
    }

    // Stream notifications until turn/completed | turn/failed | error, or
    // until turn/start failure short-circuits the loop. The error frame for
    // a turn/start failure is emitted by the spawned task above; here we
    // just drive the loop to a clean exit so the post-loop `complete`
    // frame still fires.
    let mut success = true;
    loop {
        let notif = tokio::select! {
            maybe = rx.recv() => match maybe {
                Some(n) => n,
                None => break,
            },
            _ = abort.notified() => {
                success = false;
                break;
            }
        };

        if notif.method == "turn/started" {
            if let Some(tid) = notif
                .params
                .get("turn")
                .and_then(|t| t.get("id"))
                .and_then(|v| v.as_str())
            {
                let mut map = states.lock().await;
                if let Some(entry) = map.get_mut(&routing_id) {
                    entry.turn_id = Some(tid.to_string());
                }
            }
        }

        let envelope = serde_json::json!({
            "type": "output",
            "data": serde_json::json!({
                "type": "notification",
                "method": &notif.method,
                "params": &notif.params,
            }).to_string(),
            "sessionId": &routing_id,
        })
        .to_string();
        if tx.send(envelope).await.is_err() {
            break;
        }

        let terminal = notif.method == "turn/completed"
            || notif.method == "turn/failed"
            || notif.method == "error";
        if notif.method == "turn/failed" || notif.method == "error" {
            success = false;
        }
        if terminal {
            break;
        }
    }

    // Clear turn id so a stale one can't fire on the next cancel.
    {
        let mut map = states.lock().await;
        if let Some(entry) = map.get_mut(&routing_id) {
            entry.turn_id = None;
        }
    }

    let complete = serde_json::json!({
        "type": "complete",
        "success": success,
        "sessionId": &routing_id,
    })
    .to_string();
    let _ = tx.send(complete).await;

    Ok(())
}

/// Compose PATH for the spawned CLI: prepend the CLI's own directory (so it
/// can find sibling scripts) AND node's directory. The latter matters under
/// minimal PATH environments (systemd, daemons) where `#!/usr/bin/env node`
/// in the CLI entry script would otherwise fail with `node: No such file`.
fn compose_chat_path(cmd: &mut Command, cli_path: &str) -> Result<(), String> {
    let mut paths: Vec<PathBuf> = Vec::new();

    if let Some(cli_dir) = Path::new(cli_path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        paths.push(cli_dir.to_path_buf());
    }

    if let Some(node_path) = cli::find_node() {
        if let Some(node_dir) = Path::new(&node_path)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
        {
            let node_dir_buf = node_dir.to_path_buf();
            if !paths.iter().any(|p| p == &node_dir_buf) {
                paths.push(node_dir_buf);
            }
        }
    }

    if let Some(existing_path) = std::env::var_os("PATH") {
        for p in std::env::split_paths(&existing_path) {
            if !paths.iter().any(|existing| existing == &p) {
                paths.push(p);
            }
        }
    }

    if paths.is_empty() {
        return Ok(());
    }

    let joined = std::env::join_paths(paths)
        .map_err(|e| format!("Failed to compose PATH for chat CLI: {}", e))?;
    cmd.env("PATH", joined);
    Ok(())
}

fn apply_provider_env(cmd: &mut Command, source: &str, credentials: &ResolvedCliCredentials) {
    if source == "omp" {
        return;
    }
    for key in &[
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_BASE_URL",
        "CODEX_API_KEY",
        "OPENAI_API_KEY",
        "CODEX_BASE_URL",
        "OPENAI_BASE_URL",
    ] {
        cmd.env_remove(key);
    }

    if source == "codex" {
        if !credentials.api_key.is_empty() {
            cmd.env("CODEX_API_KEY", &credentials.api_key);
            cmd.env("OPENAI_API_KEY", &credentials.api_key);
        }
        if !credentials.base_url.is_empty() {
            cmd.env("CODEX_BASE_URL", &credentials.base_url);
            cmd.env("OPENAI_BASE_URL", &credentials.base_url);
        }
    } else if source == "claude" {
        if !credentials.api_key.is_empty() {
            cmd.env("ANTHROPIC_API_KEY", &credentials.api_key);
            cmd.env("ANTHROPIC_AUTH_TOKEN", &credentials.api_key);
        }
        if !credentials.base_url.is_empty() {
            cmd.env("ANTHROPIC_BASE_URL", &credentials.base_url);
        }
    }
}
