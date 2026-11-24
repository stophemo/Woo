//! WebDAV 账号：地址、用户名、应用密码。
//!
//! 账号与应用密码单独加密保存在本机，不写入日志、不做明文备份。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::db;
use crate::services::webdav_sync;

use super::secure;

const ACCOUNT_FILE: &str = "webdav-account.enc.json";
const MIGRATION_MARKER: &str = "webdav_account_migrated_v1";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WebDavAccount {
    pub endpoint: String,
    pub username: String,
    pub app_password: String,
    /// 账号配置绑定的数据库账号；切换本地用户后不复用。
    pub bound_user: Option<String>,
}

impl Default for WebDavAccount {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            username: String::new(),
            app_password: String::new(),
            bound_user: None,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavAccountView {
    pub configured: bool,
    pub active: bool,
    pub endpoint: String,
    pub username: String,
    pub has_password: bool,
}

pub fn account_path() -> PathBuf {
    db::data_dir().join(ACCOUNT_FILE)
}

pub fn normalize_endpoint(value: &str) -> Result<String, String> {
    normalize_endpoint_with_options(value, false)
}

pub(crate) fn normalize_endpoint_with_options(
    value: &str,
    allow_loopback_http: bool,
) -> Result<String, String> {
    let raw = value.trim().trim_end_matches('/');
    if raw.is_empty() {
        return Err("请填写 WebDAV 地址".to_string());
    }
    let parsed = reqwest::Url::parse(raw).map_err(|_| "WebDAV 地址格式不正确".to_string())?;
    let host = parsed.host_str().ok_or("WebDAV 地址缺少主机名")?;
    let loopback = host == "127.0.0.1" || host == "localhost" || host == "[::1]";
    if parsed.scheme() != "https" && !(allow_loopback_http && parsed.scheme() == "http" && loopback) {
        return Err("WebDAV 地址必须使用 HTTPS".to_string());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("WebDAV 地址不能包含用户名或密码，请分别填写".to_string());
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err("WebDAV 地址不能带查询参数或片段".to_string());
    }
    Ok(raw.to_string())
}

pub fn load_account() -> Result<Option<WebDavAccount>, String> {
    let Some(bytes) = secure::open_file(&account_path(), "account")? else {
        return Ok(None);
    };
    let account: WebDavAccount = serde_json::from_slice(&bytes)
        .map_err(|_| "WebDAV 账号配置格式损坏".to_string())?;
    Ok(Some(account))
}

pub fn configured_account() -> Result<Option<WebDavAccount>, String> {
    let account = load_account()?;
    Ok(account.filter(account_is_active))
}

pub fn account_is_active(account: &WebDavAccount) -> bool {
    account.bound_user == db::current_user_key()
        && !account.endpoint.is_empty()
        && !account.username.is_empty()
        && !account.app_password.is_empty()
}

pub fn account_view(account: Option<&WebDavAccount>) -> WebDavAccountView {
    match account {
        Some(account) => WebDavAccountView {
            configured: !account.endpoint.is_empty()
                && !account.username.is_empty()
                && !account.app_password.is_empty(),
            active: account_is_active(account),
            endpoint: account.endpoint.clone(),
            username: account.username.clone(),
            has_password: !account.app_password.is_empty(),
        },
        None => WebDavAccountView {
            configured: false,
            active: false,
            endpoint: String::new(),
            username: String::new(),
            has_password: false,
        },
    }
}

pub fn save_account(
    endpoint: &str,
    username: &str,
    password: Option<String>,
) -> Result<WebDavAccountView, String> {
    let endpoint = normalize_endpoint(endpoint)?;
    let username = username.trim().to_string();
    if username.is_empty() {
        return Err("请填写 WebDAV 用户名".to_string());
    }
    let existing = load_account()?;
    let can_reuse_password = existing.as_ref().is_some_and(|account| {
        account.bound_user == db::current_user_key()
            && account.endpoint == endpoint
            && account.username == username
    });
    let app_password = match password {
        Some(value) if !value.is_empty() => value,
        _ if can_reuse_password => existing
            .as_ref()
            .map(|account| account.app_password.clone())
            .unwrap_or_default(),
        _ => String::new(),
    };
    if app_password.is_empty() {
        return Err("请填写 WebDAV 应用密码".to_string());
    }
    if app_password.chars().any(char::is_control) {
        return Err("WebDAV 应用密码包含无效字符".to_string());
    }

    let account = WebDavAccount {
        endpoint,
        username,
        app_password,
        bound_user: db::current_user_key(),
    };
    let bytes = serde_json::to_vec(&account)
        .map_err(|error| format!("序列化 WebDAV 账号失败：{error}"))?;
    secure::save_file(&account_path(), "account", &bytes)?;
    log::info!(
        "[WebDAV] 账号已保存 (endpoint={}, username={})",
        account.endpoint,
        account.username
    );
    Ok(account_view(Some(&account)))
}

/// 把旧版 `webdav-config.json` 转成新账号配置。
///
/// 只迁移地址/用户名/应用密码；旧远端目录里的 manifest 数据不复用。
/// 成功后删除旧配置文件，避免清除本机数据后又被旧配置回填。
fn account_from_legacy(
    legacy: &webdav_sync::WebdavConfig,
) -> Result<Option<WebDavAccount>, String> {
    if legacy.url.trim().is_empty()
        || legacy.username.trim().is_empty()
        || legacy.password.is_empty()
        || (db::current_user_key().is_some() && legacy.bound_user != db::current_user_key())
    {
        return Ok(None);
    }
    let endpoint = normalize_endpoint(&legacy.url)?;
    Ok(Some(WebDavAccount {
        endpoint,
        username: legacy.username.trim().to_string(),
        app_password: legacy.password.clone(),
        bound_user: db::current_user_key(),
    }))
}

pub fn migrate_legacy_account() -> Result<bool, String> {
    if account_path().exists() {
        return Ok(false);
    }
    let legacy = webdav_sync::load_config();
    let Some(account) = account_from_legacy(&legacy)? else {
        return Ok(false);
    };
    let bytes = serde_json::to_vec(&account)
        .map_err(|error| format!("序列化迁移账号失败：{error}"))?;
    secure::save_file(&account_path(), "account", &bytes)?;

    let legacy_path = db::data_dir().join("webdav-config.json");
    match std::fs::remove_file(&legacy_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => log::warn!("[WebDAV] 删除旧配置失败：{}", error),
    }
    let _ = db::with_db(|conn| {
        conn.execute(
            "INSERT OR REPLACE INTO sync_meta(key, value) VALUES (?1, ?2)",
            rusqlite::params![MIGRATION_MARKER, "1"],
        )
    });
    log::info!("[WebDAV] 已迁移旧版 WebDAV 账号（不包含旧远端清单）");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_requires_https() {
        assert_eq!(
            normalize_endpoint("https://dav.example.com/").unwrap(),
            "https://dav.example.com"
        );
        assert!(normalize_endpoint("http://dav.example.com").is_err());
        assert!(normalize_endpoint("http://127.0.0.1:8080").is_err());
        assert!(normalize_endpoint("https://user:pass@example.com").is_err());
        assert!(normalize_endpoint("https://example.com?x=1").is_err());
    }

    #[test]
    fn legacy_config_maps_to_account_without_losing_password_rules() {
        let legacy = webdav_sync::WebdavConfig {
            enabled: true,
            url: "https://dav.example.com/dav/".to_string(),
            username: "user".to_string(),
            password: "app-password".to_string(),
            folder: "Woo".to_string(),
            bound_user: None,
        };
        let account = account_from_legacy(&legacy).unwrap().unwrap();
        assert_eq!(account.endpoint, "https://dav.example.com/dav");
        assert_eq!(account.username, "user");
        assert_eq!(account.app_password, "app-password");

        let mut incomplete = legacy.clone();
        incomplete.password.clear();
        assert!(account_from_legacy(&incomplete).unwrap().is_none());
    }

    #[test]
    fn loopback_http_is_only_allowed_for_client_tests() {
        assert_eq!(
            normalize_endpoint_with_options("http://127.0.0.1:8080/", true).unwrap(),
            "http://127.0.0.1:8080"
        );
        assert!(normalize_endpoint_with_options("http://192.168.1.2:8080", true).is_err());
    }
}
