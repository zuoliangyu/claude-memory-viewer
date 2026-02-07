use std::process::Command;

#[tauri::command]
pub fn resume_session(session_id: String, project_path: String) -> Result<(), String> {
    // Normalize path: replace forward slashes with backslashes on Windows
    let project_path = normalize_path(&project_path);

    // Validate path exists
    if !std::path::Path::new(&project_path).exists() {
        return Err(format!("项目路径不存在: {}", project_path));
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x00000010;

        Command::new("cmd")
            .args(["/k", &format!("claude --resume {}", session_id)])
            .current_dir(&project_path)
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "tell application \"Terminal\" to do script \"cd '{}' && claude --resume {}\"",
            project_path, session_id
        );
        Command::new("osascript")
            .args(["-e", &script])
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        let cmd_str = format!(
            "cd '{}' && claude --resume {}",
            project_path, session_id
        );

        // Try various terminal emulators
        let terminals = [
            ("gnome-terminal", vec!["--", "bash", "-c", &cmd_str]),
            ("konsole", vec!["-e", "bash", "-c", &cmd_str]),
            ("xfce4-terminal", vec!["-e", &format!("bash -c '{}'", cmd_str)]),
            ("xterm", vec!["-e", &format!("bash -c '{}'", cmd_str)]),
        ];

        let mut launched = false;
        for (terminal, args) in &terminals {
            if Command::new(terminal)
                .args(args)
                .spawn()
                .is_ok()
            {
                launched = true;
                break;
            }
        }

        if !launched {
            return Err("No supported terminal emulator found".to_string());
        }
    }

    Ok(())
}

/// Normalize path for the current OS
fn normalize_path(path: &str) -> String {
    if cfg!(windows) {
        path.replace('/', "\\")
    } else {
        path.replace('\\', "/")
    }
}
