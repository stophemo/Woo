use crate::commands::CommandResult;
use crate::services::document_service;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ReorderDocItem {
    pub id: String,
    pub sort_order: i32,
}

#[tauri::command(rename_all = "camelCase", rename = "documentListByFolder")]
pub fn document_list_by_folder(folder_id: String) -> CommandResult<Vec<document_service::DocumentDTO>> {
    match document_service::list_by_folder(&folder_id) {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "documentListAll")]
pub fn document_list_all() -> CommandResult<Vec<document_service::DocumentDTO>> {
    match document_service::list_all() {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "documentListTrash")]
pub fn document_list_trash() -> CommandResult<Vec<document_service::DocumentDTO>> {
    match document_service::list_trash() {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "documentListOrphans")]
pub fn document_list_orphans() -> CommandResult<Vec<document_service::DocumentDTO>> {
    match document_service::list_orphans() {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "documentSearch")]
pub fn document_search(keyword: String) -> CommandResult<Vec<document_service::DocumentDTO>> {
    match document_service::search(&keyword) {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "documentGet")]
pub fn document_get(id: String) -> CommandResult<document_service::DocumentDTO> {
    match document_service::get_by_id(&id) {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "documentCreate")]
pub fn document_create(title: String, folder_id: String) -> CommandResult<document_service::DocumentDTO> {
    match document_service::create(&folder_id, &title, None) {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "documentRename")]
pub fn document_rename(id: String, title: String) -> CommandResult<()> {
    match document_service::rename(&id, &title) {
        Ok(_) => CommandResult::success(()),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "documentUpdateContent")]
pub fn document_update_content(id: String, content: String) -> CommandResult<()> {
    match document_service::update_content(&id, &content) {
        Ok(_) => CommandResult::success(()),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "documentRemove")]
pub fn document_remove(id: String) -> CommandResult<()> {
    match document_service::remove(&id) {
        Ok(_) => CommandResult::success(()),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "documentRestore")]
pub fn document_restore(id: String) -> CommandResult<()> {
    match document_service::restore(&id) {
        Ok(_) => CommandResult::success(()),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "documentHardDelete")]
pub fn document_hard_delete(id: String) -> CommandResult<()> {
    match document_service::hard_delete(&id) {
        Ok(_) => CommandResult::success(()),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "documentEmptyTrash")]
pub fn document_empty_trash() -> CommandResult<()> {
    match document_service::empty_trash() {
        Ok(_) => CommandResult::success(()),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "documentReorder")]
pub fn document_reorder(folder_id: String, items: Vec<ReorderDocItem>) -> CommandResult<()> {
    let items: Vec<(String, i32)> = items.into_iter().map(|i| (i.id, i.sort_order)).collect();
    match document_service::reorder_documents(&folder_id, &items) {
        Ok(_) => CommandResult::success(()),
        Err(msg) => CommandResult::error(&msg),
    }
}
