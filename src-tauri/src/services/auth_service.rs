use crate::db;
use crate::supabase;
use chrono::Utc;
use once_cell::sync::Lazy;
use serde::Serialize;
use tokio::sync::Mutex;

// Refresh slightly before expiry so a multi-request sync cannot cross the JWT deadline.
const TOKEN_REFRESH_SKEW_SECS: i64 = 120;
static SESSION_REFRESH_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

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

fn activate_user_database(user: &supabase::SupabaseUser) -> Result<(), String> {
    let legacy_username = supabase::get_username_from_user(user);
    db::prepare_user_db_on_first_login(&user.id, legacy_username)?;
    db::set_current_user(Some(&user.id));
    Ok(())
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
    activate_user_database(&sb_session.user)?;

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
    activate_user_database(&sb_session.user)?;

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

/// Return the current session with an access token that is valid for the next
/// refresh window. The periodic sync path uses this instead of reading the
/// session token directly, because the app can stay open for many hours.
pub async fn ensure_fresh_session() -> Result<supabase::SupabaseSession, String> {
    let session = supabase::CURRENT_SESSION
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or_else(|| "未登录，无法同步".to_string())?;
    let now = Utc::now().timestamp();
    if session.expires_at > now.saturating_add(TOKEN_REFRESH_SKEW_SECS) {
        return Ok(session);
    }

    // Auth bootstrap and the periodic timer can reach this path together. Re-check
    // after taking the lock so only one request rotates the refresh token.
    let _refresh_guard = SESSION_REFRESH_LOCK.lock().await;
    let current = supabase::CURRENT_SESSION
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or_else(|| "未登录，无法同步".to_string())?;
    let now = Utc::now().timestamp();
    if current.expires_at > now.saturating_add(TOKEN_REFRESH_SKEW_SECS) {
        return Ok(current);
    }

    match supabase::refresh_session(&current.refresh_token).await {
        Ok(new_session) => {
            supabase::persist_session(&new_session);
            if let Ok(mut current_session) = supabase::CURRENT_SESSION.lock() {
                *current_session = Some(new_session.clone());
            }
            log::info!("[Auth] Access token refreshed before sync");
            Ok(new_session)
        }
        Err(error) => {
            // A temporary network failure should not invalidate a token that is
            // still usable; the next sync can retry the refresh.
            if current.expires_at > now {
                log::warn!(
                    "[Auth] Token refresh deferred (current token still valid): {}",
                    error
                );
                Ok(current)
            } else {
                Err(format!("登录已过期，请重新登录: {}", error))
            }
        }
    }
}

pub async fn get_session() -> Result<Option<AuthSession>, String> {
    let session = supabase::CURRENT_SESSION
        .lock()
        .map_err(|e| e.to_string())?
        .clone();

    let Some(_) = session else {
        return Ok(None);
    };

    // Step 1: refresh before expiry (also used by periodic sync).
    let session = match ensure_fresh_session().await {
        Ok(session) => session,
        Err(error) => {
            log::warn!("[Auth] Token refresh failed: {}", error);
            // 刷新失败 → session 已失效（可能被服务端吊销）
            supabase::clear_persisted_session();
            if let Ok(mut current) = supabase::CURRENT_SESSION.lock() {
                *current = None;
            }
            db::set_current_user(None);
            return Ok(None);
        }
    };

    // Step 2: 向 Supabase 验证 token 是否仍有效
    match supabase::get_user(&session.access_token).await {
        Ok(Some(user)) => {
            // Token 有效：使用服务端最新用户资料替换持久化会话中的旧元数据。
            let mut verified_session = session;
            verified_session.user = user;
            activate_user_database(&verified_session.user)?;
            supabase::persist_session(&verified_session);
            if let Ok(mut current) = supabase::CURRENT_SESSION.lock() {
                *current = Some(verified_session.clone());
            }
            Ok(Some(map_session(&verified_session)))
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
    let user_id = session.user.id.clone();
    activate_user_database(&session.user).ok()?;

    // Store in global
    if let Ok(mut s) = supabase::CURRENT_SESSION.lock() {
        *s = Some(session);
    }

    Some(user_id)
}
