use crate::commands::CommandResult;
use crate::services::mcp::{self, McpView};

#[tauri::command(rename_all = "camelCase", rename = "mcpGetConfig")]
pub fn mcp_get_config() -> CommandResult<McpView> {
    CommandResult::success(mcp::get_config_view())
}

#[tauri::command(rename_all = "camelCase", rename = "mcpSetEnabled")]
pub async fn mcp_set_enabled(enabled: bool) -> CommandResult<McpView> {
    match mcp::set_enabled(enabled).await {
        Ok(view) => CommandResult::success(view),
        Err(error) => CommandResult::error(&error),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "mcpSetAllowWrites")]
pub fn mcp_set_allow_writes(allow_writes: bool) -> CommandResult<McpView> {
    match mcp::set_allow_writes(allow_writes) {
        Ok(view) => CommandResult::success(view),
        Err(error) => CommandResult::error(&error),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "mcpSetPort")]
pub async fn mcp_set_port(port: u16) -> CommandResult<McpView> {
    match mcp::set_port(port).await {
        Ok(view) => CommandResult::success(view),
        Err(error) => CommandResult::error(&error),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "mcpRegenerateToken")]
pub fn mcp_regenerate_token() -> CommandResult<McpView> {
    match mcp::regenerate_token() {
        Ok(view) => CommandResult::success(view),
        Err(error) => CommandResult::error(&error),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "mcpSetEditorPresence")]
pub fn mcp_set_editor_presence(document_id: Option<String>, dirty: bool) -> CommandResult<()> {
    mcp::set_editor_presence(document_id, dirty);
    CommandResult::success(())
}
