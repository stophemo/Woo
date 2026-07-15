use crate::db;
use crate::supabase;
use chrono::Utc;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    pub id: String,
    pub email: Option<String>,
    pub username: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSession {
    pub user: Option<AuthUser>,
    pub access_token: Option<String>,
}

fn map_user(sb_user: &supabase::SupabaseUser) -> AuthUser {
    AuthUser {
        id: sb_user.id.clone(),
        email: sb_user.email.clone(),
        username: sb_user
            .user_metadata
            .username
            .as_deref()
            .or_else(|| sb_user.user_metadata.display_name.as_deref())
            .map(|s| s.to_string()),
    }
}

fn map_session(sb_session: &supabase::SupabaseSession) -> AuthSession {
    AuthSession {
        user: Some(map_user(&sb_session.user)),
        access_token: Some(sb_session.access_token.clone()),
    }
}

pub async fn sign_up(email: &str, password: &str, username: &str) -> Result<AuthSession, String> {
    if email.is_empty() || password.is_empty() || username.is_empty() {
        return Err("邮箱、密码和用户名不能为空".to_string());
    }
    if password.len() < 6 {
        return Err("密码长度不能少于 6 位".to_string());
    }

    let sb_session = supabase::sign_up(email, password, username).await?;

    // Persist session to supabase-auth.json
    supabase::persist_session(&sb_session);
    if let Ok(mut s) = supabase::CURRENT_SESSION.lock() {
        *s = Some(sb_session.clone());
    }

    // Switch to user-specific database (matches sign_in behavior)
    let username = supabase::get_username_from_session(&sb_session)
        .map(|s| s.to_string())
        .unwrap_or_default();
    if !username.is_empty() {
        db::copy_local_to_user_db_on_first_login(&username).ok();
        db::set_current_user(Some(&username));
    }

    Ok(map_session(&sb_session))
}

pub async fn sign_in(identifier: &str, password: &str) -> Result<AuthSession, String> {
    if identifier.is_empty() {
        return Err("请输入邮箱或用户名".to_string());
    }

    // If identifier is not an email, resolve username → email via RPC
    let email = if identifier.contains('@') {
        identifier.to_string()
    } else {
        supabase::resolve_email_by_username(identifier).await?
    };

    let sb_session = supabase::sign_in(&email, password).await?;

    // After login, check if user DB should be created
    let username = supabase::get_username_from_session(&sb_session)
        .map(|s| s.to_string())
        .unwrap_or_default();

    if !username.is_empty() {
        db::copy_local_to_user_db_on_first_login(&username).ok();
        db::set_current_user(Some(&username));
    }

    // Persist session
    supabase::persist_session(&sb_session);
    if let Ok(mut s) = supabase::CURRENT_SESSION.lock() {
        *s = Some(sb_session.clone());
    }

    Ok(map_session(&sb_session))
}

pub async fn sign_out() -> Result<(), String> {
    let token = supabase::CURRENT_SESSION
        .lock()
        .ok()
        .and_then(|s| s.clone())
        .map(|s| s.access_token);

    if let Some(token) = token {
        supabase::sign_out(&token).await.ok();
    }

    supabase::clear_persisted_session();
    if let Ok(mut s) = supabase::CURRENT_SESSION.lock() {
        *s = None;
    }

    db::set_current_user(None);

    Ok(())
}

pub fn get_current_user() -> Result<Option<AuthUser>, String> {
    let session = supabase::CURRENT_SESSION
        .lock()
        .map_err(|e| e.to_string())?
        .clone();

    match session {
        Some(s) => Ok(Some(map_user(&s.user))),
        None => Ok(None),
    }
}

pub async fn get_session() -> Result<Option<AuthSession>, String> {
    let session = supabase::CURRENT_SESSION
        .lock()
        .map_err(|e| e.to_string())?
        .clone();

    let Some(s) = session else {
        return Ok(None);
    };

    // Step 1: refresh if expired
    let session = {
        let now = Utc::now().timestamp();
        if now >= s.expires_at {
            match supabase::refresh_session(&s.refresh_token).await {
                Ok(new_session) => {
                    supabase::persist_session(&new_session);
                    if let Ok(mut current) = supabase::CURRENT_SESSION.lock() {
                        *current = Some(new_session.clone());
                    }
                    new_session
                }
                Err(e) => {
                    log::warn!("[Auth] Token refresh failed: {}", e);
                    // 刷新失败 → session 已失效（可能被服务端吊销）
                    supabase::clear_persisted_session();
                    if let Ok(mut current) = supabase::CURRENT_SESSION.lock() {
                        *current = None;
                    }
                    db::set_current_user(None);
                    return Ok(None);
                }
            }
        } else {
            s
        }
    };

    // Step 2: 向 Supabase 验证 token 是否仍有效
    match supabase::get_user(&session.access_token).await {
        Ok(Some(_)) => {
            // Token 有效
            Ok(Some(map_session(&session)))
        }
        Ok(None) => {
            // 服务端返回 401 → token 已被吊销
            log::info!("[Auth] Session revoked on server, clearing");
            supabase::clear_persisted_session();
            if let Ok(mut current) = supabase::CURRENT_SESSION.lock() {
                *current = None;
            }
            db::set_current_user(None);
            Ok(None)
        }
        Err(_) => {
            // 网络错误（离线等）→ 容忍，使用本地缓存的 session
            log::warn!("[Auth] Server validation failed (offline?), using cached session");
            Ok(Some(map_session(&session)))
        }
    }
}

/// Called at startup to restore session from supabase-auth.json
pub fn try_restore_session() -> Option<String> {
    let session = supabase::read_persisted_session()?;
    let username = supabase::get_username_from_session(&session)?.to_string();

    // Store in global
    if let Ok(mut s) = supabase::CURRENT_SESSION.lock() {
        *s = Some(session);
    }

    Some(username)
}
