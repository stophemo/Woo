//! 多设备同步空间：六位配对码、端到端加密增量操作日志、分块正文。

pub mod client;
pub mod config;
pub mod store;

use std::sync::Mutex;

use once_cell::sync::Lazy;
use serde::Serialize;
use woo_sync_core::crypto::{
    base64url_decode, base64url_encode, open_pairing_vault_key, pairing_verification_code,
    seal_pairing_vault_key, PairingKeyPair,
};
use woo_sync_core::SyncRequest;

use crate::db;
use crate::services::sync_engine::{SyncResult, SyncStatus};
use crate::services::utils::now_str;

use self::client::{DeviceRegistration, WorkerClient};
use self::config::{
    config_is_active, default_device_name, delete_config, is_configured, load_config,
    normalize_endpoint, platform_name, save_config, VaultSyncConfig, VaultSyncConfigView,
};
use self::store::SyncIdentity;

// 单条密文最大 32 KiB，base64 后约 43.7 KiB；30 条约 1.3 MiB，
// 低于 Worker 2 MiB 请求体上限，避免满批 manifest 触发 413。
const PUSH_BATCH: usize = 30;
const PREPARE_BATCH: usize = 100;

enum PendingPairing {
    Initiator {
        endpoint: String,
        pairing_id: String,
        pairing_secret: String,
        keypair: PairingKeyPair,
        expires_at: i64,
    },
    Joiner {
        endpoint: String,
        pairing_id: String,
        pairing_secret: String,
        keypair: PairingKeyPair,
        device_token: String,
        device_name: String,
        vault_id: String,
        expires_at: i64,
    },
}

