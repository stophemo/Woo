use chrono::Utc;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

// ============================================================
// Configuration (matches electron/config/supabase.cjs)
// Priority: env vars → hardcoded fallback
// ============================================================

const FALLBACK_URL: &str = "https://urgpunxxupufmygmutxa.supabase.co";
const FALLBACK_ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InVyZ3B1bnh4dXB1Zm15Z211dHhhIiwicm9sZSI6ImFub24iLCJpYXQiOjE3Nzk1MTA1OTYsImV4cCI6MjA5NTA4NjU5Nn0.Ezs2uFwgrxkwCpLVfdByFC8m5PNjjP92i26Sg0YX-RI";

pub fn supabase_url() -> String {
    std::env::var("SUPABASE_URL").unwrap_or_else(|_| FALLBACK_URL.to_string())
}

pub fn supabase_anon_key() -> String {
    std::env::var("SUPABASE_ANON_KEY").unwrap_or_else(|_| FALLBACK_ANON_KEY.to_string())
}

// ============================================================
// Data directory (set once at startup for session file)
// ============================================================

static DATA_DIR: Lazy<Mutex<Option<PathBuf>>> = Lazy::new(|| Mutex::new(None));

pub fn set_data_dir(dir: PathBuf) {
    if let Ok(mut d) = DATA_DIR.lock() {
        *d = Some(dir);
    }
}

fn get_data_dir() -> Option<PathBuf> {
    DATA_DIR
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

// ============================================================
// HTTP client (shared, connection-pooled)
// ============================================================

/// 网络层失败统一使用该前缀，调用方用 `is_network_error` 判断，
/// 不要再在业务代码里裸写 `contains("网络错误:")`。
pub const NETWORK_ERROR_PREFIX: &str = "网络错误: ";

pub fn is_network_error(message: &str) -> bool {
    message.starts_with(NETWORK_ERROR_PREFIX)
}

fn network_error(error: impl std::fmt::Display) -> String {
    format!("{}{}", NETWORK_ERROR_PREFIX, error)
}

static HTTP: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        // 部分 CDN / WAF 会拒绝无 User-Agent 的请求，缺失时登录会直接失败。
        .user_agent(concat!(
            "Woo/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/stophemo/Woo)"
        ))
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create reqwest client")
});

// ============================================================
// Session persistence (matches supabase-auth.json from JS SDK)
// ============================================================

pub static CURRENT_SESSION: Lazy<Mutex<Option<SupabaseSession>>> = Lazy::new(|| Mutex::new(None));

