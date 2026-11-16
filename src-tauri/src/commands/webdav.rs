use crate::commands::CommandResult;
use crate::services::webdav_sync;

#[tauri::command(rename_all = "camelCase", rename = "webdavGetConfig")]
pub fn webdav_get_config() -> CommandResult<webdav_sync::WebdavConfigView> {
    CommandResult::success(webdav_sync::get_config_view())
}

#[tauri::command(rename_all = "camelCase", rename = "webdavSaveConfig")]
pub fn webdav_save_config(
    enabled: bool,
    url: String,
    username: String,
    password: Option<String>,
    folder: String,
) -> CommandResult<webdav_sync::WebdavConfigView> {
    match webdav_sync::save_config_from_input(enabled, &url, &username, password, &folder) {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "webdavTest")]
pub async fn webdav_test(
    url: String,
    username: String,
    password: Option<String>,
    folder: String,
) -> CommandResult<String> {
    match webdav_sync::test_connection(&url, &username, password, &folder).await {
        Ok(message) => CommandResult::success(message),
        Err(message) => CommandResult::error(&message),
    }
}
