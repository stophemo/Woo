use crate::commands::CommandResult;
use crate::services::kb_service;

#[tauri::command(rename_all = "camelCase", rename = "kbRebuild")]
pub fn kb_rebuild() -> CommandResult<kb_service::KbRebuildResult> {
    match kb_service::rebuild() {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "kbSearch")]
pub fn kb_search(query: String, limit: Option<i32>) -> CommandResult<Vec<kb_service::KbChunkDTO>> {
    match kb_service::search(&query, limit) {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "kbStatus")]
pub fn kb_status() -> CommandResult<kb_service::KbStatus> {
    match kb_service::status() {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}
