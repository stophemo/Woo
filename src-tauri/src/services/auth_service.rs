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

/// 登录/注册成功后统一收尾：切换用户库 → 落盘会话 → 写入内存会话。
///
/// 三者顺序很重要：先切库，保证后续任何写入都落在正确的用户数据库；
/// 落盘失败不阻断本次登录（内存会话仍可用），但必须留下明确的日志，
/// 否则用户会看到“登录成功、重启又掉线”这种无法解释的现象。
fn adopt_session(sb_session: &supabase::SupabaseSession) -> Result<(), String> {
    activate_user_database(&sb_session.user)?;

    let persisted = supabase::persist_session(sb_session);
    supabase::set_current_session(Some(sb_session.clone()));

    if !persisted {
        log::error!(
            "[Auth] 会话无法持久化到本机，重启后需要重新登录（请检查数据目录写权限）"
        );
    } else {
        // 读回校验：确认写入内容可被正确解析，避免启动时静默丢失登录态。
        match supabase::read_persisted_session() {
            Some(restored) if restored.access_token == sb_session.access_token => {
                log::info!("[Auth] 会话已落盘并校验通过 (user={})", sb_session.user.id);
            }
            _ => log::error!("[Auth] 会话落盘后读回校验失败，重启后可能需要重新登录"),
        }
    }

    Ok(())
}

pub async fn sign_up(
    email: &str,
    password: &str,
    username: &str,
) -> Result<AuthSession, String> {
    if email.is_empty() || password.is_empty() || username.is_empty() {
        return Err("邮箱、密码和用户名不能为空".to_string());
    }
    if password.len() < 6 {
        return Err("密码长度不能少于 6 位".to_string());
    }

    log::info!("[Auth] 注册请求: username={}", username);
    let outcome = supabase::sign_up(email, password, username).await?;

    let Some(sb_session) = outcome else {
        // 项目开启了邮箱确认：账号已创建但没有会话，前端据此提示“查看邮箱”。
        return Ok(AuthSession {
            user: None,
            access_token: None,
        });
    };

    adopt_session(&sb_session)?;
    Ok(map_session(&sb_session))
}

pub async fn sign_in(identifier: &str, password: &str) -> Result<AuthSession, String> {
    if identifier.is_empty() {
        return Err("请输入邮箱或用户名".to_string());
    }
    if password.is_empty() {
        return Err("请输入密码".to_string());
    }

    // If identifier is not an email, resolve username → email via RPC
    let email = if identifier.contains('@') {
        identifier.to_string()
    } else {
        log::info!("[Auth] 登录：按用户名解析邮箱");
        supabase::resolve_email_by_username(identifier)
            .await
            .map_err(|error| {
                if supabase::is_network_error(&error) {
                    format!("{}（请检查网络或代理设置）", error)
                } else {
                    error
                }
            })?
    };

    log::info!("[Auth] 登录：请求 access token");
    let sb_session = supabase::sign_in(&email, password).await.map_err(|error| {
        log::warn!("[Auth] 登录失败: {}", error);
        if supabase::is_network_error(&error) {
            format!("{}（请检查网络或代理设置）", error)
        } else {
            error
        }
    })?;

    log::info!(
        "[Auth] 登录成功: user={} expires_at={}",
        sb_session.user.id,
        sb_session.expires_at
    );

    adopt_session(&sb_session)?;
    Ok(map_session(&sb_session))
}

pub async fn sign_out() -> Result<(), String> {
    let token = supabase::current_session().map(|session| session.access_token);

    if let Some(token) = token {
        supabase::sign_out(&token).await.ok();
    }

    clear_local_session();

    Ok(())
}

/// Clear only the session stored on this device. Used when the refresh token is
/// definitively revoked; it must not wait for a network request.
pub fn clear_local_session() {
    supabase::clear_persisted_session();
    supabase::set_current_session(None);
    db::set_current_user(None);
}

pub fn get_current_user() -> Result<Option<AuthUser>, String> {
    Ok(supabase::current_session().map(|session| map_user(&session.user)))
}

