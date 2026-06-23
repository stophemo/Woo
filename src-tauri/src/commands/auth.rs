use crate::commands::CommandResult;
use crate::services::auth_service;

#[tauri::command(rename_all = "camelCase", rename = "authSignUp")]
pub async fn auth_sign_up(email: String, password: String, username: String) -> CommandResult<auth_service::AuthSession> {
    match auth_service::sign_up(&email, &password, &username).await {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "authSignIn")]
pub async fn auth_sign_in(email: String, password: String) -> CommandResult<auth_service::AuthSession> {
    match auth_service::sign_in(&email, &password).await {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "authSignOut")]
pub async fn auth_sign_out() -> CommandResult<()> {
    match auth_service::sign_out().await {
        Ok(_) => CommandResult::success(()),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "authGetUser")]
pub fn auth_get_user() -> CommandResult<Option<auth_service::AuthUser>> {
    match auth_service::get_current_user() {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "authGetSession")]
pub async fn auth_get_session() -> CommandResult<Option<auth_service::AuthSession>> {
    match auth_service::get_session().await {
        Ok(data) => CommandResult::success(data),
        Err(msg) => CommandResult::error(&msg),
    }
}
