mod commands;
mod db;
mod models;
mod services;
mod supabase;

use once_cell::sync::Lazy;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Emitter, Manager};

// Global AppHandle for emitting events to the frontend from background tasks.
static APP_HANDLE: Lazy<Mutex<Option<tauri::AppHandle>>> = Lazy::new(|| Mutex::new(None));

#[derive(Default)]
struct OpenFileState {
    pending: Vec<String>,
    frontend_ready: bool,
}

// 前端就绪状态和待打开路径必须由同一把锁保护，避免 drain 与 dispatch 交错丢失请求。
static OPEN_FILE_STATE: Lazy<Mutex<OpenFileState>> =
    Lazy::new(|| Mutex::new(OpenFileState::default()));

/// Drain and return all pending file open paths (called by the frontend after init).
pub fn drain_pending_open_files() -> Vec<String> {
    let paths = match OPEN_FILE_STATE.lock() {
        Ok(mut state) => {
            state.frontend_ready = true;
            std::mem::take(&mut state.pending)
        }
        Err(error) => {
            log::error!("[OpenFile] 无法读取待打开路径: {error}");
            Vec::new()
        }
    };
    if !paths.is_empty() {
        log::info!("[OpenFile] 前端已消费 {} 个缓冲路径", paths.len());
    }
    paths
}

fn is_supported_external_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "md" | "markdown" | "mdown" | "mkd" | "txt" | "text"
            )
        })
}

