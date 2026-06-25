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
    DATA_DIR.lock().ok().and_then(|d| d.clone())
}

// ============================================================
// HTTP client (shared, connection-pooled)
// ============================================================

static HTTP: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create reqwest client")
});

// ============================================================
// Session persistence (matches supabase-auth.json from JS SDK)
// ============================================================

pub static CURRENT_SESSION: Lazy<Mutex<Option<SupabaseSession>>> =
    Lazy::new(|| Mutex::new(None));

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SupabaseUserMetadata {
    pub username: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
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

pub fn persist_session(session: &SupabaseSession) {
    let path = match session_file_path() {
        Some(p) => p,
        None => return,
    };
    let key = session_file_key();
    let session_json = serde_json::to_string(session).unwrap_or_default();
    let mut map = serde_json::Map::new();
    map.insert(key, serde_json::Value::String(session_json));
    let content = serde_json::to_string(&map).unwrap_or_default();
    std::fs::write(&path, content).ok();
    log::info!("[Supabase] Session persisted to {:?}", path);
}

pub fn read_persisted_session() -> Option<SupabaseSession> {
    let path = session_file_path()?;
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&content).ok()?;
    let key = session_file_key();
    let session_str = map.get(&key)?.as_str()?;
    let session: SupabaseSession = serde_json::from_str(session_str).ok()?;
    Some(session)
}

pub fn clear_persisted_session() {
    let path = match session_file_path() {
        Some(p) => p,
        None => return,
    };
    let key = session_file_key();
    let mut map = serde_json::Map::new();
    map.insert(key, serde_json::Value::String("{}".to_string()));
    let content = serde_json::to_string(&map).unwrap_or_default();
    std::fs::write(&path, content).ok();
}

pub fn get_username_from_session(session: &SupabaseSession) -> Option<&str> {
    session
        .user
        .user_metadata
        .username
        .as_deref()
        .or_else(|| session.user.user_metadata.display_name.as_deref())
}

// ============================================================
// Token refresh
// ============================================================

pub async fn refresh_session(refresh_token: &str) -> Result<SupabaseSession, String> {
    let url = format!("{}/auth/v1/token?grant_type=refresh_token", supabase_url());
    let body = serde_json::json!({ "refresh_token": refresh_token });

    let resp = HTTP
        .post(&url)
        .header("apikey", supabase_anon_key())
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if status.is_success() {
        serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))
    } else {
        let msg = extract_auth_error(&text);
        Err(format!("令牌刷新失败: {}", msg))
    }
}

// ============================================================
// Auth API
// ============================================================

pub async fn sign_up(email: &str, password: &str, username: &str) -> Result<SupabaseSession, String> {
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
        .map_err(|e| format!("网络错误: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if status.is_success() {
        serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))
    } else {
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
        .map_err(|e| format!("网络错误: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if status.is_success() {
        serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))
    } else {
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
        .map_err(|e| format!("网络错误: {}", e))?;

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
        .map_err(|e| format!("网络错误: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if status.is_success() {
        let user: SupabaseUser = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;
        Ok(Some(user))
    } else if status == reqwest::StatusCode::UNAUTHORIZED {
        Ok(None)
    } else {
        Err(extract_auth_error(&text))
    }
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
        .map_err(|e| format!("网络错误: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if status.is_success() {
        // Response is a plain email string
        Ok(text.trim_matches('"').to_string())
    } else {
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
        .map_err(|e| format!("网络错误: {}", e))?;

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
        .map_err(|e| format!("网络错误: {}", e))?;

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
        .map_err(|e| format!("网络错误: {}", e))?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        log::warn!("[Supabase] delete failed [{}]: {}", table, text);
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
        .map_err(|e| format!("网络错误: {}", e))?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        log::warn!("[Supabase] insert_tombstone failed: {}", text);
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
        .map_err(|e| format!("网络错误: {}", e))?;

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
        .map_err(|e| format!("网络错误: {}", e))?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        log::warn!("[Supabase] cleanup_tombstones failed: {}", text);
    }
    Ok(())
}

// ============================================================
// Error parsing helpers
// ============================================================

fn extract_auth_error(response_body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(response_body) {
        if let Some(msg) = v.get("error_description").and_then(|m| m.as_str()) {
            return msg.to_string();
        }
        if let Some(msg) = v.get("msg").and_then(|m| m.as_str()) {
            return msg.to_string();
        }
        if let Some(msg) = v.get("error").and_then(|m| m.as_str()) {
            return msg.to_string();
        }
        if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
            return msg.to_string();
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
