use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::db;

const CONFIG_FILE: &str = "vault-sync-config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VaultSyncConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub vault_id: String,
    pub device_id: String,
    pub device_token: String,
    pub vault_key: String,
    pub device_name: String,
    /// 配置绑定的数据库账号 key；退出登录后不复用旧账号的同步凭据。
    pub bound_user: Option<String>,
}

impl Default for VaultSyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: String::new(),
            vault_id: String::new(),
            device_id: String::new(),
            device_token: String::new(),
            vault_key: String::new(),
            device_name: default_device_name(),
            bound_user: None,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultSyncConfigView {
    pub enabled: bool,
    pub active: bool,
    pub configured: bool,
    pub endpoint: String,
    pub vault_id: String,
    pub device_id: String,
    pub device_name: String,
}

pub fn default_device_name() -> String {
    #[cfg(target_os = "macos")]
    {
        return "Mac".to_string();
    }
    #[cfg(target_os = "windows")]
    {
        return "Windows".to_string();
    }
    #[cfg(target_os = "linux")]
    {
        return "Linux".to_string();
    }
    #[cfg(target_os = "android")]
    {
        return "Android".to_string();
    }
    #[cfg(target_os = "ios")]
    {
        return "iPhone".to_string();
    }
    #[allow(unreachable_code)]
    "Woo 设备".to_string()
}

pub fn platform_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        return "macos";
    }
    #[cfg(target_os = "windows")]
    {
        return "windows";
    }
    #[cfg(target_os = "linux")]
    {
        return "linux";
    }
    #[cfg(target_os = "android")]
    {
        return "android";
    }
    #[cfg(target_os = "ios")]
    {
        return "ios";
    }
    #[allow(unreachable_code)]
    "linux"
}

fn config_path() -> PathBuf {
    db::data_dir().join(CONFIG_FILE)
}

pub fn load_config() -> VaultSyncConfig {
    std::fs::read_to_string(config_path())
        .ok()
        .and_then(|text| serde_json::from_str::<VaultSyncConfig>(&text).ok())
        .unwrap_or_default()
}

pub fn save_config(config: &VaultSyncConfig) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(config)
        .map_err(|error| format!("保存同步空间配置失败：{error}"))?;
    write_atomic(&config_path(), &bytes)
}

pub fn delete_config() -> Result<(), String> {
    match std::fs::remove_file(config_path()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("删除同步空间配置失败：{error}")),
    }
}

pub fn is_configured(config: &VaultSyncConfig) -> bool {
    !config.endpoint.is_empty()
        && !config.vault_id.is_empty()
        && !config.device_id.is_empty()
        && !config.device_token.is_empty()
        && !config.vault_key.is_empty()
}

pub fn config_is_active(config: &VaultSyncConfig) -> bool {
    config.enabled
        && is_configured(config)
        && config.bound_user == db::current_user_key()
}

pub fn is_enabled() -> bool {
    config_is_active(&load_config())
}

pub fn get_config_view() -> VaultSyncConfigView {
    let config = load_config();
    VaultSyncConfigView {
        enabled: config.enabled,
        active: config_is_active(&config),
        configured: is_configured(&config),
        endpoint: config.endpoint,
        vault_id: config.vault_id,
        device_id: config.device_id,
        device_name: config.device_name,
    }
}

pub fn normalize_endpoint(value: &str) -> Result<String, String> {
    let raw = value.trim().trim_end_matches('/');
    if raw.is_empty() {
        return Err("请填写同步服务地址".to_string());
    }
    let parsed = reqwest::Url::parse(raw)
        .map_err(|_| "同步服务地址无效".to_string())?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("同步服务地址不能包含用户名或密码".to_string());
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err("同步服务地址不能包含查询参数或片段".to_string());
    }
    let host = parsed.host_str().ok_or("同步服务地址缺少主机名")?;
    let https = parsed.scheme() == "https";
    let local_http = parsed.scheme() == "http"
        && (host == "127.0.0.1" || host == "localhost" || host == "[::1]");
    if !https && !local_http {
        return Err("同步服务必须使用 HTTPS（本机调试可使用 http://127.0.0.1）".to_string());
    }
    Ok(raw.to_string())
}

fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension("json.tmp");
    {
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp)
            .map_err(|error| format!("写入同步空间配置失败：{error}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
        }
        file.write_all(bytes)
            .map_err(|error| format!("写入同步空间配置失败：{error}"))?;
        file.sync_all()
            .map_err(|error| format!("写入同步空间配置失败：{error}"))?;
    }
    std::fs::rename(&temp, path)
        .map_err(|error| format!("保存同步空间配置失败：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_requires_https_except_loopback() {
        assert_eq!(
            normalize_endpoint("https://sync.example.com/").unwrap(),
            "https://sync.example.com"
        );
        assert!(normalize_endpoint("http://127.0.0.1:8787").is_ok());
        assert!(normalize_endpoint("http://192.168.1.2:8787").is_err());
        assert!(normalize_endpoint("https://user:pass@example.com").is_err());
        assert!(normalize_endpoint("https://example.com?x=1").is_err());
    }
}
