use axum::Router;
use once_cell::sync::Lazy;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use std::sync::{
    atomic::AtomicBool,
    Arc, Mutex, RwLock,
};
use tokio_util::sync::CancellationToken;

use super::{config::McpConfig, security, tools::WooMcp, McpShared};

struct RunningServer {
    port: u16,
    cancel: CancellationToken,
    handle: tauri::async_runtime::JoinHandle<()>,
    shared: Arc<McpShared>,
}

static SERVER: Lazy<Mutex<Option<RunningServer>>> = Lazy::new(|| Mutex::new(None));
static LAST_ERROR: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

fn lock_server() -> std::sync::MutexGuard<'static, Option<RunningServer>> {
    SERVER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn lock_error() -> std::sync::MutexGuard<'static, Option<String>> {
    LAST_ERROR
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn is_running() -> bool {
    lock_server().is_some()
}

pub fn running_port() -> Option<u16> {
    lock_server().as_ref().map(|server| server.port)
}

pub fn current_shared() -> Option<Arc<McpShared>> {
    lock_server()
        .as_ref()
        .map(|server| server.shared.clone())
}

pub fn last_error() -> Option<String> {
    lock_error().clone()
}

pub fn clear_last_error() {
    *lock_error() = None;
}

pub fn set_last_error(error: impl Into<String>) {
    *lock_error() = Some(error.into());
}

fn bind_listener(preferred: u16) -> Result<(std::net::TcpListener, u16), String> {
    let preferred = if (1024..=65535).contains(&preferred) {
        preferred
    } else {
        super::config::DEFAULT_PORT
    };
    let mut last_error = String::new();
    for offset in 0..super::config::PORT_FALLBACK_COUNT {
        let port = preferred.saturating_add(offset);
        if port == 0 {
            break;
        }
        match std::net::TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => {
                listener
                    .set_nonblocking(true)
                    .map_err(|error| format!("设置监听端口失败: {error}"))?;
                return Ok((listener, port));
            }
            Err(error) => {
                last_error = format!("127.0.0.1:{port}: {error}");
            }
        }
    }
    Err(format!("MCP 端口绑定失败: {last_error}"))
}

pub async fn start_server(config: &McpConfig) -> Result<(u16, Arc<McpShared>), String> {
    if config.token.trim().is_empty() {
        return Err("MCP 令牌为空，请重新启用。".to_string());
    }
    if config.bound_user.as_deref() != crate::db::current_user_key().as_deref() {
        return Err("当前账号与 MCP 配置不匹配，请重新启用。".to_string());
    }

    let (std_listener, port) = bind_listener(config.port)?;
    let listener =
        tokio::net::TcpListener::from_std(std_listener).map_err(|error| error.to_string())?;
    let cancel = CancellationToken::new();
    let shared = Arc::new(McpShared {
        token: Arc::new(RwLock::new(config.token.clone())),
        allow_writes: Arc::new(AtomicBool::new(config.allow_writes)),
        bound_user: config.bound_user.clone(),
        app_handle: crate::get_app_handle(),
    });

    let service = StreamableHttpService::new(
        {
            let shared = shared.clone();
            move || Ok(WooMcp::new(shared.clone()))
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true)
            .with_allowed_origins([
                format!("http://127.0.0.1:{port}"),
                format!("http://localhost:{port}"),
            ])
            .with_max_request_body_bytes(8 * 1024 * 1024)
            .with_cancellation_token(cancel.child_token()),
    );

    let router = Router::new()
        .nest_service("/mcp", service)
        .layer(axum::middleware::from_fn_with_state(
            shared.clone(),
            security::auth_middleware,
        ));

    let handle = tauri::async_runtime::spawn({
        let cancel = cancel.clone();
        async move {
            if let Err(error) = axum::serve(listener, router)
                .with_graceful_shutdown(async move { cancel.cancelled_owned().await })
                .await
            {
                log::warn!("[MCP] HTTP 服务退出: {error}");
            }
        }
    });

    {
        let mut guard = lock_server();
        guard.replace(RunningServer {
            port,
            cancel,
            handle,
            shared: shared.clone(),
        });
    }
    clear_last_error();
    log::info!("[MCP] 服务已启动: http://127.0.0.1:{port}/mcp");
    Ok((port, shared))
}