/// 把「刷新失败」翻译成用户能行动的结论。
/// 只有确定失效（refresh token 被吊销）才清本地会话；网络/服务端问题必须保留。
enum RefreshFailure {
    KeepCurrent(String),
    Revoked(String),
}

fn resolve_refresh_failure(
    error: supabase::RefreshError,
    current: &supabase::SupabaseSession,
    now: i64,
) -> RefreshFailure {
    match error {
        supabase::RefreshError::Network(detail) | supabase::RefreshError::Server(detail) => {
            if current.expires_at > now {
                // 当前 token 仍有效，下一轮再刷新。
                RefreshFailure::KeepCurrent(detail)
            } else {
                // 无法刷新且已过期：保留 refresh token，联网后重试，不清会话。
                RefreshFailure::KeepCurrent(format!("登录凭证已过期，无法自动续期（{}）", detail))
            }
        }
        supabase::RefreshError::Invalid(detail) => RefreshFailure::Revoked(detail),
    }
}

/// Return the current session with an access token that is valid for the next
/// refresh window. The periodic sync path uses this instead of reading the
/// session token directly, because the app can stay open for many hours.
pub async fn ensure_fresh_session() -> Result<supabase::SupabaseSession, String> {
    let session = supabase::current_session().ok_or_else(|| "未登录，无法同步".to_string())?;
    let now = Utc::now().timestamp();
    if session.expires_at > now.saturating_add(TOKEN_REFRESH_SKEW_SECS) {
        return Ok(session);
    }

    // Auth bootstrap and the periodic timer can reach this path together. Re-check
    // after taking the lock so only one request rotates the refresh token.
    let _refresh_guard = SESSION_REFRESH_LOCK.lock().await;
    let current = supabase::current_session().ok_or_else(|| "未登录，无法同步".to_string())?;
    let now = Utc::now().timestamp();
    if current.expires_at > now.saturating_add(TOKEN_REFRESH_SKEW_SECS) {
        return Ok(current);
    }

    match supabase::refresh_session(&current.refresh_token).await {
        Ok(new_session) => {
            supabase::persist_session(&new_session);
            supabase::set_current_session(Some(new_session.clone()));
            log::info!("[Auth] Access token refreshed before sync");
            Ok(new_session)
        }
        Err(error) => match resolve_refresh_failure(error, &current, now) {
            RefreshFailure::KeepCurrent(detail) => {
                log::warn!("[Auth] Token refresh deferred: {}", detail);
                Ok(current)
            }
            RefreshFailure::Revoked(detail) => {
                log::warn!("[Auth] Refresh token revoked: {}", detail);
                clear_local_session();
                Err(format!("登录已失效，请重新登录: {}", detail))
            }
        },
    }
}

pub async fn get_session() -> Result<Option<AuthSession>, String> {
    let Some(cached_session) = supabase::current_session() else {
        return Ok(None);
    };

    // Step 1: refresh before expiry (also used by periodic sync).
    let session = match ensure_fresh_session().await {
        Ok(session) => session,
        Err(error) => {
            log::warn!("[Auth] Token refresh failed: {}", error);
            // refresh token 被吊销时 ensure_fresh_session 已清空内存会话，
            // 这里用状态判断而不是匹配错误文案。
            if supabase::current_session().is_none() {
                return Ok(None);
            }
            // 网络问题/临时失败 → 继续用本地缓存的会话，保持本地优先可用性。
            return Ok(Some(map_session(&cached_session)));
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
            supabase::set_current_session(Some(verified_session.clone()));
            Ok(Some(map_session(&verified_session)))
        }
        Ok(None) => {
            // 服务端返回 401 → token 已被吊销
            log::info!("[Auth] Session revoked on server, clearing");
            clear_local_session();
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
    if let Err(error) = activate_user_database(&session.user) {
        // 用户库准备失败不应阻止应用启动，但必须暴露出来，否则会表现为“登录态丢失”。
        log::error!("[Auth] 恢复会话时切换用户库失败: {}", error);
        return None;
    }

    supabase::set_current_session(Some(session));
    log::info!("[Auth] Restored session for user id: {}", user_id);
    Some(user_id)
}
