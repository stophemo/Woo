//! 原生 Markdown 文件库命令。
//!
//! 0.9.0 只提供本地索引/读写/迁移，不改变现有 WebDAV 同步。

use std::path::Path;

use crate::commands::CommandResult;
use crate::db;
use crate::services::markdown_vault;

#[tauri::command(rename_all = "camelCase", rename = "markdownVaultScan")]
pub async fn markdown_vault_scan(
    root: String,
) -> Result<CommandResult<markdown_vault::ScanSummary>, String> {
    let result = db::with_db(|conn| markdown_vault::scan_vault(conn, Path::new(&root)));
    Ok(match result {
        Ok(summary) => CommandResult::success(summary),
        Err(error) => CommandResult::error(&error),
    })
}

#[tauri::command(rename_all = "camelCase", rename = "markdownVaultList")]
pub async fn markdown_vault_list() -> Result<CommandResult<Vec<markdown_vault::NoteSummary>>, String>
{
    let result = db::with_db(markdown_vault::list_notes);
    Ok(match result {
        Ok(notes) => CommandResult::success(notes),
        Err(error) => CommandResult::error(&error),
    })
}

#[tauri::command(rename_all = "camelCase", rename = "markdownVaultRead")]
pub async fn markdown_vault_read(
    root: String,
    note_path: String,
) -> Result<CommandResult<String>, String> {
    Ok(
        match markdown_vault::read_note(Path::new(&root), &note_path) {
            Ok(content) => CommandResult::success(content),
            Err(error) => CommandResult::error(&error),
        },
    )
}

#[tauri::command(rename_all = "camelCase", rename = "markdownVaultSave")]
pub async fn markdown_vault_save(
    root: String,
    note_path: String,
    content: String,
) -> Result<CommandResult<markdown_vault::SavedNote>, String> {
    let result =
        db::with_db(|conn| markdown_vault::save_note(conn, Path::new(&root), &note_path, &content));
    Ok(match result {
        Ok(saved) => CommandResult::success(saved),
        Err(error) => CommandResult::error(&error),
    })
}

#[cfg(desktop)]
#[tauri::command(rename_all = "camelCase", rename = "markdownVaultMigratePreview")]
pub async fn markdown_vault_migrate_preview(
    target: String,
) -> Result<CommandResult<markdown_vault::migrate::MigratePreview>, String> {
    let result = db::with_db(|conn| markdown_vault::migrate::preview(conn, Path::new(&target)));
    Ok(match result {
        Ok(preview) => CommandResult::success(preview),
        Err(error) => CommandResult::error(&error),
    })
}

#[cfg(desktop)]
#[tauri::command(rename_all = "camelCase", rename = "markdownVaultMigrate")]
pub async fn markdown_vault_migrate(
    target: String,
    overwrite: Option<bool>,
) -> Result<CommandResult<markdown_vault::migrate::MigrateReport>, String> {
    let result = db::with_db(|conn| {
        markdown_vault::migrate::execute(conn, Path::new(&target), overwrite.unwrap_or(false))
    });
    Ok(match result {
        Ok(report) => CommandResult::success(report),
        Err(error) => CommandResult::error(&error),
    })
}