fn collect_open_file_paths<I, S>(args: I, cwd: &Path) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    args.into_iter()
        .filter_map(|arg| {
            let path = PathBuf::from(arg.into());
            let path = if path.is_absolute() { path } else { cwd.join(path) };
            (path.is_file() && is_supported_external_file(&path))
                .then(|| path.to_string_lossy().into_owned())
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn collect_open_file_urls<I>(urls: I) -> Vec<String>
where
    I: IntoIterator<Item = tauri::Url>,
{
    urls.into_iter()
        .filter_map(|url| url.to_file_path().ok())
        .filter(|path| path.is_file() && is_supported_external_file(path))
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

fn append_pending_open_files(pending: &mut Vec<String>, paths: Vec<String>) -> usize {
    let original_len = pending.len();
    for path in paths {
        if !pending.contains(&path) {
            pending.push(path);
        }
    }
    pending.len() - original_len
}

fn dispatch_open_files(paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }

    match OPEN_FILE_STATE.lock() {
        Ok(mut state) if !state.frontend_ready => {
            let added = append_pending_open_files(&mut state.pending, paths);
            if added > 0 {
                log::info!("[OpenFile] 前端就绪前已缓冲 {} 个路径", added);
            }
            return;
        }
        Ok(_) => {}
        Err(error) => {
            log::error!("[OpenFile] 无法访问打开文件状态: {error}");
            return;
        }
    }

    if let Some(app_handle) = get_app_handle() {
        if app_handle.emit("open-file", &paths).is_ok() {
            log::info!(
                "[OpenFile] 已向运行中的前端派发 {} 个路径",
                paths.len()
            );
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
            return;
        }
        log::warn!("[OpenFile] 前端事件派发失败，路径已退回缓冲队列");
    } else {
        log::warn!("[OpenFile] AppHandle 尚未就绪，路径已退回缓冲队列");
    }

    if let Ok(mut state) = OPEN_FILE_STATE.lock() {
        let added = append_pending_open_files(&mut state.pending, paths);
        if added > 0 {
            log::info!("[OpenFile] 已重新缓冲 {} 个路径", added);
        }
    }
}

#[cfg(test)]
mod open_file_tests {
    use super::{append_pending_open_files, collect_open_file_paths, is_supported_external_file};
    use std::path::Path;

    #[test]
    fn recognizes_supported_external_file_extensions_case_insensitively() {
        for path in ["note.md", "note.MARKDOWN", "note.mdown", "note.mkd", "note.txt", "note.TEXT"] {
            assert!(is_supported_external_file(Path::new(path)), "应支持 {path}");
        }
        assert!(!is_supported_external_file(Path::new("note.pdf")));
        assert!(!is_supported_external_file(Path::new("note")));
    }

    #[test]
    fn collects_existing_relative_file_paths() {
        let directory = std::env::temp_dir().join(format!("woo-open-file-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).expect("创建临时目录失败");
        let markdown_path = directory.join("sample.md");
        let ignored_path = directory.join("sample.pdf");
        std::fs::write(&markdown_path, "# sample").expect("写入 Markdown 样例失败");
        std::fs::write(&ignored_path, "sample").expect("写入忽略样例失败");

        let paths = collect_open_file_paths(["sample.md", "sample.pdf", "missing.md"], &directory);
        assert_eq!(paths, vec![markdown_path.to_string_lossy().into_owned()]);

        std::fs::remove_dir_all(&directory).expect("清理临时目录失败");
    }

    #[test]
    fn deduplicates_pending_open_file_paths() {
        let mut pending = vec!["first.md".to_string()];
        let added = append_pending_open_files(
            &mut pending,
            vec!["first.md".to_string(), "second.md".to_string()],
        );

        assert_eq!(added, 1);
        assert_eq!(pending, vec!["first.md", "second.md"]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn collects_supported_file_urls() {
        use super::collect_open_file_urls;

        let directory = std::env::temp_dir().join(format!("woo-open-url-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).expect("创建临时目录失败");
        let markdown_path = directory.join("带空格 sample.md");
        let ignored_path = directory.join("sample.pdf");
        std::fs::write(&markdown_path, "# sample").expect("写入 Markdown 样例失败");
        std::fs::write(&ignored_path, "sample").expect("写入忽略样例失败");

        let urls = [
            tauri::Url::from_file_path(&markdown_path).expect("生成 Markdown file URL 失败"),
            tauri::Url::from_file_path(&ignored_path).expect("生成 PDF file URL 失败"),
            tauri::Url::parse("https://example.com/note.md").expect("生成网络 URL 失败"),
        ];
        let paths = collect_open_file_urls(urls);

        assert_eq!(paths, vec![markdown_path.to_string_lossy().into_owned()]);
        std::fs::remove_dir_all(&directory).expect("清理临时目录失败");
    }
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
    let base = app.path().app_data_dir().expect("failed to get app data dir");
    #[cfg(mobile)]
    {
        // Android/iOS：app_data_dir() 已是应用私有可写目录（如 /data/data/<pkg>/files），
        // 不能像桌面那样 pop 上跳一级，否则会落到沙盒外导致 SQLite 初始化失败。
        base
    }
    #[cfg(not(mobile))]
    {
        // 桌面：app_data_dir() 形如 .../Application Support/<bundle-id>，
        // 上跳一级后拼自定义目录名，与历史数据目录保持一致。
        let mut dir = base;
        dir.pop();
        let dir_name = if cfg!(debug_assertions) { "woo-dev" } else { "Woo" };
        dir.push(dir_name);
        dir
    }
}

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logger();

    let builder = tauri::Builder::default();

    #[cfg(desktop)]
    let builder = builder
        // 必须最先注册，确保文件关联触发的第二实例能转发启动参数。
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            let paths = collect_open_file_paths(args.into_iter().skip(1), Path::new(&cwd));
            dispatch_open_files(paths);
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_process::init());

    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            // 在平台回调可能触发打开请求前保存 AppHandle。
            *APP_HANDLE.lock().unwrap() = Some(app.handle().clone());

            #[cfg(target_os = "macos")]
            {
                set_macos_dock_icon();
            }

            let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let initial_paths = collect_open_file_paths(std::env::args_os().skip(1), &current_dir);
            dispatch_open_files(initial_paths);

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
                // Tokio 的首次 tick 会立即返回；先消费它，避免前端监听器注册前完成同步并丢失刷新事件。
                interval.tick().await;
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
        .run(|app_handle, event| {
            #[cfg(target_os = "macos")]
            match event {
                tauri::RunEvent::Opened { urls } => {
                    let paths = collect_open_file_urls(urls);
                    if !paths.is_empty() {
                        log::info!("[OpenFile] 收到 macOS Opened 事件: {:?}", paths);
                        dispatch_open_files(paths);
                    }
                }
                tauri::RunEvent::Reopen { .. } => {
                    // macOS: 点击 Dock 图标时重新显示窗口
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                _ => {}
            }
        });
}
