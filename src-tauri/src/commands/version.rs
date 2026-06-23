use crate::commands::CommandResult;
use crate::services::version_service;

#[tauri::command(rename_all = "camelCase", rename = "versionList")]
pub fn version_list(document_id: String) -> CommandResult<Vec<version_service::VersionDTO>> {
    match version_service::list_versions(&document_id) {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "versionGet")]
pub fn version_get(document_id: String, version_no: i32) -> CommandResult<version_service::VersionDTO> {
    match version_service::get_version(&document_id, version_no) {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "versionSaveManual")]
pub fn version_save_manual(document_id: String) -> CommandResult<version_service::VersionDTO> {
    match version_service::save_manual(&document_id) {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "versionCommit")]
pub fn version_commit(document_id: String, change_type: String) -> CommandResult<version_service::VersionDTO> {
    match version_service::commit(&document_id, &change_type) {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "versionRestore")]
pub fn version_restore(document_id: String, version_no: i32) -> CommandResult<()> {
    match version_service::restore(&document_id, version_no) {
        Ok(_) => CommandResult::success(()),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "versionCompact")]
pub fn version_compact(document_id: String, max_versions: Option<i32>) -> CommandResult<i32> {
    let mv = max_versions.unwrap_or(50);
    match version_service::compact_history(&document_id, mv) {
        Ok(count) => CommandResult::success(count),
        Err(msg) => CommandResult::error(&msg),
    }
}