/// 会话锁中毒不应导致永久无法登录：总是取回内部数据。
pub fn current_session() -> Option<SupabaseSession> {
    CURRENT_SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

pub fn set_current_session(session: Option<SupabaseSession>) {
    *CURRENT_SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = session;
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SupabaseUserMetadata {
    pub username: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub woo_lock_password_hash: Option<String>,
    #[serde(default)]
    pub woo_lock_password_mode: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SupabaseUser {
    pub id: String,
    pub email: Option<String>,
    #[serde(default)]
    pub user_metadata: SupabaseUserMetadata,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SupabaseSession {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub expires_at: i64,
    pub refresh_token: String,
    pub user: SupabaseUser,
}

fn session_file_key() -> String {
    format!("{}-auth-token", supabase_url())
}

fn session_file_path() -> Option<PathBuf> {
    get_data_dir().map(|d| d.join("supabase-auth.json"))
}

pub fn persist_session(session: &SupabaseSession) -> bool {
    let Some(path) = session_file_path() else {
        log::error!("[Supabase] 数据目录未初始化，会话无法持久化");
        return false;
    };
    let key = session_file_key();
    let session_json = match serde_json::to_string(session) {
        Ok(json) => json,
        Err(error) => {
            log::error!("[Supabase] 序列化会话失败: {}", error);
            return false;
        }
    };
    let mut map = serde_json::Map::new();
    map.insert(key, serde_json::Value::String(session_json));
    let content = serde_json::to_string(&map).unwrap_or_default();

    // 先写临时文件再原子替换：写到一半被杀进程会留下损坏的会话文件，
    // 下次启动就只能重新登录。
    let temp_path = path.with_extension("json.tmp");
    if let Err(error) = std::fs::write(&temp_path, &content) {
        log::error!("[Supabase] 写入会话临时文件失败 {:?}: {}", temp_path, error);
        return false;
    }
    if let Err(error) = std::fs::rename(&temp_path, &path) {
        log::error!("[Supabase] 原子替换会话文件失败 {:?}: {}", path, error);
        let _ = std::fs::remove_file(&temp_path);
        return false;
    }
    log::info!("[Supabase] Session persisted to {:?}", path);
    true
}

pub fn read_persisted_session() -> Option<SupabaseSession> {
    let path = session_file_path()?;
    if !path.exists() {
        return None;
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) => {
            log::warn!("[Supabase] 读取会话文件失败 {:?}: {}", path, error);
            return None;
        }
    };
    let map: serde_json::Map<String, serde_json::Value> = match serde_json::from_str(&content) {
        Ok(map) => map,
        Err(error) => {
            log::warn!("[Supabase] 会话文件格式无效 {:?}: {}", path, error);
            return None;
        }
    };
    let key = session_file_key();
    let Some(session_str) = map.get(&key).and_then(|value| value.as_str()) else {
        return None;
    };
    match serde_json::from_str::<SupabaseSession>(session_str) {
        Ok(session) => Some(session),
        Err(error) => {
            // 文件已损坏（历史版本退出登录时写过 "{}"）：直接删掉自愈，
            // 否则每次启动都要告警一次，且永远不会被清理。
            log::warn!("[Supabase] 会话内容无法解析，已删除损坏的会话文件: {}", error);
            drop(map);
            let _ = std::fs::remove_file(&path);
            None
        }
    }
}

pub fn clear_persisted_session() {
    let Some(path) = session_file_path() else {
        return;
    };
    // 直接删文件。旧实现写入 `{"<key>":"{}"}`，读取端解析失败后会一直
    // 被当成「有文件但内容无效」，每次启动刷一条告警。
    match std::fs::remove_file(&path) {
        Ok(()) => log::info!("[Supabase] 已删除本地会话文件 {:?}", path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => log::warn!("[Supabase] 清除本地会话失败 {:?}: {}", path, error),
    }
    let temp_path = path.with_extension("json.tmp");
    let _ = std::fs::remove_file(temp_path);
}

pub fn get_username_from_user(user: &SupabaseUser) -> Option<&str> {
    user.user_metadata
        .username
        .as_deref()
        .or_else(|| user.user_metadata.display_name.as_deref())
}

// ============================================================
// Token refresh
// ============================================================

/// 刷新失败的原因分类。调用方据此决定是「保留会话稍后重试」还是「彻底登出」，
/// 不能仅凭错误文案猜测——旧实现用 `contains("网络错误:")` 判断，
/// 一旦文案微调就会误清用户登录态。
#[derive(Debug)]
pub enum RefreshError {
    /// 网络/超时等可重试问题。
    Network(String),
    /// refresh token 已被服务端判为无效/吊销。
    Invalid(String),
    /// 服务端暂时性错误（5xx 等）。
    Server(String),
}

pub async fn refresh_session(
    refresh_token: &str,
) -> Result<SupabaseSession, RefreshError> {
    let url = format!("{}/auth/v1/token?grant_type=refresh_token", supabase_url());
    let body = serde_json::json!({ "refresh_token": refresh_token });

    let resp = HTTP
        .post(&url)
        .header("apikey", supabase_anon_key())
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| RefreshError::Network(e.to_string()))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if status.is_success() {
        return serde_json::from_str(&text)
            .map_err(|e| RefreshError::Server(format!("解析响应失败: {}", e)));
    }

    let message = extract_auth_error(&text);
    log_failed_auth_response("refresh_session", status, &text);
    if status.is_server_error() {
        Err(RefreshError::Server(message))
    } else {
        Err(RefreshError::Invalid(message))
    }
}

// ============================================================
// Auth API
// ============================================================

/// 注册账号。
///
/// 返回 `Ok(None)` 表示 Supabase 项目开启了邮箱确认：账号已创建，但本次响应
/// 不包含会话（没有 access_token）。旧实现直接按会话反序列化，会报
/// “解析响应失败: missing field `access_token`”，让用户误以为注册失败。
pub async fn sign_up(
    email: &str,
    password: &str,
    username: &str,
) -> Result<Option<SupabaseSession>, String> {
    let url = format!("{}/auth/v1/signup", supabase_url());
    let body = serde_json::json!({
        "email": email,
        "password": password,
        "data": {
            "username": username,
            "display_name": username,
        }
    });

    let resp = HTTP
        .post(&url)
        .header("apikey", supabase_anon_key())
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(network_error)?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if status.is_success() {
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;
        if value.get("access_token").is_none() {
            log::info!("[Supabase] 注册成功，等待邮箱确认");
            return Ok(None);
        }
        serde_json::from_value(value)
            .map(Some)
            .map_err(|e| format!("解析响应失败: {}", e))
    } else {
        log_failed_auth_response("sign_up", status, &text);
        Err(extract_auth_error(&text))
    }
}

pub async fn sign_in(email: &str, password: &str) -> Result<SupabaseSession, String> {
    let url = format!("{}/auth/v1/token?grant_type=password", supabase_url());
    let body = serde_json::json!({
        "email": email,
        "password": password,
    });

    let resp = HTTP
        .post(&url)
        .header("apikey", supabase_anon_key())
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(network_error)?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if status.is_success() {
        serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))
    } else {
        log_failed_auth_response("sign_in", status, &text);
        Err(extract_auth_error(&text))
    }
}

pub async fn sign_out(access_token: &str) -> Result<(), String> {
    let url = format!("{}/auth/v1/logout", supabase_url());

    let resp = HTTP
        .post(&url)
        .header("apikey", supabase_anon_key())
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(network_error)?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        log::warn!("[Supabase] sign_out non-200: {}", text);
    }

    Ok(())
}

