mod commands;
mod db;
mod models;
mod services;
mod supabase;

use once_cell::sync::Lazy;
use std::sync::Mutex;
use tauri::{Emitter, Manager};

// Global AppHandle for emitting events to the frontend from background tasks.
static APP_HANDLE: Lazy<Mutex<Option<tauri::AppHandle>>> = Lazy::new(|| Mutex::new(None));

// Buffer for file open requests received before the frontend is ready.
static PENDING_OPEN_FILES: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(Vec::new()));

/// Drain and return all pending file open paths (called by the frontend after init).
pub fn drain_pending_open_files() -> Vec<String> {
    PENDING_OPEN_FILES.lock().ok().map_or(Vec::new(), |mut v| std::mem::take(&mut *v))
}

pub fn get_app_handle() -> Option<tauri::AppHandle> {
    APP_HANDLE.lock().ok().and_then(|h| h.clone())
}

fn init_logger() {
    env_logger::Builder::from_env(
        env_logger::Env::default().filter_or("WOO_LOG", "info"),
    )
    .format_timestamp_secs()
    .init();
}

/// Determine data directory matching app-desktop:
/// - dev  (debug):  ~/Library/Application Support/woo-dev/
/// - prod (release): ~/Library/Application Support/Woo/
fn resolve_data_dir(app: &tauri::App) -> PathBuf {
    let mut base = app.path().app_data_dir().expect("failed to get app data dir");
    // Go up one level to remove bundle identifier, use our own dir name
    base.pop();
    let dir_name = if cfg!(debug_assertions) { "woo-dev" } else { "Woo" };
    base.push(dir_name);
    base
}

use std::path::PathBuf;

#[cfg(target_os = "macos")]
fn set_macos_dock_icon() {
    use objc2::AnyThread;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSString;

    let Some(icon_path) = resolve_macos_icon_path() else {
        log::warn!("[DockIcon] icon.png not found for dock icon");
        return;
    };

    let Some(path_str) = icon_path.to_str() else {
        log::warn!("[DockIcon] icon path is not valid UTF-8");
        return;
    };

    log::info!("[DockIcon] Setting dock icon from {:?}", icon_path);

    unsafe {
        let ns_path = NSString::from_str(path_str);
        let image = NSImage::initWithContentsOfFile(NSImage::alloc(), &ns_path);
        let Some(image) = image else {
            log::warn!("[DockIcon] failed to load NSImage from {:?}", icon_path);
            return;
        };

        let Some(mtm) = objc2::MainThreadMarker::new() else {
            log::warn!("[DockIcon] not running on main thread");
            return;
        };

        let app = NSApplication::sharedApplication(mtm);
        app.setApplicationIconImage(Some(&image));
    }
}

#[cfg(target_os = "macos")]
fn resolve_macos_icon_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;

    // Dev layout: src-tauri/target/debug/woo-tauri -> src-tauri/icons/icon.png
    let dev_path = exe_dir
        .parent()?
        .parent()?
        .join("icons")
        .join("icon.png");
    if dev_path.exists() {
        return Some(dev_path);
    }

    // Bundled layout: Woo.app/Contents/MacOS/woo-tauri -> Woo.app/Contents/Resources/icons/icon.png
    let bundled_path = exe_dir
        .parent()?
        .join("Resources")
        .join("icons")
        .join("icon.png");
    if bundled_path.exists() {
        return Some(bundled_path);
    }

    None
}

