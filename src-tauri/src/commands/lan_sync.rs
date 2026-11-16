use crate::commands::CommandResult;
use crate::services::lan_sync;

#[tauri::command(rename_all = "camelCase", rename = "lanSyncGetConfig")]
pub fn lan_sync_get_config() -> CommandResult<lan_sync::LanSyncView> {
    CommandResult::success(lan_sync::get_config_view())
}

#[tauri::command(rename_all = "camelCase", rename = "lanSyncSetEnabled")]
pub fn lan_sync_set_enabled(enabled: bool) -> CommandResult<lan_sync::LanSyncView> {
    match lan_sync::set_enabled(enabled) {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "lanPairDiscover")]
pub fn lan_pair_discover(code: String) -> CommandResult<lan_sync::LanPairResult> {
    match lan_sync::discover_pair(&code) {
        Ok(result) => CommandResult::success(result),
        Err(message) => CommandResult::error(&message),
    }
}
