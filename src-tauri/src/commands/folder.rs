use crate::commands::CommandResult;
use crate::services::folder_service;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ReorderItem {
    pub id: String,
    pub sort_order: i32,
}

#[tauri::command(rename_all = "camelCase", rename = "folderTree")]
pub fn folder_tree() -> CommandResult<Vec<folder_service::FolderTreeNode>> {
    let data = folder_service::get_folder_tree();
    CommandResult::success(data)
}

#[tauri::command(rename_all = "camelCase", rename = "folderCreate")]
pub fn folder_create(name: String, parent_id: Option<String>) -> CommandResult<String> {
    match folder_service::create_folder(&name, parent_id.as_deref()) {
        Ok(id) => CommandResult::success(id),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "folderRename")]
pub fn folder_rename(folder_id: String, name: String) -> CommandResult<()> {
    match folder_service::rename_folder(&folder_id, &name) {
        Ok(_) => CommandResult::success(()),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "folderRemove")]
pub fn folder_remove(folder_id: String) -> CommandResult<()> {
    match folder_service::delete_folder(&folder_id) {
        Ok(_) => CommandResult::success(()),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "folderMove")]
pub fn folder_move(
    folder_id: String,
    new_parent_id: Option<String>,
    position: Option<i32>,
) -> CommandResult<()> {
    log::info!(
        "[folder-move] folder={} new_parent={:?} position={:?}",
        folder_id,
        new_parent_id,
        position
    );
    match folder_service::move_folder(&folder_id, new_parent_id.as_deref(), position) {
        Ok(_) => CommandResult::success(()),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "folderReorder")]
pub fn folder_reorder(parent_id: Option<String>, items: Vec<ReorderItem>) -> CommandResult<()> {
    log::info!("[folder-reorder] parent={:?} items={:?}", parent_id, items);
    let items: Vec<(String, i32)> = items.into_iter().map(|i| (i.id, i.sort_order)).collect();
    match folder_service::reorder_folders(parent_id.as_deref(), &items) {
        Ok(_) => CommandResult::success(()),
        Err(msg) => CommandResult::error(&msg),
    }
}
