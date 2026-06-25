pub mod auth;
pub mod document;
pub mod folder;
pub mod lock;
pub mod sync;
pub mod system;
pub mod version;

use serde::Serialize;
use tauri::AppHandle;

/// Standard command result matching Electron's { ok, data/message } pattern
#[derive(Serialize)]
pub struct CommandResult<T: Serialize> {
    pub ok: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

impl<T: Serialize> CommandResult<T> {
    pub fn success(data: T) -> Self {
        Self { ok: true, data: Some(data), message: None }
    }

    pub fn error(msg: &str) -> Self {
        Self { ok: false, data: None, message: Some(msg.to_string()) }
    }
}

/// Return the app version
#[tauri::command(rename_all = "camelCase", rename = "appGetVersion")]
pub fn app_get_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Check for app updates (placeholder)
#[tauri::command(rename_all = "camelCase", rename = "updateCheck")]
pub async fn update_check() -> UpdateResult {
    UpdateResult {
        has_update: false,
        version: None,
        download_url: None,
        error: None,
    }
}

#[derive(Serialize)]
pub struct UpdateResult {
    pub has_update: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Placeholder greet command
#[tauri::command(rename_all = "camelCase")]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Woo Tauri v2 is running.", name)
}