static PAIRING: Lazy<Mutex<Option<PendingPairing>>> = Lazy::new(|| Mutex::new(None));

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingCodeView {
    pub pairing_id: String,
    pub short_code: String,
    pub expires_at: i64,
    pub server_time: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimedDeviceView {
    pub device_id: String,
    pub name: String,
    pub platform: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingStatusView {
    pub pairing_id: String,
    pub status: String,
    pub expires_at: i64,
    pub claimed_device: Option<ClaimedDeviceView>,
    pub verification_code: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinStartView {
    pub pairing_id: String,
    pub device_id: String,
    pub expires_at: i64,
    pub verification_code: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinPollView {
    pub status: String,
    pub configured: bool,
    pub vault_id: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub created_at: i64,
    pub last_seen_at: Option<i64>,
    pub is_current: bool,
}

pub fn is_enabled() -> bool {
    config_is_active(&load_config())
}

pub fn get_config_view() -> VaultSyncConfigView {
    config::get_config_view()
}

pub fn active_config() -> Option<VaultSyncConfig> {
    let config = load_config();
    if config_is_active(&config) {
        Some(config)
    } else {
        None
    }
}

fn decode_vault_key(config: &VaultSyncConfig) -> Result<[u8; 32], String> {
    let bytes = base64url_decode(&config.vault_key)?;
    <[u8; 32]>::try_from(bytes.as_slice())
        .map_err(|_| "同步空间密钥长度无效，请重新配对".to_string())
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub async fn test_connection(endpoint: &str) -> Result<String, String> {
    let endpoint = normalize_endpoint(endpoint)?;
    let client = WorkerClient::new(&endpoint, None)?;
    client
        .health()
        .await
        .map(|version| format!("连接成功：{version}"))
        .map_err(|error| error.to_string())
}

pub async fn create_vault(
    endpoint: &str,
    invite_code: &str,
    device_name: &str,
) -> Result<VaultSyncConfigView, String> {
    let endpoint = normalize_endpoint(endpoint)?;
    let invite_code = invite_code.trim();
    if invite_code.is_empty() {
        return Err("请填写部署同步服务时设置的创建邀请码".to_string());
    }
    let device_name = sanitize_device_name(device_name);
    let existing = load_config();
    if is_configured(&existing) && existing.enabled {
        return Err("当前设备已经加入同步空间，请先离开再创建新的空间".to_string());
    }
    let client = WorkerClient::new(&endpoint, None)?;
    client
        .health()
        .await
        .map_err(|error| format!("同步服务不可用：{error}"))?;
    let vault_key = woo_sync_core::crypto::random_vault_key()?;
    let created = client
        .create_vault(
            invite_code,
            &DeviceRegistration {
                name: device_name.clone(),
                platform: platform_name().to_string(),
                public_key: None,
            },
        )
        .await
        .map_err(|error| error.to_string())?;

    let config = VaultSyncConfig {
        enabled: true,
        endpoint,
        vault_id: created.vault_id,
        device_id: created.device.id,
        device_token: created.device.token,
        vault_key: base64url_encode(&vault_key),
        device_name,
        bound_user: db::current_user_key(),
    };
    save_config(&config)?;
    disable_legacy_sync();
    prepare_new_vault_binding(&config, true)?;
    log::info!(
        "[VaultSync] 已创建同步空间 {}（设备 {}）",
        config.vault_id,
        config.device_id
    );
    Ok(config::get_config_view())
}

pub async fn start_pairing_code() -> Result<PairingCodeView, String> {
    let config = active_config().ok_or_else(|| "请先启用同步空间".to_string())?;
    let client = WorkerClient::new(&config.endpoint, Some(&config.device_token))?;
    let keypair = PairingKeyPair::generate()?;
    let data = client
        .create_pairing_code(&keypair.public_key_base64url())
        .await
        .map_err(|error| error.to_string())?;
    *PAIRING.lock().map_err(|_| "配对状态锁已损坏".to_string())? =
        Some(PendingPairing::Initiator {
            endpoint: config.endpoint,
            pairing_id: data.pairing_id.clone(),
            pairing_secret: data.pairing_secret,
            keypair,
            expires_at: data.expires_at,
        });
    Ok(PairingCodeView {
        pairing_id: data.pairing_id,
        short_code: data.short_code,
        expires_at: data.expires_at,
        server_time: data.server_time,
    })
}

pub async fn pairing_status(pairing_id: &str) -> Result<PairingStatusView, String> {
    let (endpoint, pairing_secret, keypair) = {
        let guard = PAIRING.lock().map_err(|_| "配对状态锁已损坏".to_string())?;
        match guard.as_ref() {
            Some(PendingPairing::Initiator {
                endpoint,
                pairing_id: stored_id,
                pairing_secret,
                keypair,
                expires_at,
            }) if stored_id == pairing_id => {
                if now_ms() > *expires_at {
                    return Err("配对码已过期，请重新生成".to_string());
                }
                (
                    endpoint.clone(),
                    pairing_secret.clone(),
                    PairingKeyPair::from_private_key(keypair_private_bytes(keypair)),
                )
            }
            _ => return Err("配对会话不存在，请重新生成配对码".to_string()),
        }
    };
    let config = active_config().ok_or_else(|| "请先启用同步空间".to_string())?;
    let client = WorkerClient::new(&endpoint, Some(&config.device_token))?;
    let status = client
        .pairing_status(pairing_id)
        .await
        .map_err(|error| error.to_string())?;
    if status.status == "expired" || status.status == "canceled" {
        let _ = PAIRING.lock().map(|mut guard| *guard = None);
    }
    let mut claimed_device = None;
    let mut verification_code = None;
    if let Some(claim) = &status.claim {
        let secret = base64url_decode(&pairing_secret)?;
        let claimed_public = base64url_decode(&claim.public_key)?;
        let session_key =
            keypair.session_key(&secret, pairing_id, &claimed_public)?;
        verification_code = Some(pairing_verification_code(
            &session_key,
            &keypair.public_key_bytes(),
            &claimed_public,
        ));
        claimed_device = Some(ClaimedDeviceView {
            device_id: claim.device_id.clone(),
            name: claim.name.clone(),
            platform: claim.platform.clone(),
        });
    }
    Ok(PairingStatusView {
        pairing_id: status.pairing_id,
        status: status.status,
        expires_at: status.expires_at,
        claimed_device,
        verification_code,
    })
}

pub async fn confirm_pairing(pairing_id: &str) -> Result<(), String> {
    let (endpoint, pairing_secret, keypair) = {
        let guard = PAIRING.lock().map_err(|_| "配对状态锁已损坏".to_string())?;
        match guard.as_ref() {
            Some(PendingPairing::Initiator {
                endpoint,
                pairing_id: stored_id,
                pairing_secret,
                keypair,
                ..
            }) if stored_id == pairing_id => (
                endpoint.clone(),
                pairing_secret.clone(),
                PairingKeyPair::from_private_key(keypair_private_bytes(keypair)),
            ),
            _ => return Err("配对会话不存在，请重新生成配对码".to_string()),
        }
    };
    let config = active_config().ok_or_else(|| "请先启用同步空间".to_string())?;
    let client = WorkerClient::new(&endpoint, Some(&config.device_token))?;
    let status = client
        .pairing_status(pairing_id)
        .await
        .map_err(|error| error.to_string())?;
    let claim = status
        .claim
        .ok_or_else(|| "新设备还没有认领配对码".to_string())?;
    let claimed_public = base64url_decode(&claim.public_key)?;
    let secret = base64url_decode(&pairing_secret)?;
    let session_key = keypair.session_key(&secret, pairing_id, &claimed_public)?;
    let vault_key = decode_vault_key(&config)?;
    let envelope = seal_pairing_vault_key(
        &vault_key,
        &session_key,
        pairing_id,
        &claim.device_id,
    )?;
    client
        .confirm_pairing(pairing_id, &envelope)
        .await
        .map_err(|error| error.to_string())?;
    let _ = PAIRING.lock().map(|mut guard| *guard = None);
    log::info!("[VaultSync] 已确认新设备 {} 加入", claim.device_id);
    Ok(())
}

pub async fn begin_join(
    endpoint: &str,
    code: &str,
    device_name: &str,
) -> Result<JoinStartView, String> {
    let endpoint = normalize_endpoint(endpoint)?;
    let code = code.trim();
    if !(code.len() == 6 && code.chars().all(|c| c.is_ascii_digit())) {
        return Err("请输入六位数字配对码".to_string());
    }
    let existing = load_config();
    if is_configured(&existing) && existing.enabled {
        return Err("当前设备已经加入同步空间，请先离开再加入新的空间".to_string());
    }
    let device_name = sanitize_device_name(device_name);
    let client = WorkerClient::new(&endpoint, None)?;
    let resolved = client
        .resolve_pairing_code(code)
        .await
        .map_err(|error| error.to_string())?;
    if now_ms() > resolved.expires_at {
        return Err("配对码已过期，请在已有设备上重新生成".to_string());
    }
    let keypair = PairingKeyPair::generate()?;
    let device_token = woo_sync_core::crypto::random_device_token()?;
    let initiator_public = base64url_decode(&resolved.initiator_public_key)?;
    let secret = base64url_decode(&resolved.pairing_secret)?;
    let session_key = keypair.session_key(&secret, &resolved.pairing_id, &initiator_public)?;
    let verification_code = pairing_verification_code(
        &session_key,
        &initiator_public,
        &keypair.public_key_bytes(),
    );
    let claimed = client
        .claim_pairing(
            &resolved.pairing_id,
            &resolved.pairing_secret,
            &device_token,
            &DeviceRegistration {
                name: device_name.clone(),
                platform: platform_name().to_string(),
                public_key: Some(keypair.public_key_base64url()),
            },
        )
        .await
        .map_err(|error| error.to_string())?;
    *PAIRING.lock().map_err(|_| "配对状态锁已损坏".to_string())? =
        Some(PendingPairing::Joiner {
            endpoint,
            pairing_id: resolved.pairing_id.clone(),
            pairing_secret: resolved.pairing_secret,
            keypair,
            device_token,
            device_name,
            vault_id: resolved.vault_id,
            expires_at: resolved.expires_at,
        });
    Ok(JoinStartView {
        pairing_id: claimed.pairing_id,
        device_id: claimed.device_id,
        expires_at: resolved.expires_at,
        verification_code,
    })
}

pub async fn poll_join(pairing_id: &str) -> Result<JoinPollView, String> {
    let (endpoint, pairing_secret, keypair, device_token, device_name, expected_vault, expires_at) = {
        let guard = PAIRING.lock().map_err(|_| "配对状态锁已损坏".to_string())?;
        match guard.as_ref() {
            Some(PendingPairing::Joiner {
                endpoint,
                pairing_id: stored_id,
                pairing_secret,
                keypair,
                device_token,
                device_name,
                vault_id,
                expires_at,
            }) if stored_id == pairing_id => (
                endpoint.clone(),
                pairing_secret.clone(),
                PairingKeyPair::from_private_key(keypair_private_bytes(keypair)),
                device_token.clone(),
                device_name.clone(),
                vault_id.clone(),
                *expires_at,
            ),
            _ => return Err("配对会话不存在，请重新输入配对码".to_string()),
        }
    };
    if now_ms() > expires_at {
        let _ = PAIRING.lock().map(|mut guard| *guard = None);
        return Ok(JoinPollView {
            status: "expired".to_string(),
            configured: false,
            vault_id: None,
            message: Some("配对码已过期，请在已有设备上重新生成".to_string()),
        });
    }
    let client = WorkerClient::new(&endpoint, None)?;
    let result = client
        .pairing_result(pairing_id, &pairing_secret, &device_token)
        .await
        .map_err(|error| error.to_string())?;
    if result.status != "confirmed" {
        return Ok(JoinPollView {
            status: result.status,
            configured: false,
            vault_id: None,
            message: None,
        });
    }
    let vault_id = result
        .vault_id
        .ok_or_else(|| "配对结果缺少同步空间 ID".to_string())?;
    if vault_id != expected_vault {
        return Err("配对返回的同步空间与配对码不一致".to_string());
    }
    let envelope = result
        .vault_key_envelope
        .ok_or_else(|| "配对结果缺少同步空间密钥".to_string())?;
    let initiator_public = result
        .initiator_public_key
        .ok_or_else(|| "配对结果缺少发起设备公钥".to_string())?;
    let initiator_public = base64url_decode(&initiator_public)?;
    let secret = base64url_decode(&pairing_secret)?;
    let session_key = keypair.session_key(&secret, pairing_id, &initiator_public)?;
    let device_id = result
        .device_id
        .ok_or_else(|| "配对结果缺少设备 ID".to_string())?;
    let vault_key = open_pairing_vault_key(&envelope, &session_key, pairing_id, &device_id)?;

    let config = VaultSyncConfig {
        enabled: true,
        endpoint,
        vault_id: vault_id.clone(),
        device_id,
        device_token,
        vault_key: base64url_encode(&vault_key),
        device_name,
        bound_user: db::current_user_key(),
    };
    save_config(&config)?;
    disable_legacy_sync();
    // 加入既有空间时不播种本机数据：加入设备上的旧副本可能落后于
    // 权威设备，用新 lamport 全量上传会把较新的远端内容覆盖掉。
    // 新设备先完整拉取远端；本机独有文稿保留在本地。
    prepare_new_vault_binding(&config, false)?;
    let _ = PAIRING.lock().map(|mut guard| *guard = None);
    log::info!("[VaultSync] 已加入同步空间 {}", vault_id);
    Ok(JoinPollView {
        status: "confirmed".to_string(),
        configured: true,
        vault_id: Some(vault_id),
        message: None,
    })
}

fn disable_legacy_sync() {
    let _ = crate::services::lan_sync::set_enabled(false);
    let webdav = crate::services::webdav_sync::load_config();
    if webdav.enabled {
        let _ = crate::services::webdav_sync::save_config_from_input(
            false,
            &webdav.url,
            &webdav.username,
            None,
            &webdav.folder,
        );
    }
}

fn prepare_new_vault_binding(
    config: &VaultSyncConfig,
    seed_local: bool,
) -> Result<(), String> {
    // 校验密钥可解码后再落库，避免配置成功但同步时才发现密钥损坏。
    let _ = decode_vault_key(config)?;
    db::with_db(|conn| {
        store::clear_sync_data(conn)?;
        store::set_binding(conn, &config.vault_id, &config.device_id, true)?;
        store::set_active(conn, true)?;
        if seed_local {
            store::enqueue_all_local(conn)?;
        }
        Ok::<(), String>(())
    })?;
    Ok(())
}

pub async fn leave_vault() -> Result<(), String> {
    let config = load_config();
    if is_configured(&config) {
        if let Ok(client) = WorkerClient::new(&config.endpoint, Some(&config.device_token)) {
            let _ = client.revoke_device(&config.device_id).await;
        }
    }
    delete_config()?;
    db::with_db(|conn| store::clear_sync_data(conn))?;
    // 离开后不要让旧 WebDAV/局域网自动接管；用户可在旧版区手动重新启用。
    disable_legacy_sync();
    let _ = PAIRING.lock().map(|mut guard| *guard = None);
    Ok(())
}

/// 加入空间后手动上传本机独有文稿（远端没有同 ID 实体版本的内容）。
pub fn seed_local_missing() -> Result<usize, String> {
    active_config().ok_or_else(|| "请先启用同步空间".to_string())?;
    db::with_db(|conn| {
        let state = store::load_state(conn)?;
        if state.last_success_at.is_none() {
            return Err("请先完成一次同步，再上传本机独有文稿".to_string());
        }
        store::enqueue_local_missing(conn)
    })
}

pub async fn list_devices() -> Result<Vec<DeviceView>, String> {
    let config = active_config().ok_or_else(|| "请先启用同步空间".to_string())?;
    let client = WorkerClient::new(&config.endpoint, Some(&config.device_token))?;
    let data = client
        .list_devices()
        .await
        .map_err(|error| error.to_string())?;
    Ok(data
        .devices
        .into_iter()
        .map(|device| DeviceView {
            id: device.id,
            name: device.name,
            platform: device.platform,
            created_at: device.created_at,
            last_seen_at: device.last_seen_at,
            is_current: device.is_current,
        })
        .collect())
}

pub async fn revoke_device(device_id: &str) -> Result<(), String> {
    let config = active_config().ok_or_else(|| "请先启用同步空间".to_string())?;
    if device_id == config.device_id {
        return Err("不能在这里撤销当前设备，请使用「离开同步空间」".to_string());
    }
    let client = WorkerClient::new(&config.endpoint, Some(&config.device_token))?;
    client
        .revoke_device(device_id)
        .await
        .map_err(|error| error.to_string())
}

/// 由 sync_engine 在持有全局同步锁后调用。
pub async fn sync_now() -> Result<SyncResult, String> {
    let config = active_config().ok_or_else(|| "同步空间未启用".to_string())?;
    let vault_key = decode_vault_key(&config)?;
    let client = WorkerClient::new(&config.endpoint, Some(&config.device_token))?;
    let identity = SyncIdentity {
        vault_id: &config.vault_id,
        device_id: &config.device_id,
        vault_key: &vault_key,
    };

    // 回收站里超过清理窗口的记录在这里物理删除并生成 tombstone。
    // 旧实现把这段逻辑留在 WebDAV 引擎里，新后端会永远保留 deleted=2。
    let _ = db::with_db(|conn| store::cleanup_expired_deletes(conn))?;

    let mut pushed = 0i64;
    let mut pulled = 0i64;
    let mut pages = 0i64;
    let mut did_reset_cursor = false;

    loop {
        let prepared = db::with_db(|conn| store::prepare_outbox(conn, &identity, PREPARE_BATCH))?;
        let (cursor, pending) = db::with_db(|conn| {
            let state = store::load_state(conn)?;
            let pending = store::pending_operations(conn, PUSH_BATCH)?;
            Ok::<_, String>((state.cursor, pending))
        })?;

        let request = SyncRequest {
            cursor,
            ack: cursor,
            pull_limit: 100,
            push: pending.clone(),
        };
        let data = match client.sync(&request).await {
            Ok(data) => data,
            Err(error) if error.is_cursor_ahead() || error.is_cursor_not_found() => {
                if did_reset_cursor {
                    return Err(format!("同步游标无法恢复：{error}"));
                }
                did_reset_cursor = true;
                db::with_db(|conn| {
                    store::set_cursor(conn, 0)?;
                    conn.execute_batch(
                        "DELETE FROM vault_sync_applied_ops;
                         DELETE FROM vault_sync_deferred_documents;
                         DELETE FROM vault_sync_chunk_refs;
                         DELETE FROM vault_sync_known_chunks;",
                    )
                    .map_err(|error| error.to_string())?;
                    store::enqueue_all_local(conn)
                })?;
                continue;
            }
            Err(error) => {
                let message = error.to_string();
                let _ = db::with_db(|conn| store::set_last_error(conn, &message));
                return Err(message);
            }
        };

        let applied = db::with_db(|conn| store::apply_remote_page(conn, &identity, &data))?;
        if !pending.is_empty() {
            let op_ids: Vec<String> = pending.iter().map(|op| op.op_id.clone()).collect();
            db::with_db(|conn| {
                store::acknowledge_operations(conn, &config.device_id, &op_ids)
            })?;
            pushed += pending.len() as i64;
        }
        pulled += applied as i64;
        pages += 1;
        resolve_deferred(&client, &identity).await?;

        let has_more = data.has_more;
        if !has_more && pending.len() < PUSH_BATCH && prepared == 0 {
            break;
        }
        if pages > 10_000 {
            return Err("同步分页超过安全上限".to_string());
        }
    }

    let finished_at = now_str();
    db::with_db(|conn| store::set_last_success(conn, &finished_at))?;
    Ok(SyncResult {
        pushed_count: pushed as i32,
        pulled_count: pulled as i32,
        conflict_count: 0,
        cleanup_count: 0,
        tombstone_count: 0,
        sync_time: finished_at,
    })
}

async fn resolve_deferred(
    client: &WorkerClient,
    identity: &SyncIdentity<'_>,
) -> Result<(), String> {
    let deferred = db::with_db(|conn| store::deferred_documents(conn))?;
    for item in deferred {
        let payload = store::deferred_payload(identity, &item)?;
        // 正文清单本身也可能分片；deferred 里保存的是顶层 payload。
        let missing = db::with_db(|conn| store::missing_chunk_hashes(conn, &payload))?;
        for hash in missing {
            let op_id = format!("chunk:{hash}");
            match client.get_operation(&op_id).await {
                Ok(data) => {
                    db::with_db(|conn| {
                        store::store_remote_chunk(conn, identity, &data.operation)
                    })?;
                }
                Err(error) if error.is_not_found() => {
                    return Err(
                        "远端文稿还缺少内容块，等待发送设备补齐后重试".to_string()
                    );
                }
                Err(error) => return Err(error.to_string()),
            }
        }
        db::with_db(|conn| store::apply_deferred_document(conn, identity, &item))?;
    }
    Ok(())
}

pub fn status() -> SyncStatus {
    let state = db::with_db(|conn| store::load_state(conn)).unwrap_or_default();
    SyncStatus {
        is_syncing: false,
        last_sync_time: state.last_success_at,
        pending_changes: db::with_db(|conn| store::count_pending_changes(conn)) as i32,
        backend: "vault".to_string(),
    }
}

fn sanitize_device_name(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        default_device_name()
    } else {
        trimmed.chars().take(80).collect()
    }
}

fn keypair_private_bytes(keypair: &PairingKeyPair) -> [u8; 32] {
    keypair.private_key_bytes()
}