/// macOS: 给 tao 的 AppDelegate 类动态添加 application:openFiles: 方法.
/// Tauri/tao 默认只实现了 openURLs，拖拽到 Dock 图标和"打开方式"发给已运行
/// 实例时走的是 kAEOpenDocuments → application:openFiles: 没有被处理。
#[cfg(target_os = "macos")]
fn patch_app_delegate_open_files() {
    use objc2::runtime::{AnyClass, Sel, AnyObject};
    use objc2_foundation::{NSArray, NSString};
    use objc2::msg_send;

    unsafe {
        let Some(cls) = AnyClass::get(c"TaoAppDelegateParent") else {
            log::warn!("[Odoc] TaoAppDelegateParent class not found");
            return;
        };

        // Define the openFiles handler
        unsafe extern "C-unwind" fn application_open_files(
            _this: &AnyObject,
            _sel: Sel,
            _ns_app: &AnyObject,
            files: &NSArray<NSString>,
        ) {
            let count = files.count();
            log::info!("[Odoc] openFiles: called with {} file(s)", count);
            let mut paths = Vec::with_capacity(count as usize);
            for i in 0..count {
                let s = files.objectAtIndex(i);
                paths.push(s.to_string());
            }
            log::info!("[Odoc] paths: {:?}", paths);

            if let (Ok(mut buf), Some(ref app_handle)) =
                (PENDING_OPEN_FILES.lock(), APP_HANDLE.lock().unwrap().as_ref())
            {
                buf.extend(paths.clone());
                let _ = app_handle.emit("open-file", paths);
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        }

        // Cast fn pointer to objc2 runtime's Imp type
        let imp: unsafe extern "C-unwind" fn() = unsafe {
            std::mem::transmute(application_open_files as unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject, &NSArray<NSString>))
        };

        let sel = Sel::register(c"application:openFiles:");
        let types = c"v@:@@";

        let ok = objc2::ffi::class_addMethod(
            cls as *const AnyClass as *mut AnyClass,
            sel,
            imp,
            types.as_ptr(),
        );

        if ok.as_bool() {
            log::info!("[Odoc] Successfully patched AppDelegate with application:openFiles:");
        } else {
            log::warn!("[Odoc] Failed to add application:openFiles: (may already exist)");
        }

        // Drain queued Apple Events
        let Some(mtm) = objc2::MainThreadMarker::new() else { return };
        let ns_app = objc2_app_kit::NSApplication::sharedApplication(mtm);
        let _: () = msg_send![&ns_app, replyToOpenOrPrint: 0i32];
        log::info!("[Odoc] Called replyToOpenOrPrint");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logger();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                set_macos_dock_icon();
                patch_app_delegate_open_files();
            }

            // Store AppHandle for event emission
            *APP_HANDLE.lock().unwrap() = Some(app.handle().clone());

            // Initialize database directory (shared with app-desktop)
            let db_path = resolve_data_dir(app);
            std::fs::create_dir_all(&db_path).ok();
            db::set_data_dir(db_path.clone());
            supabase::set_data_dir(db_path.clone());
            log::info!("[App] Data directory: {:?}", db_path);

            // Restore session from supabase-auth.json
            if let Some(username) = services::auth_service::try_restore_session() {
                log::info!("[App] Restored session for user: {}", username);
                db::set_current_user(Some(&username));
            }

            // Lazy-init database
            db::with_db(|_conn| {
                log::info!("[DB] Database ready");
            });

            // Compact version history on startup (keep latest 50 per document)
            match services::version_service::compact_all_documents(50) {
                Ok(n) => {
                    if n > 0 {
                        log::info!("[App] Compacted {} old versions on startup", n);
                    }
                }
                Err(e) => log::warn!("[App] Version compaction on startup failed: {}", e),
            }

            // Start periodic sync timer with event emission
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
                loop {
                    interval.tick().await;
                    // Only sync if logged in
                    let has_session = supabase::CURRENT_SESSION
                        .lock()
                        .ok()
                        .and_then(|s| s.is_some().then_some(()));
                    if has_session.is_some() {
                        log::debug!("[Sync] Periodic sync tick");
                        let result = services::sync_engine::sync_now_async().await;

                        // Emit sync status event to frontend
                        let status = services::sync_engine::get_status();
                        app_handle.emit("sync-status", &status).ok();

                        // Emit toast for sync feedback
                        let (toast_message, toast_type): (String, &str) = match &result {
                            Ok(sr) => {
                                log::info!(
                                    "[Sync] ⇄ push={} pull={} conflict={} cleanup={} tombstone={}",
                                    sr.pushed_count,
                                    sr.pulled_count,
                                    sr.conflict_count,
                                    sr.cleanup_count,
                                    sr.tombstone_count,
                                );
                                // Emit data-changed event → triggers workspaceStore.syncRefresh()
                                app_handle.emit("sync-data-changed", &sr).ok();
                                (
                                    format!("同步完成 ↑{} ↓{}", sr.pushed_count, sr.pulled_count),
                                    "success",
                                )
                            }
                            Err(e) => {
                                log::warn!("[Sync] Periodic sync failed: {}", e);
                                (format!("同步失败: {}", e), "error")
                            }
                        };
                        app_handle
                            .emit(
                                "sync:toast",
                                serde_json::json!({
                                    "message": toast_message,
                                    "type": toast_type,
                                }),
                            )
                            .ok();
                    }
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                #[cfg(target_os = "macos")]
                {
                    // macOS: Cmd+W 隐藏窗口到后台，而非退出应用
                    api.prevent_close();
                    let _ = window.hide();
                }
                #[cfg(not(target_os = "macos"))]
                {
                    db::close_db();
                }
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
            // Sync
            commands::sync::sync_status,
            commands::sync::sync_trigger,
            // System (dialogs, file IO, export)
            commands::system::dialog_save_image,
            commands::system::file_write,
            commands::system::dialog_save_document,
            commands::system::log_write,
            // External file (open with / drag-drop)
            commands::external_file::read_external_file,
            commands::external_file::write_external_file,
            commands::external_file::pop_pending_open_files,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                // macOS: 点击 Dock 图标时重新显示窗口
                if let Some(window) = _app_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });
}
