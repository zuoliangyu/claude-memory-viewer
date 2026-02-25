use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;

use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use session_core::cli;
use session_core::cli_config::{self, CliConfig};
use session_core::model_list::{self, ModelInfo};
use session_core::quick_chat::{self, ChatMsg};

/// State to track active chat processes.
pub struct ChatProcessState {
    pub processes: Arc<Mutex<HashMap<String, u32>>>, // session_id -> PID
}

impl ChatProcessState {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[tauri::command]
pub fn detect_cli() -> Result<Vec<cli::CliInstallation>, String> {
    Ok(cli::discover_installations())
}

#[tauri::command]
pub fn get_cli_config(source: String) -> Result<CliConfig, String> {
    cli_config::read_cli_config(&source)
}

#[tauri::command]
pub async fn quick_chat(
    app: AppHandle,
    source: String,
    messages: Vec<ChatMsg>,
    model: String,
) -> Result<(), String> {
    let app_handle = app.clone();

    tokio::spawn(async move {
        let result = quick_chat::stream_chat(&source, messages, &model, |chunk| {
            let _ = app_handle.emit("quick-chat-chunk", chunk);
        })
        .await;

        match result {
            Ok(()) => {
                let _ = app_handle.emit(
                    "quick-chat-done",
                    serde_json::json!({ "success": true }).to_string(),
                );
            }
            Err(e) => {
                let _ = app_handle.emit("quick-chat-error", &e);
                let _ = app_handle.emit(
                    "quick-chat-done",
                    serde_json::json!({ "success": false }).to_string(),
                );
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn list_models(
    source: String,
    api_key: String,
    base_url: String,
) -> Result<Vec<ModelInfo>, String> {
    model_list::list_models(&source, &api_key, &base_url).await
}

#[tauri::command]
pub async fn start_chat(
    app: AppHandle,
    source: String,
    project_path: String,
    prompt: String,
    model: String,
    skip_permissions: bool,
) -> Result<String, String> {
    let session_id = uuid::Uuid::new_v4().to_string();

    let cli_path = cli::find_cli(&source)?;

    let cmd = build_chat_command(
        &cli_path,
        &source,
        &project_path,
        &prompt,
        &model,
        skip_permissions,
        None,
    )?;

    spawn_and_stream(app, cmd, session_id.clone(), source)?;

    Ok(session_id)
}

#[tauri::command]
pub async fn continue_chat(
    app: AppHandle,
    source: String,
    session_id: String,
    project_path: String,
    prompt: String,
    model: String,
    skip_permissions: bool,
) -> Result<String, String> {
    let cli_path = cli::find_cli(&source)?;

    let cmd = build_chat_command(
        &cli_path,
        &source,
        &project_path,
        &prompt,
        &model,
        skip_permissions,
        Some(&session_id),
    )?;

    spawn_and_stream(app, cmd, session_id.clone(), source)?;

    Ok(session_id)
}

#[tauri::command]
pub async fn cancel_chat(app: AppHandle, session_id: String) -> Result<(), String> {
    let state = app.state::<ChatProcessState>();
    let pid = {
        let mut processes = state.processes.lock();
        processes.remove(&session_id)
    };

    if let Some(pid) = pid {
        kill_process(pid);
        let _ = app.emit(&format!("chat-complete:{}", session_id), "cancelled");
    }

    Ok(())
}

fn build_chat_command(
    cli_path: &str,
    source: &str,
    project_path: &str,
    prompt: &str,
    model: &str,
    skip_permissions: bool,
    resume_session_id: Option<&str>,
) -> Result<Command, String> {
    let mut cmd = Command::new(cli_path);

    match source {
        "claude" => {
            if let Some(sid) = resume_session_id {
                cmd.arg("--resume").arg(sid);
            }
            cmd.arg("-p").arg(prompt);
            if !model.is_empty() {
                // Strip "-latest" suffix — Claude CLI expects full names like
                // "claude-sonnet-4-6", not API-style "claude-sonnet-4-6-latest"
                let cli_model = model.strip_suffix("-latest").unwrap_or(model);
                cmd.arg("--model").arg(cli_model);
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

    eprintln!("[chat] source={}, model={}, project={}", source, model, project_path);

    // Clean environment: use a whitelist approach (like opcode) to avoid
    // inheriting Claude Code session vars that cause conflicts.
    // Clear everything, then only pass essential system variables.
    cmd.env_clear();
    for key in &[
        "PATH",
        "PATHEXT",
        "SYSTEMROOT",
        "SYSTEMDRIVE",
        "COMSPEC",
        "TEMP",
        "TMP",
        "HOME",
        "HOMEDRIVE",
        "HOMEPATH",
        "USERPROFILE",
        "USERNAME",
        "USER",
        "SHELL",
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
        "NODE_PATH",
        "NVM_DIR",
        "NVM_BIN",
        "NVM_SYMLINK",
        "APPDATA",
        "LOCALAPPDATA",
        "PROGRAMFILES",
        "PROGRAMDATA",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "NO_PROXY",
        "ALL_PROXY",
    ] {
        if let Ok(val) = std::env::var(key) {
            cmd.env(key, val);
        }
    }

    cmd.current_dir(project_path);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Don't create a console window on Windows
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    Ok(cmd)
}

fn spawn_and_stream(
    app: AppHandle,
    mut cmd: Command,
    session_id: String,
    source: String,
) -> Result<(), String> {
    let child = cmd.spawn().map_err(|e| format!("Failed to spawn CLI process: {}", e))?;
    let pid = child.id().unwrap_or(0);

    // Register the process
    let state = app.state::<ChatProcessState>();
    state.processes.lock().insert(session_id.clone(), pid);

    let app_handle = app.clone();
    let sid = session_id.clone();

    tokio::spawn(async move {
        stream_process_output(app_handle, child, sid, source).await;
    });

    Ok(())
}

async fn stream_process_output(
    app: AppHandle,
    mut child: Child,
    session_id: String,
    _source: String,
) {
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let app_stdout = app.clone();
    let sid_stdout = session_id.clone();

    // Stream stdout
    let stdout_task = tokio::spawn(async move {
        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let event_name = format!("chat-output:{}", sid_stdout);
                let _ = app_stdout.emit(&event_name, &line);
            }
        }
    });

    let app_stderr = app.clone();
    let sid_stderr = session_id.clone();

    // Stream stderr
    let stderr_task = tokio::spawn(async move {
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                eprintln!("[chat stderr] {}", line);
                let event_name = format!("chat-error:{}", sid_stderr);
                let _ = app_stderr.emit(&event_name, &line);
            }
        }
    });

    // Wait for process to complete
    let exit_status = child.wait().await;
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    let success = exit_status.map(|s| s.success()).unwrap_or(false);

    // Clean up from process registry
    let state = app.state::<ChatProcessState>();
    state.processes.lock().remove(&session_id);

    let event_name = format!("chat-complete:{}", session_id);
    let _ = app.emit(
        &event_name,
        serde_json::json!({ "success": success }).to_string(),
    );
}

fn kill_process(pid: u32) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output();
    }

    #[cfg(not(windows))]
    {
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
        // Give it a moment, then force kill
        std::thread::sleep(std::time::Duration::from_millis(500));
        unsafe {
            libc::kill(pid as i32, libc::SIGKILL);
        }
    }
}