pub async fn get_user(access_token: &str) -> Result<Option<SupabaseUser>, String> {
    let url = format!("{}/auth/v1/user", supabase_url());

    let resp = HTTP
        .get(&url)
        .header("apikey", supabase_anon_key())
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(network_error)?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if status.is_success() {
        let user: SupabaseUser =
            serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;
        Ok(Some(user))
    } else if status == reqwest::StatusCode::UNAUTHORIZED {
        Ok(None)
    } else {
        Err(extract_auth_error(&text))
    }
}

/// Update the current user's lock settings in auth metadata.
/// Only the bcrypt hash is uploaded; the plaintext password never leaves the device.
pub async fn update_lock_settings(
    access_token: &str,
    password_hash: &str,
    mode: &str,
) -> Result<SupabaseUser, String> {
    let url = format!("{}/auth/v1/user", supabase_url());
    let body = serde_json::json!({
        "data": {
            "woo_lock_password_hash": password_hash,
            "woo_lock_password_mode": mode,
        }
    });

    let resp = HTTP
        .put(&url)
        .header("apikey", supabase_anon_key())
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(network_error)?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if status.is_success() {
        serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))
    } else {
        Err(format!("同步锁设置失败: {}", extract_auth_error(&text)))
    }
}

/// Keep the persisted and in-memory session aligned after fetching/updating user metadata.
pub fn cache_current_user(user: SupabaseUser) {
    let Some(mut session) = current_session() else {
        return;
    };
    session.user = user;
    persist_session(&session);
    set_current_session(Some(session));
}

/// Resolve username → email via Supabase RPC (matches authService.resolveEmailByUsername)
pub async fn resolve_email_by_username(username: &str) -> Result<String, String> {
    let url = format!("{}/rest/v1/rpc/get_email_by_username", supabase_url());
    let body = serde_json::json!({ "p_username": username });

    let resp = HTTP
        .post(&url)
        .header("apikey", supabase_anon_key())
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(network_error)?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if status.is_success() {
        // Response is a plain JSON string；用户名不存在时返回 null。
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed == "null" {
            return Err("该用户名尚未注册".to_string());
        }
        Ok(trimmed.trim_matches('"').trim().to_string())
    } else {
        log_failed_auth_response("resolve_email_by_username", status, &text);
        let msg = extract_data_error(&text);
        Err(format!("获取邮箱失败: {}", msg))
    }
}

// ============================================================
// Data API (PostgREST)
// ============================================================

pub async fn upsert(table: &str, rows: &[serde_json::Value], access_token: &str) -> Result<(), String> {
    if rows.is_empty() {
        return Ok(());
    }
    let url = format!("{}/rest/v1/{}", supabase_url(), table);

    let resp = HTTP
        .post(&url)
        .header("apikey", supabase_anon_key())
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .header("Prefer", "resolution=merge-duplicates")
        .json(rows)
        .send()
        .await
        .map_err(network_error)?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("上传失败 [{}]: {}", table, extract_data_error(&text)));
    }
    Ok(())
}

pub async fn select(
    table: &str,
    user_id: &str,
    last_sync: Option<&str>,
    access_token: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let mut url = format!(
        "{}/rest/v1/{}?user_id=eq.{}&order=update_time.asc",
        supabase_url(),
        table,
        user_id
    );

    if let Some(sync_time) = last_sync {
        url.push_str(&format!("&update_time=gt.{}", sync_time));
    }

    let resp = HTTP
        .get(&url)
        .header("apikey", supabase_anon_key())
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(network_error)?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if status.is_success() {
        if text.trim().is_empty() || text.trim() == "[]" {
            return Ok(Vec::new());
        }
        serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))
    } else {
        Err(format!("拉取失败 [{}]: {}", table, extract_data_error(&text)))
    }
}

