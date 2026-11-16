use crate::commands::CommandResult;
use crate::supabase;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfigView {
    pub base_url: String,
    pub default_base_url: String,
    pub is_custom: bool,
}

fn view() -> ServerConfigView {
    ServerConfigView {
        base_url: supabase::supabase_url(),
        default_base_url: supabase::default_supabase_url().to_string(),
        is_custom: supabase::is_custom_server_url(),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "serverGetConfig")]
pub fn server_get_config() -> CommandResult<ServerConfigView> {
    CommandResult::success(view())
}

#[tauri::command(rename_all = "camelCase", rename = "serverSaveConfig")]
pub fn server_save_config(base_url: String) -> CommandResult<ServerConfigView> {
    match supabase::save_server_url(&base_url) {
        Ok(_) => CommandResult::success(view()),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "serverTest")]
pub async fn server_test(base_url: Option<String>) -> CommandResult<String> {
    match supabase::test_server_connection(base_url).await {
        Ok(message) => CommandResult::success(message),
        Err(message) => CommandResult::error(&message),
    }
}
