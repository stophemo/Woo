#![allow(dead_code)]
use crate::commands::CommandResult;
use serde::Deserialize;
use tauri::AppHandle;
use std::fs;
use base64::{Engine as _, engine::general_purpose::STANDARD};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

/// Save image dialog
#[tauri::command(rename_all = "camelCase", rename = "dialogSaveImage")]
pub async fn dialog_save_image(
    app: AppHandle,
    default_name: Option<String>,
) -> Result<CommandResult<ImageSaveResult>, String> {
    let default_name = default_name.unwrap_or_else(|| "mindmap.png".to_string());
    
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

/// Write data to file. When `is_base64` is true, decode `data` as standard
/// base64 and write the raw bytes (used for binary exports like PNG);
/// otherwise write the string as-is (text like Markdown / TXT).
#[tauri::command(rename_all = "camelCase", rename = "fileWrite")]
pub async fn file_write(
    file_path: String,
    data: String,
    is_base64: Option<bool>,
) -> Result<CommandResult<()>, String> {
    let write_result = if is_base64.unwrap_or(false) {
        match STANDARD.decode(data.as_bytes()) {
            Ok(bytes) => fs::write(&file_path, &bytes),
            Err(e) => return Ok(CommandResult::error(&format!("base64 解码失败: {}", e))),
        }
    } else {
        fs::write(&file_path, &data)
    };
    match write_result {
        Ok(_) => Ok(CommandResult::success(())),
        Err(e) => Ok(CommandResult::error(&format!("写入文件失败: {}", e))),
    }
}

/// Save document dialog. Uses the filters supplied by the frontend when
/// present (each export format passes its own), falling back to a default set.
#[tauri::command(rename_all = "camelCase", rename = "dialogSaveDocument")]
pub async fn dialog_save_document(
    app: AppHandle,
    default_name: Option<String>,
    filters: Option<Vec<FileFilter>>,
) -> Result<CommandResult<DocumentSaveResult>, String> {
    let default_name = default_name.unwrap_or_else(|| "untitled.md".to_string());

    use tauri_plugin_dialog::DialogExt;
    let mut builder = app.dialog().file().set_file_name(&default_name);

    match &filters {
        Some(filters) if !filters.is_empty() => {
            for f in filters {
                let exts: Vec<&str> = f.extensions.iter().map(|s| s.as_str()).collect();
                builder = builder.add_filter(&f.name, &exts);
            }
        }
        _ => {
            builder = builder
                .add_filter("Markdown", &["md"])
                .add_filter("HTML", &["html", "htm"])
                .add_filter("PDF", &["pdf"])
                .add_filter("PNG", &["png"]);
        }
    }

    let file = builder.blocking_save_file();

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
#[serde(rename_all = "camelCase")]
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


