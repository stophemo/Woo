#![allow(dead_code)]
use crate::commands::CommandResult;
use serde::Deserialize;
use tauri::AppHandle;
use std::fs;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveImagePayload {
    pub default_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileWritePayload {
    pub file_path: String,
    pub data: String,
    pub is_base64: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDocumentPayload {
    pub default_name: Option<String>,
    pub filters: Option<Vec<FileFilter>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPdfPayload {
    pub file_path: String,
    pub html: String,
}

/// Save image dialog
#[tauri::command(rename_all = "camelCase", rename = "dialogSaveImage")]
pub async fn dialog_save_image(
    app: AppHandle,
    payload: SaveImagePayload,
) -> Result<CommandResult<ImageSaveResult>, String> {
    let default_name = payload.default_name.unwrap_or_else(|| "mindmap.png".to_string());
    
    use tauri_plugin_dialog::DialogExt;
    let file = app.dialog()
        .file()
        .add_filter("PNG", &["png"])
        .add_filter("JPEG", &["jpg", "jpeg"])
        .add_filter("SVG", &["svg"])
        .set_file_name(&default_name)
        .blocking_save_file();

    match file {
        Some(path) => {
            let path_str = path.to_string();
            let ext = path_str.rsplit('.').next().unwrap_or("png").to_lowercase();
            let format = match ext.as_str() {
                "svg" => "svg",
                "jpg" | "jpeg" => "jpg",
                "webp" => "webp",
                _ => "png",
            };
            Ok(CommandResult::success(ImageSaveResult {
                file_path: path_str,
                format: format.to_string(),
            }))
        }
        None => Ok(CommandResult::success(ImageSaveResult {
            file_path: String::new(),
            format: "png".to_string(),
        })),
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSaveResult {
    pub file_path: String,
    pub format: String,
}

/// Write data to file
#[tauri::command(rename_all = "camelCase", rename = "fileWrite")]
pub async fn file_write(
    payload: FileWritePayload,
) -> Result<CommandResult<()>, String> {
    match std::fs::write(&payload.file_path, &payload.data) {
        Ok(_) => Ok(CommandResult::success(())),
        Err(e) => Ok(CommandResult::error(&format!("写入文件失败: {}", e))),
    }
}

/// Save document dialog
#[tauri::command(rename_all = "camelCase", rename = "dialogSaveDocument")]
pub async fn dialog_save_document(
    app: AppHandle,
    payload: SaveDocumentPayload,
) -> Result<CommandResult<DocumentSaveResult>, String> {
    let default_name = payload.default_name.unwrap_or_else(|| "untitled.md".to_string());
    
    use tauri_plugin_dialog::DialogExt;
    let file = app.dialog()
        .file()
        .add_filter("Markdown", &["md"])
        .add_filter("HTML", &["html", "htm"])
        .add_filter("PDF", &["pdf"])
        .add_filter("PNG", &["png"])
        .set_file_name(&default_name)
        .blocking_save_file();

    match file {
        Some(path) => Ok(CommandResult::success(DocumentSaveResult {
            file_path: path.to_string(),
        })),
        None => Ok(CommandResult::success(DocumentSaveResult {
            file_path: String::new(),
        })),
    }
}

#[derive(Debug, serde::Serialize)]
pub struct DocumentSaveResult {
    pub file_path: String,
}

/// Receive log messages from the frontend and forward to Rust's log system
#[tauri::command(rename_all = "camelCase", rename = "logWrite")]
pub fn log_write(level: String, tag: String, message: String) {
    let msg = format!("[{}] {}", tag, message);
    match level.as_str() {
        "error" => log::error!("[WebView] {}", msg),
        "warn"  => log::warn!("[WebView] {}", msg),
        _       => log::info!("[WebView] {}", msg),
    }
}

/// Export document as PDF (simplified: writes HTML to file with .pdf extension)
#[tauri::command(rename_all = "camelCase", rename = "documentExportPdf")]
pub async fn document_export_pdf(
    payload: ExportPdfPayload,
) -> Result<CommandResult<()>, String> {
    // Write HTML to the requested path
    // For proper PDF conversion, a headless browser or wkhtmltopdf would be needed
    match fs::write(&payload.file_path, &payload.html) {
        Ok(_) => Ok(CommandResult::success(())),
        Err(e) => Ok(CommandResult::error(&format!("导出失败: {}", e))),
    }
}


