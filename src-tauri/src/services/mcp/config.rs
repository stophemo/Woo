use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::db;
use crate::services::utils::now_str;
use crate::services::webdav_sync;

pub const DEFAULT_PORT: u16 = 47823;
pub const PORT_FALLBACK_COUNT: u16 = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpConfig {
    pub enabled: bool,
    pub port: u16,
    pub token: String,
    pub allow_writes: bool,
    pub bound_user: Option<String>,
    pub token_rotated_at: Option<String>,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
            token: String::new(),
            allow_writes: false,
            bound_user: None,
            token_rotated_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpView {
    pub enabled: bool,
    pub running: bool,
    pub active: bool,
    pub port: u16,
    pub url: Option<String>,
    pub token: String,
    pub allow_writes: bool,
    pub bound_user: Option<String>,
    pub token_rotated_at: Option<String>,
    pub last_error: Option<String>,
}

pub fn config_path() -> PathBuf {
    db::data_dir().join("mcp-config.json")
}

pub fn load_config() -> McpConfig {
    match std::fs::read(config_path()) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
            log::warn!("[MCP] 配置文件解析失败，使用默认配置: {error}");
            McpConfig::default()
        }),
        Err(_) => McpConfig::default(),
    }
}

pub fn save_config(config: &McpConfig) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(config).map_err(|error| error.to_string())?;
    webdav_sync::write_atomic(&config_path(), &bytes)
}

pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn rotate_token(config: &mut McpConfig) {
    config.token = new_token();
    config.token_rotated_at = Some(now_str());
}

pub fn view(
    config: &McpConfig,
    running: bool,
    actual_port: Option<u16>,
    last_error: Option<String>,
) -> McpView {
    let port = actual_port.unwrap_or(config.port);
    let active = running
        && config.enabled
        && config.bound_user.as_deref() == db::current_user_key().as_deref();
    McpView {
        enabled: config.enabled,
        running,
        active,
        port,
        url: running.then(|| format!("http://127.0.0.1:{port}/mcp")),
        token: config.token.clone(),
        allow_writes: config.allow_writes,
        bound_user: config.bound_user.clone(),
        token_rotated_at: config.token_rotated_at.clone(),
        last_error,
    }
}