pub async fn stop_server() {
    let running = { lock_server().take() };
    if let Some(running) = running {
        running.cancel.cancel();
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            running.handle,
        )
        .await;
        log::info!("[MCP] 服务已停止");
    }
}

/// Best-effort shutdown for app exit paths that are not async.
pub fn shutdown() {
    let running = { lock_server().take() };
    if let Some(running) = running {
        running.cancel.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{document_service, folder_service};
    use serde_json::{json, Value};

    fn free_port() -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    }

    fn response_json(body: &str) -> Value {
        for line in body.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if let Ok(value) = serde_json::from_str(data) {
                    return value;
                }
            }
        }
        serde_json::from_str(body).unwrap_or_else(|error| {
            panic!("无法解析 MCP 响应: {error}\n{body}");
        })
    }

    async fn rpc(
        client: &reqwest::Client,
        url: &str,
        token: &str,
        id: u64,
        method: &str,
        params: Value,
    ) -> Value {
        let mut request = client
            .post(url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");
        if method != "initialize" {
            request = request.header("MCP-Protocol-Version", "2025-11-25");
        }
        let response = request
            .json(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params,
            }))
            .send()
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "HTTP {} for {method}",
            response.status()
        );
        response_json(&response.text().await.unwrap())
    }

    // 该测试会修改全局数据目录，和 webdav/lan 的端到端测试一样需要串行执行；
    // 默认 ignore，验证时用 `cargo test mcp_smoke -- --ignored --test-threads=1`。
    #[ignore = "uses the global app data dir; run explicitly with --ignored --test-threads=1"]
    #[tokio::test(flavor = "multi_thread")]
    async fn mcp_smoke_read_write_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "woo-mcp-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        crate::db::set_data_dir(dir.clone());
        crate::db::with_db(|_| {});

        let folder_id = folder_service::create_folder("MCP Test", None).unwrap();
        let doc = document_service::create(
            &folder_id,
            "Hello",
            Some("<p>Hello world</p>"),
        )
        .unwrap();

        let port = free_port();
        let token = "test-token";
        let config = McpConfig {
            enabled: true,
            port,
            token: token.to_string(),
            allow_writes: false,
            bound_user: None,
            token_rotated_at: None,
        };
        crate::services::mcp::config::save_config(&config).unwrap();
        let (actual_port, _shared) = start_server(&config).await.unwrap();
        let url = format!("http://127.0.0.1:{actual_port}/mcp");
        let client = reqwest::Client::new();

        let init = rpc(
            &client,
            &url,
            token,
            1,
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "woo-test", "version": "0" }
            }),
        )
        .await;
        assert_eq!(init["result"]["serverInfo"]["name"], "woo-notes");

        let tools = rpc(&client, &url, token, 2, "tools/list", json!({})).await;
        let names: Vec<String> = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap().to_string())
            .collect();
        assert!(names.contains(&"list_folders".to_string()));
        assert!(names.contains(&"create_note".to_string()));

        let folders = rpc(
            &client,
            &url,
            token,
            3,
            "tools/call",
            json!({ "name": "list_folders", "arguments": {} }),
        )
        .await;
        let folders_text = folders["result"]["content"][0]["text"].as_str().unwrap();
        assert!(folders_text.contains("MCP Test"));

        let note = rpc(
            &client,
            &url,
            token,
            4,
            "tools/call",
            json!({ "name": "get_note", "arguments": { "id": doc.id } }),
        )
        .await;
        let note_payload: Value =
            serde_json::from_str(note["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(note_payload["data"]["title"], "Hello");
        assert!(note_payload["data"]["content"].as_str().unwrap().contains("Hello world"));
        let content_hash = note_payload["data"]["contentHash"].as_str().unwrap().to_string();

        // Read-only mode rejects writes.
        let rejected = rpc(
            &client,
            &url,
            token,
            5,
            "tools/call",
            json!({
                "name": "create_note",
                "arguments": { "folderId": folder_id, "content": "blocked" }
            }),
        )
        .await;
        assert_eq!(rejected["result"]["isError"], true);
        assert!(rejected["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("READ_ONLY"));

        crate::services::mcp::set_allow_writes(true).unwrap();

        // Create a note through MCP.
        let created = rpc(
            &client,
            &url,
            token,
            6,
            "tools/call",
            json!({
                "name": "create_note",
                "arguments": {
                    "folderId": folder_id,
                    "title": "AI note",
                    "content": "## AI note\n\nFrom MCP"
                }
            }),
        )
        .await;
        let created_text = created["result"]["content"][0]["text"].as_str().unwrap();
        let created_payload: Value = serde_json::from_str(created_text).unwrap();
        assert_eq!(created_payload["ok"], true);
        let created_id = created_payload["data"]["id"].as_str().unwrap().to_string();

        // A stale hash must conflict.
        let conflict = rpc(
            &client,
            &url,
            token,
            7,
            "tools/call",
            json!({
                "name": "update_note",
                "arguments": {
                    "id": created_id,
                    "content": "changed",
                    "expectedContentHash": "stale"
                }
            }),
        )
        .await;
        assert_eq!(conflict["result"]["isError"], true);
        assert!(conflict["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("CONFLICT"));

        // Update with the current hash succeeds.
        let current = rpc(
            &client,
            &url,
            token,
            8,
            "tools/call",
            json!({ "name": "get_note", "arguments": { "id": created_id } }),
        )
        .await;
        let current_payload: Value =
            serde_json::from_str(current["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        let current_hash = current_payload["data"]["contentHash"].as_str().unwrap().to_string();
        let updated = rpc(
            &client,
            &url,
            token,
            9,
            "tools/call",
            json!({
                "name": "update_note",
                "arguments": {
                    "id": created_id,
                    "content": "changed body",
                    "expectedContentHash": current_hash
                }
            }),
        )
        .await;
        assert_eq!(updated["result"]["isError"], false);

        let updated_payload: Value =
            serde_json::from_str(updated["result"]["content"][0]["text"].as_str().unwrap())
                .unwrap();
        let updated_hash = updated_payload["data"]["contentHash"]
            .as_str()
            .unwrap()
            .to_string();

        // List and search the new note.
        let listed = rpc(
            &client,
            &url,
            token,
            10,
            "tools/call",
            json!({ "name": "list_notes", "arguments": {} }),
        )
        .await;
        assert!(listed["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("AI note"));
        let searched = rpc(
            &client,
            &url,
            token,
            11,
            "tools/call",
            json!({
                "name": "search_notes",
                "arguments": { "query": "changed" }
            }),
        )
        .await;
        let searched_text = searched["result"]["content"][0]["text"].as_str().unwrap();
        assert!(searched_text.contains("AI note"));

        // Append with the current hash.
        let appended = rpc(
            &client,
            &url,
            token,
            12,
            "tools/call",
            json!({
                "name": "append_note",
                "arguments": {
                    "id": created_id,
                    "content": "appended paragraph",
                    "expectedContentHash": updated_hash
                }
            }),
        )
        .await;
        assert_eq!(appended["result"]["isError"], false);

        // Create a folder and move the note into it.
        let new_folder = rpc(
            &client,
            &url,
            token,
            13,
            "tools/call",
            json!({
                "name": "create_folder",
                "arguments": { "name": "MCP destination" }
            }),
        )
        .await;
        let folder_payload: Value = serde_json::from_str(
            new_folder["result"]["content"][0]["text"].as_str().unwrap(),
        )
        .unwrap();
        let destination_id = folder_payload["data"]["id"].as_str().unwrap().to_string();
        let moved = rpc(
            &client,
            &url,
            token,
            14,
            "tools/call",
            json!({
                "name": "move_note",
                "arguments": { "id": created_id, "folderId": destination_id }
            }),
        )
        .await;
        assert_eq!(moved["result"]["isError"], false);

        // Locked folders are invisible to reads and writes.
        let locked_folder = folder_service::create_folder("Locked folder", None).unwrap();
        let locked_doc = document_service::create(
            &locked_folder,
            "Secret",
            Some("<p>Secret body</p>"),
        )
        .unwrap();
        crate::services::lock_service::lock_folder(&locked_folder).unwrap();
        let listed_locked = rpc(
            &client,
            &url,
            token,
            15,
            "tools/call",
            json!({ "name": "list_notes", "arguments": {} }),
        )
        .await;
        assert!(!listed_locked["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Secret"));
        let locked_get = rpc(
            &client,
            &url,
            token,
            16,
            "tools/call",
            json!({ "name": "get_note", "arguments": { "id": locked_doc.id } }),
        )
        .await;
        assert_eq!(locked_get["result"]["isError"], true);
        assert!(locked_get["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("LOCKED"));

        // A dirty editor blocks writes to the open note.
        let current = rpc(
            &client,
            &url,
            token,
            17,
            "tools/call",
            json!({ "name": "get_note", "arguments": { "id": created_id } }),
        )
        .await;
        let current_payload: Value =
            serde_json::from_str(current["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        let current_hash = current_payload["data"]["contentHash"]
            .as_str()
            .unwrap()
            .to_string();
        crate::services::mcp::set_editor_presence(Some(created_id.clone()), true);
        let busy = rpc(
            &client,
            &url,
            token,
            18,
            "tools/call",
            json!({
                "name": "update_note",
                "arguments": {
                    "id": created_id,
                    "content": "should not apply",
                    "expectedContentHash": current_hash
                }
            }),
        )
        .await;
        assert_eq!(busy["result"]["isError"], true);
        assert!(busy["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("EDITOR_BUSY"));
        crate::services::mcp::set_editor_presence(None, false);

        // Soft delete and restore.
        let deleted = rpc(
            &client,
            &url,
            token,
            19,
            "tools/call",
            json!({
                "name": "delete_note",
                "arguments": { "id": created_id, "confirm": true }
            }),
        )
        .await;
        assert_eq!(deleted["result"]["isError"], false);
        let trash = rpc(
            &client,
            &url,
            token,
            20,
            "tools/call",
            json!({ "name": "list_trash", "arguments": {} }),
        )
        .await;
        assert!(trash["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("AI note"));
        let restored = rpc(
            &client,
            &url,
            token,
            21,
            "tools/call",
            json!({ "name": "restore_note", "arguments": { "id": created_id } }),
        )
        .await;
        assert_eq!(restored["result"]["isError"], false);

        // The expected content hash was updated by the successful update.
        let _ = content_hash;

        // Toggle through the public lifecycle API as the settings page does.
        let disabled = crate::services::mcp::set_enabled(false).await.unwrap();
        assert!(!disabled.running);
        let enabled = crate::services::mcp::set_enabled(true).await.unwrap();
        assert!(enabled.running);

        // Token rotation takes effect without a restart.
        let rotated = crate::services::mcp::regenerate_token().unwrap();
        assert_ne!(rotated.token, token);
        let stale = client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 99,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-11-25",
                    "capabilities": {},
                    "clientInfo": { "name": "woo-test", "version": "0" }
                }
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(stale.status(), reqwest::StatusCode::UNAUTHORIZED);
        let init_after_rotation = rpc(
            &client,
            &url,
            &rotated.token,
            100,
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "woo-test", "version": "0" }
            }),
        )
        .await;
        assert_eq!(init_after_rotation["result"]["serverInfo"]["name"], "woo-notes");

        stop_server().await;
        crate::db::close_db();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
