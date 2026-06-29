use crate::commands::CommandResult;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadExternalFileResult {
    pub file_path: String,
    pub file_name: String,
    pub content: String,
}

/// 读取外部文件内容（文本文件 .md/.txt 等）
#[tauri::command(rename_all = "camelCase", rename = "readExternalFile")]
pub async fn read_external_file(file_path: String) -> Result<CommandResult<ReadExternalFileResult>, String> {
    let path = Path::new(&file_path);

    // 安全检查：拒绝非文件路径
    if !path.exists() {
        return Ok(CommandResult::error("文件不存在"));
    }
    if !path.is_file() {
        return Ok(CommandResult::error("路径不是文件"));
    }

    // 读取文件大小限制（10MB），防止大文件卡死编辑器
    let metadata = fs::metadata(&file_path).map_err(|e| format!("读取文件元数据失败: {}", e))?;
    if metadata.len() > 10 * 1024 * 1024 {
        return Ok(CommandResult::error("文件超过 10MB，暂不支持打开"));
    }

    // 读取文件内容（UTF-8）
    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("读取文件失败: {}", e))?;

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("未命名")
        .to_string();

    Ok(CommandResult::success(ReadExternalFileResult {
        file_path,
        file_name,
        content,
    }))
}

/// 写回外部文件
#[tauri::command(rename_all = "camelCase", rename = "writeExternalFile")]
pub async fn write_external_file(file_path: String, content: String) -> Result<CommandResult<()>, String> {
    fs::write(&file_path, &content)
        .map_err(|e| format!("写入文件失败: {}", e))?;
    Ok(CommandResult::success(()))
}

/// 前端启动后调用，拉取在 frontend 就绪前收到的文件打开请求
#[tauri::command(rename_all = "camelCase", rename = "popPendingOpenFiles")]
pub fn pop_pending_open_files() -> Vec<String> {
    crate::drain_pending_open_files()
}
