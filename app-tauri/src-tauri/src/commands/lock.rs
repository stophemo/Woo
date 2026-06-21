use crate::commands::CommandResult;
use crate::services::lock_service;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LockStatus {
    pub has_password: bool,
    pub mode: String,
}

#[tauri::command(rename_all = "camelCase", rename = "lockStatus")]
pub fn lock_status() -> CommandResult<LockStatus> {
    CommandResult::success(LockStatus {
        has_password: lock_service::has_password(),
        mode: lock_service::get_password_mode(),
    })
}

#[tauri::command(rename_all = "camelCase", rename = "lockSetPassword")]
pub fn lock_set_password(password: String) -> CommandResult<()> {
    match lock_service::set_password(&password) {
        Ok(_) => CommandResult::success(()),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "lockVerifyPassword")]
pub fn lock_verify_password(password: String) -> CommandResult<bool> {
    match lock_service::verify_password(&password) {
        Ok(result) => CommandResult::success(result),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "lockLockFolder")]
pub fn lock_lock_folder(folder_id: String) -> CommandResult<()> {
    match lock_service::lock_folder(&folder_id) {
        Ok(_) => CommandResult::success(()),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "lockUnlockFolder")]
pub fn lock_unlock_folder(folder_id: String) -> CommandResult<()> {
    match lock_service::unlock_folder(&folder_id) {
        Ok(_) => CommandResult::success(()),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "lockIsFolderLocked")]
pub fn lock_is_folder_locked(folder_id: String) -> CommandResult<bool> {
    CommandResult::success(lock_service::is_folder_locked(&folder_id))
}

#[tauri::command(rename_all = "camelCase", rename = "lockIsFolderEffectivelyLocked")]
pub fn lock_is_folder_effectively_locked(folder_id: String) -> CommandResult<bool> {
    CommandResult::success(lock_service::is_folder_effectively_locked(&folder_id))
}

#[tauri::command(rename_all = "camelCase", rename = "lockLockDocument")]
pub fn lock_lock_document(document_id: String) -> CommandResult<()> {
    match lock_service::lock_document(&document_id) {
        Ok(_) => CommandResult::success(()),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "lockUnlockDocument")]
pub fn lock_unlock_document(document_id: String) -> CommandResult<()> {
    match lock_service::unlock_document(&document_id) {
        Ok(_) => CommandResult::success(()),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "lockIsDocumentLocked")]
pub fn lock_is_document_locked(document_id: String) -> CommandResult<bool> {
    CommandResult::success(lock_service::is_document_locked(&document_id))
}

#[tauri::command(rename_all = "camelCase", rename = "lockCloudPushSettings")]
pub fn lock_cloud_push_settings(password: String) -> CommandResult<()> {
    lock_service::cloud_push_settings(&password);
    CommandResult::success(())
}

#[tauri::command(rename_all = "camelCase", rename = "lockCloudPullSettings")]
pub fn lock_cloud_pull_settings() -> CommandResult<()> {
    lock_service::cloud_pull_settings();
    CommandResult::success(())
}
