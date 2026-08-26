mod commands;
mod watcher;

use commands::chat::ChatProcessState;
use session_core::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 给 UI 主线程留一个 CPU 核，避免冷启动并行扫描吃满 CPU 导致界面卡顿。
    session_core::scan_progress::configure_rayon_pool();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .manage(ChatProcessState::new())
        .invoke_handler(tauri::generate_handler![
            commands::projects::get_projects,
            commands::projects::refresh_projects_cache,
            commands::projects::rebuild_projects_cache,
            commands::projects::delete_project,
            commands::projects::set_project_alias,
            commands::sessions::get_sessions,
            commands::sessions::refresh_sessions_cache,
            commands::sessions::get_invalid_sessions,
            commands::sessions::delete_session,
            commands::sessions::update_session_meta,
            commands::sessions::rename_chat_session,
            commands::sessions::get_all_tags,
            commands::sessions::get_cross_project_tags,
            commands::messages::get_messages,
            commands::messages::get_messages_range,
            commands::perf::report_perf_events,
            commands::trajectory::get_trajectory,
            commands::export::export_session,
            commands::export::write_export_file,
            commands::progress::get_scan_progress,
            commands::search::global_search,
            commands::skills::list_skills,
            commands::skills::get_skill_content,
            commands::skills::delete_skill,
            commands::skills::import_skills,
            commands::stats::get_stats,
            commands::stats::get_request_log,
            commands::stats::get_project_costs,
            commands::stats::get_session_cost,
            commands::terminal::resume_session,
            commands::terminal::fork_and_resume,
            commands::updater::get_install_type,
            commands::chat::detect_cli,
            commands::chat::get_cli_config,
            commands::chat::list_models,
            commands::chat::start_chat,
            commands::chat::continue_chat,
            commands::chat::cancel_chat,
            commands::bookmarks::list_bookmarks,
            commands::bookmarks::add_bookmark,
            commands::bookmarks::remove_bookmark,
            commands::recyclebin::list_recycled_items,
            commands::recyclebin::restore_recycled_item,
            commands::recyclebin::permanently_delete_recycled_item,
            commands::recyclebin::empty_recyclebin,
            commands::recyclebin::cleanup_orphan_dirs,
            commands::provider_sync::provider_sync_status,
            commands::provider_sync::provider_sync_run,
            commands::provider_sync::provider_sync_switch,
            commands::provider_sync::provider_sync_clone,
            commands::provider_sync::provider_sync_restore,
            commands::provider_sync::provider_sync_prune,
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
