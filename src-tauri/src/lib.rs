mod commands;
mod watcher;

use commands::chat::ChatProcessState;
use session_core::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .manage(ChatProcessState::new())
        .invoke_handler(tauri::generate_handler![
            commands::projects::get_projects,
            commands::sessions::get_sessions,
            commands::sessions::delete_session,
            commands::sessions::update_session_meta,
            commands::sessions::get_all_tags,
            commands::sessions::get_cross_project_tags,
            commands::messages::get_messages,
            commands::search::global_search,
            commands::stats::get_stats,
            commands::terminal::resume_session,
            commands::updater::get_install_type,
            commands::chat::detect_cli,
            commands::chat::get_cli_config,
            commands::chat::list_models,
            commands::chat::start_chat,
            commands::chat::continue_chat,
            commands::chat::cancel_chat,
            commands::chat::quick_chat,
        ])
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            let handle = app.handle().clone();
            if let Err(e) = watcher::fs_watcher::start_watcher(handle) {
                eprintln!("Warning: Failed to start file watcher: {}", e);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