pub async fn delete(table: &str, id_field: &str, ids: &[&str], access_token: &str) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let id_list = ids.join(",");
    let url = format!(
        "{}/rest/v1/{}?{}=in.({})",
        supabase_url(),
        table,
        id_field,
        id_list
    );

    let resp = HTTP
        .delete(&url)
        .header("apikey", supabase_anon_key())
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(network_error)?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!(
            "删除失败 [{}]: {}",
            table,
            extract_data_error(&text)
        ));
    }
    Ok(())
}

pub async fn insert_tombstone(
    table_name: &str,
    record_id: &str,
    user_id: &str,
    access_token: &str,
) -> Result<(), String> {
    let url = format!("{}/rest/v1/sync_tombstone", supabase_url());
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%S%.f+00:00").to_string();
    let body = serde_json::json!([{
        "table_name": table_name,
        "record_id": record_id,
        "user_id": user_id,
        "deleted_at": now,
    }]);

    let resp = HTTP
        .post(&url)
        .header("apikey", supabase_anon_key())
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .header("Prefer", "resolution=merge-duplicates")
        .json(&body)
        .send()
        .await
        .map_err(network_error)?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("写入墓碑失败: {}", extract_data_error(&text)));
    }
    Ok(())
}

pub async fn select_tombstones(
    user_id: &str,
    last_pull: &str,
    access_token: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let url = format!("{}/rest/v1/sync_tombstone", supabase_url());

    let params = vec![
        ("user_id", format!("eq.{}", user_id)),
        ("deleted_at", format!("gt.{}", last_pull)),
        ("order", "deleted_at.asc".to_string()),
    ];

    let resp = HTTP
        .get(&url)
        .header("apikey", supabase_anon_key())
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/json")
        .query(&params)
        .send()
        .await
        .map_err(network_error)?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if status.is_success() {
        if text.trim().is_empty() || text.trim() == "[]" {
            return Ok(Vec::new());
        }
        serde_json::from_str(&text).map_err(|e| format!("解析墓碑失败: {}", e))
    } else {
        Err(format!("拉取墓碑失败: {}", extract_data_error(&text)))
    }
}

pub async fn cleanup_tombstones(user_id: &str, cutoff: &str, access_token: &str) -> Result<(), String> {
    let url = format!("{}/rest/v1/sync_tombstone", supabase_url());

    let params = vec![
        ("user_id", format!("eq.{}", user_id)),
        ("deleted_at", format!("lt.{}", cutoff)),
    ];

    let resp = HTTP
        .delete(&url)
        .header("apikey", supabase_anon_key())
        .header("Authorization", format!("Bearer {}", access_token))
        .query(&params)
        .send()
        .await
        .map_err(network_error)?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        log::warn!("[Supabase] cleanup_tombstones failed: {}", text);
    }
    Ok(())
}

// ============================================================
// Error parsing helpers
// ============================================================

/// 登录类请求失败时把状态码和响应体（截断）写进日志。
/// 登录问题没有日志就只能靠猜，这是排查的唯一线索。
fn log_failed_auth_response(action: &str, status: reqwest::StatusCode, body: &str) {
    let preview: String = body.chars().take(400).collect();
    log::warn!(
        "[Supabase] {} 失败: HTTP {} body={}",
        action,
        status.as_u16(),
        preview
    );
}

fn extract_auth_error(response_body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(response_body) {
        // 先看机器可读的错误码：同一段英文提示在不同 Supabase 版本里会变，
        // 错误码则相对稳定，可以稳定映射成中文指引。
        if let Some(code) = v.get("error_code").and_then(|c| c.as_str()) {
            let friendly = match code {
                "invalid_credentials" => "邮箱/用户名或密码不正确",
                "email_not_confirmed" => "邮箱尚未验证，请先点击注册邮件里的确认链接",
                "user_already_exists" | "email_exists" => "该邮箱已注册，请直接登录",
                "weak_password" => "密码强度不足，请使用至少 6 位密码",
                "over_email_send_rate_limit" | "over_request_rate_limit" => {
                    "请求过于频繁，请稍后再试"
                }
                "user_banned" => "该账号已被停用",
                "signup_disabled" => "当前项目已关闭注册",
                _ => "",
            };
            if !friendly.is_empty() {
                return friendly.to_string();
            }
        }
        for key in ["error_description", "msg", "error", "message"] {
            if let Some(msg) = v.get(key).and_then(|m| m.as_str()) {
                if !msg.is_empty() {
                    return msg.to_string();
                }
            }
        }
    }
    response_body.to_string()
}

fn extract_data_error(response_body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(response_body) {
        if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
            return msg.to_string();
        }
        if let Some(details) = v.get("details").and_then(|m| m.as_str()) {
            return details.to_string();
        }
    }
    response_body.to_string()
}
