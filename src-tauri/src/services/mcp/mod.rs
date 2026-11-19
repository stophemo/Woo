pub mod config;
pub mod convert;
pub mod presence;
pub mod repo;
pub mod security;
pub mod server;
pub mod tools;

use once_cell::sync::Lazy;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};

pub use config::McpView;

static LIFECYCLE: Lazy<tokio::sync::Mutex<()>> = Lazy::new(|| tokio::sync::Mutex::new(()));

/// State shared by every MCP request handler.
#[derive(Clone)]
pub struct McpShared {
    pub token: Arc<RwLock<String>>,
    pub allow_writes: Arc<AtomicBool>,
    pub bound_user: Option<String>,
    pub app_handle: Option<tauri::AppHandle>,
}

/// Business-level tool error. These are returned as MCP tool errors
/// (`isError: true`) so the model can read the message and self-correct.
#[derive(Debug)]
pub enum ToolError {
    NotFound(String),
    Locked(String),
    ReadOnly,
    Conflict {
        current_content_hash: String,
        current_update_time: String,
        current_title: String,
    },
    EditorBusy(String),
    Invalid(String),
    UserChanged(String),
    Internal(String),
}

impl ToolError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "NOT_FOUND",
            Self::Locked(_) => "LOCKED",
            Self::ReadOnly => "READ_ONLY",
            Self::Conflict { .. } => "CONFLICT",
            Self::EditorBusy(_) => "EDITOR_BUSY",
            Self::Invalid(_) => "INVALID_ARGUMENT",
            Self::UserChanged(_) => "USER_CHANGED",
            Self::Internal(_) => "INTERNAL",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::NotFound(message)
            | Self::Locked(message)
            | Self::EditorBusy(message)
            | Self::Invalid(message)
            | Self::UserChanged(message)
            | Self::Internal(message) => message.clone(),
            Self::ReadOnly => {
                "写入未开启：请在 Woo 菜单 MCP → MCP 服务设置… 中启用「允许 AI 修改笔记」。".to_string()
            }
            Self::Conflict { .. } => "文稿已被其他操作修改，请重新读取后再试。".to_string(),
        }
    }

    pub fn to_call_result(&self) -> CallToolResult {
        let details = match self {
            Self::Conflict {
                current_content_hash,
                current_update_time,
                current_title,
            } => json!({
                "currentContentHash": current_content_hash,
                "currentUpdateTime": current_update_time,
                "currentTitle": current_title,
            }),
            _ => json!({}),
        };
        let payload = json!({
            "ok": false,
            "error": {
                "code": self.code(),
                "message": self.message(),
                "details": details,
            }
        });
        CallToolResult::error(vec![ContentBlock::text(payload.to_string())])
    }
}

impl From<String> for ToolError {
    fn from(value: String) -> Self {
        Self::Internal(value)
    }
}

pub fn get_config_view() -> McpView {
    let config = config::load_config();
    let running = server::is_running();
    let port = server::running_port();
    let mut view = config::view(&config, running, port, server::last_error());
    if config.bound_user.as_deref() != crate::db::current_user_key().as_deref() {
        view.active = false;
        if view.last_error.is_none() {
            view.last_error = Some("当前账号与 MCP 配置不匹配，请重新启用。".to_string());
        }
    }
    view
}

pub async fn set_enabled(enabled: bool) -> Result<McpView, String> {
    let _guard = LIFECYCLE.lock().await;
    let mut config = config::load_config();

    if enabled {
        if config.token.is_empty() {
            config::rotate_token(&mut config);
        }
        config.enabled = true;
        config.bound_user = crate::db::current_user_key();
        server::stop_server().await;
        match server::start_server(&config).await {
            Ok((port, _)) => {
                if port != config.port {
                    config.port = port;
                }
                config::save_config(&config)?;
                Ok(get_config_view())
            }
            Err(error) => {
                server::set_last_error(&error);
                config.enabled = false;
                let _ = config::save_config(&config);
                Err(error)
            }
        }
    } else {
        server::stop_server().await;
        config.enabled = false;
        server::clear_last_error();
        config::save_config(&config)?;
        Ok(get_config_view())
    }
}

pub fn set_allow_writes(allow_writes: bool) -> Result<McpView, String> {
    let mut config = config::load_config();
    config.allow_writes = allow_writes;
    config::save_config(&config)?;
    if let Some(shared) = server::current_shared() {
        shared.allow_writes.store(allow_writes, Ordering::Relaxed);
    }
    Ok(get_config_view())
}

pub async fn set_port(port: u16) -> Result<McpView, String> {
    if !(1024..=65535).contains(&port) {
        return Err("端口必须在 1024-65535 之间".to_string());
    }
    let _guard = LIFECYCLE.lock().await;
    let mut config = config::load_config();
    let was_enabled = config.enabled;
    if was_enabled {
        server::stop_server().await;
    }
    config.port = port;
    if was_enabled {
        if config.token.is_empty() {
            config::rotate_token(&mut config);
        }
        config.bound_user = crate::db::current_user_key();
        match server::start_server(&config).await {
            Ok((actual_port, _)) => {
                config.port = actual_port;
                config::save_config(&config)?;
                Ok(get_config_view())
            }
            Err(error) => {
                server::set_last_error(&error);
                config.enabled = false;
                let _ = config::save_config(&config);
                Err(error)
            }
        }
    } else {
        server::clear_last_error();
        config::save_config(&config)?;
        Ok(get_config_view())
    }
}

pub fn regenerate_token() -> Result<McpView, String> {
    let mut config = config::load_config();
    config::rotate_token(&mut config);
    config::save_config(&config)?;
    if let Some(shared) = server::current_shared() {
        *shared
            .token
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = config.token.clone();
    }
    Ok(get_config_view())
}

pub fn set_editor_presence(document_id: Option<String>, dirty: bool) {
    match document_id {
        Some(id) => presence::set(Some(id), dirty),
        None => presence::clear(),
    }
}

pub fn restore_on_startup() {
    let config = config::load_config();
    if !config.enabled {
        return;
    }
    if config.token.is_empty() {
        server::set_last_error("MCP 令牌缺失，请停用后重新启用。");
        return;
    }
    if config.bound_user.as_deref() != crate::db::current_user_key().as_deref() {
        server::set_last_error("当前账号与 MCP 配置不匹配，请重新启用。");
        return;
    }

    tauri::async_runtime::spawn(async move {
        let _guard = LIFECYCLE.lock().await;
        match server::start_server(&config).await {
            Ok((port, _)) => {
                if port != config.port {
                    let mut updated = config.clone();
                    updated.port = port;
                    let _ = config::save_config(&updated);
                }
            }
            Err(error) => {
                log::warn!("[MCP] 启动恢复失败: {error}");
                server::set_last_error(error);
            }
        }
    });
}

pub fn shutdown() {
    server::shutdown();
}
