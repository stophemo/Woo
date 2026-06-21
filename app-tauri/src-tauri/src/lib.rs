mod commands;
mod db;
mod models;
mod services;
mod supabase;

use tauri::Manager;

fn init_logger() {
    env_logger::Builder::from_env(
        env_logger::Env::default().filter_or("WOO_LOG", "info"),
    )
    .format_timestamp_secs()
    .init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logger();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Initialize database directory
            let db_path = app
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");
            std::fs::create_dir_all(&db_path).ok();
            db::set_data_dir(db_path);

            // Main window is created automatically from tauri.conf.json
            // (dev → devUrl, prod → frontendDist). macOS-specific traffic
            // light position is also configured in tauri.conf.json.

            // Lazy-init database
            db::with_db(|_conn| {
                log::info!("[DB] Database ready");
            });

            Ok(())
        })
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                db::close_db();
            }
        })
        .invoke_handler(tauri::generate_handler![
            // System
            commands::app_get_version,
            commands::update_check,
            commands::greet,
            // Folder
            commands::folder::folder_tree,
            commands::folder::folder_create,
            commands::folder::folder_rename,
            commands::folder::folder_remove,
            commands::folder::folder_reorder,
            // Document
            commands::document::document_list_by_folder,
            commands::document::document_list_all,
            commands::document::document_list_trash,
            commands::document::document_list_orphans,
            commands::document::document_search,
            commands::document::document_get,
            commands::document::document_create,
            commands::document::document_rename,
            commands::document::document_update_content,
            commands::document::document_remove,
            commands::document::document_restore,
            commands::document::document_hard_delete,
            commands::document::document_empty_trash,
            commands::document::document_reorder,
            // Version
            commands::version::version_list,
            commands::version::version_get,
            commands::version::version_save_manual,
            commands::version::version_commit,
            commands::version::version_restore,
            commands::version::version_compact,
            // Auth
            commands::auth::auth_sign_up,
            commands::auth::auth_sign_in,
            commands::auth::auth_sign_out,
            commands::auth::auth_get_user,
            commands::auth::auth_get_session,
            // Lock
            commands::lock::lock_status,
            commands::lock::lock_set_password,
            commands::lock::lock_verify_password,
            commands::lock::lock_lock_folder,
            commands::lock::lock_unlock_folder,
            commands::lock::lock_is_folder_locked,
            commands::lock::lock_is_folder_effectively_locked,
            commands::lock::lock_lock_document,
            commands::lock::lock_unlock_document,
            commands::lock::lock_is_document_locked,
            commands::lock::lock_cloud_push_settings,
            commands::lock::lock_cloud_pull_settings,
            // Knowledge Base
            commands::kb::kb_rebuild,
            commands::kb::kb_search,
            commands::kb::kb_status,
            // Sync
            commands::sync::sync_status,
            commands::sync::sync_trigger,
            // System (dialogs, file IO, export)
            commands::system::dialog_save_image,
            commands::system::file_write,
            commands::system::dialog_save_document,
            commands::system::document_export_pdf,
            commands::system::log_write,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
