use crate::commands::CommandResult;
use crate::services::auth_service;

#[tauri::command(rename_all = "camelCase", rename = "authSignUp")]
pub fn auth_sign_up(email: String, password: String, username: String) -> CommandResult<auth_service::AuthSession> {
    match auth_service::sign_up(&email, &password, &username) {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "authSignIn")]
pub fn auth_sign_in(email: String, password: String) -> CommandResult<auth_service::AuthSession> {
    match auth_service::sign_in(&email, &password) {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "authSignOut")]
pub fn auth_sign_out() -> CommandResult<()> {
    let _ = auth_service::sign_out();
    CommandResult::success(())
}

#[tauri::command(rename_all = "camelCase", rename = "authGetUser")]
pub fn auth_get_user() -> CommandResult<Option<auth_service::AuthUser>> {
    match auth_service::get_current_user() {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "authGetSession")]
pub fn auth_get_session() -> CommandResult<Option<auth_service::AuthSession>> {
    match auth_service::get_session() {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}
