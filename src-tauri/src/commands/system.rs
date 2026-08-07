#![allow(dead_code)]
use crate::commands::CommandResult;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;
use std::fs;
use tauri::AppHandle;

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
    let file = app
        .dialog()
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

/// 选择一张图片，前端随后会将它作为 data URL 插入文稿。
#[tauri::command(rename_all = "camelCase", rename = "dialogOpenImage")]
pub async fn dialog_open_image(app: AppHandle) -> Result<CommandResult<ImageOpenResult>, String> {
    use tauri_plugin_dialog::DialogExt;
    let file = app
        .dialog()
        .file()
        .add_filter("图片", &["png", "jpg", "jpeg", "gif", "webp", "svg"])
        .blocking_pick_file();
    Ok(CommandResult::success(ImageOpenResult {
        file_path: file.map(|path| path.to_string()).unwrap_or_default(),
    }))
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageOpenResult {
    pub file_path: String,
}

/// 读取图片并编码为 data URL，避免 WebView 直接访问本机绝对路径。
#[tauri::command(rename_all = "camelCase", rename = "readImageFile")]
pub async fn read_image_file(file_path: String) -> Result<CommandResult<ImageFileResult>, String> {
    let path = std::path::Path::new(&file_path);
    if !path.is_file() {
        return Ok(CommandResult::error("图片文件不存在"));
    }
    let metadata = fs::metadata(path).map_err(|e| format!("读取图片元数据失败: {}", e))?;
    if metadata.len() > 20 * 1024 * 1024 {
        return Ok(CommandResult::error("图片超过 20MB，暂不支持插入"));
    }
    let bytes = fs::read(path).map_err(|e| format!("读取图片失败: {}", e))?;
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let mime_type = match extension.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "image/png",
    };
    Ok(CommandResult::success(ImageFileResult {
        data: STANDARD.encode(bytes),
        mime_type: mime_type.to_string(),
    }))
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageFileResult {
    pub data: String,
    pub mime_type: String,
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
        "warn" => log::warn!("[WebView] {}", msg),
        _ => log::info!("[WebView] {}", msg),
    }
}
