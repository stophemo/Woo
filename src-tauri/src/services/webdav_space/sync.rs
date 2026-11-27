//! WebDAV 同步：本地 outbox 推送为加密操作文件，PROPFIND 两层列举，
//! 按 opId 幂等应用，最后 ack 本地队列。

use std::collections::HashSet;
use std::sync::Mutex;

use once_cell::sync::Lazy;

use serde::{Deserialize, Serialize};
use woo_sync_core::wire::{
    validate_identifier, OperationKind, SyncData, SyncPushOperation, SyncPushSummary,
    SyncPulledOperation,
};

use crate::db;
use crate::services::sync_engine::SyncResult;
use crate::services::utils::{now_str, sha256};
use crate::services::vault_sync::store::{self, SyncIdentity};

use super::client::{ensure_collection, RemoteStore};
use super::devices;
use super::identity::{decode_vault_key, WebDavIdentity};

const OPS_SHARDS: usize = 4;
const PUSH_BATCH: usize = 30;
const PREPARE_BATCH: usize = 100;
const PUSH_ROUNDS: usize = 5;
const MAX_PULL_PER_SYNC: usize = 1000;
/// 设备心跳只用于展示，没必要每轮同步都写。
const HEARTBEAT_INTERVAL_MS: i64 = 6 * 60 * 60 * 1000;
/// 旧版本设备不会更新 state.json，定期做一次全量扫描兼容。
const FULL_SCAN_INTERVAL_MS: i64 = 10 * 60 * 1000;
const META_REMOTE_SEQ: &str = "woo_remote_state_seq";
const META_FULL_SCAN: &str = "woo_full_scan_at";
const META_HEARTBEAT: &str = "woo_heartbeat_at";

/// 空间级变更门闩：任何设备推送后递增 seq，其他设备先看它再决定要不要列举操作。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RemoteState {
    pub protocol_version: u32,
    pub vault_id: String,
    pub seq: i64,
    pub max_lamport: i64,
    pub updated_at: i64,
}

fn state_path(vault_id: &str) -> String {
    format!("v1/{vault_id}/state.json")
}

pub async fn read_remote_state(
    store: &dyn RemoteStore,
    vault_id: &str,
) -> Result<Option<RemoteState>, String> {
    let Some(bytes) = store
        .get(&state_path(vault_id))
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    Ok(serde_json::from_slice(&bytes).ok())
}

async fn write_remote_state(
    store: &dyn RemoteStore,
    identity: &WebDavIdentity,
    existing: Option<&RemoteState>,
    local_max_lamport: i64,
) -> Result<RemoteState, String> {
    let existing = existing.cloned().unwrap_or_default();
    let state = RemoteState {
        protocol_version: 1,
        vault_id: identity.vault_id.clone(),
        seq: existing.seq.saturating_add(1).max(1),
        max_lamport: existing.max_lamport.max(local_max_lamport),
        updated_at: chrono::Utc::now().timestamp_millis(),
    };
    let bytes = serde_json::to_vec(&state)
        .map_err(|error| format!("序列化空间状态失败：{error}"))?;
    store
        .put(&state_path(&identity.vault_id), &bytes, false)
        .await
        .map_err(|error| error.to_string())?;
    Ok(state)
}

/// 目录创建成功后在进程内缓存，避免每次同步都重复 MKCOL。
/// WebDAV 供应商通常按请求数限速，重复 MKCOL 会白白消耗额度。
static READY_ROOTS: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));
static READY_SHARDS: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

async fn ensure_root_cached(
    store: &dyn RemoteStore,
    vault_id: &str,
) -> Result<String, String> {
    let root = ops_root(vault_id);
    if READY_ROOTS.lock().map(|set| set.contains(&root)).unwrap_or(false) {
        return Ok(root);
    }
    ensure_collection(store, &root)
        .await
        .map_err(|error| error.to_string())?;
    if let Ok(mut set) = READY_ROOTS.lock() {
        set.insert(root.clone());
    }
    Ok(root)
}

async fn ensure_shard_cached(
    store: &dyn RemoteStore,
    root: &str,
    shard: &str,
) -> Result<(), String> {
    let path = format!("{root}/{shard}");
    if READY_SHARDS.lock().map(|set| set.contains(&path)).unwrap_or(false) {
        return Ok(());
    }
    store.mkcol(&path).await.map_err(|error| error.to_string())?;
    if let Ok(mut set) = READY_SHARDS.lock() {
        set.insert(path);
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebdavOperationFile {
    pub protocol_version: u32,
    pub vault_id: String,
    pub device_id: String,
    pub op_id: String,
    pub entity_id: String,
    pub kind: OperationKind,
    pub lamport: i64,
    pub ciphertext: String,
    pub nonce: String,
}

impl WebdavOperationFile {
    fn validate(&self, expected_vault: &str, expected_op_id: &str) -> Result<(), String> {
        if self.protocol_version != 1 {
            return Err("未知的同步操作协议版本".to_string());
        }
        if self.vault_id != expected_vault {
            return Err("同步操作不属于当前空间".to_string());
        }
        if self.op_id != expected_op_id {
            return Err("同步操作 ID 与文件名不一致".to_string());
        }
        validate_identifier(&self.op_id, "opId")?;
        validate_identifier(&self.device_id, "deviceId")?;
        validate_identifier(&self.entity_id, "entityId")?;
        SyncPushOperation {
            op_id: self.op_id.clone(),
            entity_id: self.entity_id.clone(),
            kind: self.kind,
            lamport: self.lamport,
            ciphertext: self.ciphertext.clone(),
            nonce: self.nonce.clone(),
        }
        .validate()
    }

    fn from_push(identity: &WebDavIdentity, operation: &SyncPushOperation) -> Self {
        Self {
            protocol_version: 1,
            vault_id: identity.vault_id.clone(),
            device_id: identity.device_id.clone(),
            op_id: operation.op_id.clone(),
            entity_id: operation.entity_id.clone(),
            kind: operation.kind,
            lamport: operation.lamport,
            ciphertext: operation.ciphertext.clone(),
            nonce: operation.nonce.clone(),
        }
    }

    fn to_pulled(&self, server_seq: i64) -> SyncPulledOperation {
        SyncPulledOperation {
            server_seq,
            op_id: self.op_id.clone(),
            device_id: self.device_id.clone(),
            entity_id: self.entity_id.clone(),
            kind: self.kind,
            lamport: self.lamport,
            ciphertext: self.ciphertext.clone(),
            nonce: self.nonce.clone(),
            created_at: 0,
        }
    }
}

pub fn ops_root(vault_id: &str) -> String {
    format!("v1/{vault_id}/ops")
}

fn shard_for(op_id: &str) -> String {
    let digest = sha256(op_id);
    let value = u8::from_str_radix(&digest[..2], 16).unwrap_or(0) as usize % OPS_SHARDS;
    format!("{value:02x}")
}

fn op_file_name(op_id: &str) -> String {
    format!("{}.json", op_id.replace(':', "~"))
}

fn op_id_from_file_name(name: &str) -> Option<String> {
    let stem = name.strip_suffix(".json")?;
    let op_id = stem.replace('~', ":");
    validate_identifier(&op_id, "opId").ok()?;
    Some(op_id)
}

fn op_path(vault_id: &str, op_id: &str) -> String {
    format!(
        "{}/{}/{}",
        ops_root(vault_id),
        shard_for(op_id),
        op_file_name(op_id)
    )
}

struct RemoteOpRef {
    op_id: String,
    path: String,
}

async fn list_remote_ops(
    store: &dyn RemoteStore,
    vault_id: &str,
) -> Result<Vec<RemoteOpRef>, String> {
    let root = ops_root(vault_id);
    let entries = store
        .list(&root)
        .await
        .map_err(|error| error.to_string())?;
    let mut ops = Vec::new();
    for entry in entries {
        if entry.is_dir {
            let shard_path = format!("{root}/{}", entry.name);
            let shard_entries = store
                .list(&shard_path)
                .await
                .map_err(|error| error.to_string())?;
            for file in shard_entries {
                if file.is_dir {
                    continue;
                }
                if let Some(op_id) = op_id_from_file_name(&file.name) {
                    ops.push(RemoteOpRef {
                        path: format!("{shard_path}/{}", file.name),
                        op_id,
                    });
                }
            }
        } else if let Some(op_id) = op_id_from_file_name(&entry.name) {
            ops.push(RemoteOpRef {
                path: format!("{root}/{}", entry.name),
                op_id,
            });
        }
    }
    Ok(ops)
}

fn parse_operation(bytes: &[u8], vault_id: &str, op_id: &str) -> Result<WebdavOperationFile, String> {
    let operation: WebdavOperationFile = serde_json::from_slice(bytes)
        .map_err(|error| format!("同步操作格式损坏：{error}"))?;
    operation.validate(vault_id, op_id)?;
    Ok(operation)
}

fn is_bad_operation_error(error: &str) -> bool {
    ["解密", "密文", "nonce", "AAD", "内容块", "内容清单", "校验", "格式", "无效", "长度"]
        .iter()
        .any(|marker| error.contains(marker))
}

async fn put_operation(
    store: &dyn RemoteStore,
    identity: &WebDavIdentity,
    operation: &SyncPushOperation,
) -> Result<(), String> {
    let root = ops_root(&identity.vault_id);
    let shard = shard_for(&operation.op_id);
    ensure_shard_cached(store, &root, &shard).await?;
    let file = WebdavOperationFile::from_push(identity, operation);
    let bytes = serde_json::to_vec(&file)
        .map_err(|error| format!("序列化同步操作失败：{error}"))?;
    let path = op_path(&identity.vault_id, &operation.op_id);
    match store.put(&path, &bytes, true).await {
        Ok(()) => Ok(()),
        Err(super::client::RemoteError::Conflict) => {
            // 幂等重试：已经存在且内容相同视为成功；不同则拒绝覆盖。
            let existing = store
                .get(&path)
                .await
                .map_err(|error| error.to_string())?;
            if let Some(existing) = existing {
                let existing = parse_operation(&existing, &identity.vault_id, &operation.op_id)?;
                let expected = WebdavOperationFile::from_push(identity, operation);
                if existing == expected {
                    return Ok(());
                }
                return Err(format!("同步操作 ID 冲突：{}", operation.op_id));
            }
            // PUT 的 409 也可能表示缓存中的父目录已在远端被删除。
            if let Ok(mut set) = READY_ROOTS.lock() {
                set.remove(&root);
            }
            if let Ok(mut set) = READY_SHARDS.lock() {
                set.remove(&format!("{root}/{shard}"));
            }
            ensure_root_cached(store, &identity.vault_id).await?;
            ensure_shard_cached(store, &root, &shard).await?;
            store
                .put(&path, &bytes, true)
                .await
                .map_err(|error| error.to_string())
        }
        Err(error) => Err(error.to_string()),
    }
}

async fn push_round(
    store: &dyn RemoteStore,
    identity: &WebDavIdentity,
) -> Result<usize, String> {
    let vault_key = decode_vault_key(identity)?;
    let sync_identity = SyncIdentity {
        vault_id: &identity.vault_id,
        device_id: &identity.device_id,
        vault_key: &vault_key,
    };
    let _ = db::with_db(|conn| store::prepare_outbox(conn, &sync_identity, PREPARE_BATCH))?;
    let pending = db::with_db(|conn| store::pending_operations(conn, PUSH_BATCH))?;
    if pending.is_empty() {
        return Ok(0);
    }
    let mut successes = Vec::new();
    let mut first_error = None;
    for operation in &pending {
        match put_operation(store, identity, operation).await {
            Ok(()) => successes.push(operation.op_id.clone()),
            Err(error) => {
                first_error = Some(error);
                break;
            }
        }
    }
    if !successes.is_empty() {
        let device_id = identity.device_id.clone();
        db::with_db(|conn| store::acknowledge_operations(conn, &device_id, &successes))?;
    }
    if let Some(error) = first_error {
        return Err(error);
    }
    Ok(successes.len())
}

/// 读取远端最大 lamport 作为加入水印。
pub async fn remote_max_lamport(
    store: &dyn RemoteStore,
    vault_id: &str,
) -> Result<i64, String> {
    let mut max_lamport = 0i64;
    for op in list_remote_ops(store, vault_id).await? {
        let Some(bytes) = store.get(&op.path).await.map_err(|error| error.to_string())? else {
            continue;
        };
        match parse_operation(&bytes, vault_id, &op.op_id) {
            Ok(operation) => max_lamport = max_lamport.max(operation.lamport),
            Err(error) => log::warn!(
                "[WebDAV] 计算 Lamport 水位时跳过坏操作 {}：{}",
                op.op_id,
                error
            ),
        }
    }
    Ok(max_lamport)
}

/// 加入空间时的 Lamport 水位：优先读 state.json，旧空间才回退逐条扫描。
pub async fn remote_lamport_floor(
    store: &dyn RemoteStore,
    vault_id: &str,
) -> Result<i64, String> {
    if let Some(state) = read_remote_state(store, vault_id).await? {
        return Ok(state.max_lamport);
    }
    remote_max_lamport(store, vault_id).await
}

async fn pull_remote_ops(
    store: &dyn RemoteStore,
    identity: &WebDavIdentity,
) -> Result<usize, String> {
    let vault_key = decode_vault_key(identity)?;
    let sync_identity = SyncIdentity {
        vault_id: &identity.vault_id,
        device_id: &identity.device_id,
        vault_key: &vault_key,
    };
    let remote_ops = list_remote_ops(store, &identity.vault_id).await?;
    let mut pulled = 0usize;
    let mut examined = 0usize;
    for op in remote_ops {
        if db::with_db(|conn| store::is_operation_applied(conn, &op.op_id))? {
            continue;
        }
        if examined >= MAX_PULL_PER_SYNC {
            log::warn!(
                "[WebDAV] 本次最多拉取 {} 条操作，剩余操作将在下一轮继续",
                MAX_PULL_PER_SYNC
            );
            break;
        }
        examined += 1;
        let Some(bytes) = store.get(&op.path).await.map_err(|error| error.to_string())? else {
            continue;
        };
        let operation = match parse_operation(&bytes, &identity.vault_id, &op.op_id) {
            Ok(operation) => operation,
            Err(error) => {
                log::warn!("[WebDAV] 跳过无法解码的操作 {}：{}", op.op_id, error);
                db::with_db(|conn| store::mark_operation_processed(conn, &op.op_id))?;
                continue;
            }
        };
        let seq = db::with_db(|conn| store::next_operation_sequence(conn))?;
        let data = SyncData {
            push: SyncPushSummary {
                received: 0,
                inserted: 0,
                duplicates: 0,
            },
            pull: vec![operation.to_pulled(seq)],
            cursor: seq,
            has_more: false,
            server_time: 0,
        };
        match db::with_db(|conn| store::apply_remote_page(conn, &sync_identity, &data)) {
            Ok(count) => pulled += count,
            Err(error) if is_bad_operation_error(&error) => {
                log::warn!(
                    "[WebDAV] 跳过校验失败的操作 {}：{}",
                    op.op_id,
                    error
                );
                db::with_db(|conn| store::mark_operation_processed(conn, &op.op_id))?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(pulled)
}

fn resolve_deferred(identity: &WebDavIdentity) -> Result<(), String> {
    let vault_key = decode_vault_key(identity)?;
    let sync_identity = SyncIdentity {
        vault_id: &identity.vault_id,
        device_id: &identity.device_id,
        vault_key: &vault_key,
    };
    let deferred = db::with_db(|conn| store::deferred_documents(conn))?;
    for item in deferred {
        match db::with_db(|conn| store::apply_deferred_document(conn, &sync_identity, &item)) {
            Ok(_) => {}
            Err(error) => log::warn!(
                "[WebDAV] 文稿 {} 的内容块仍未补齐：{}",
                item.entity_id,
                error
            ),
        }
    }
    Ok(())
}

/// 完整同步：清理回收站、推送本地 outbox、拉取远端操作、补齐延迟文稿。
pub async fn sync_now(
    store: &dyn RemoteStore,
    identity: &WebDavIdentity,
) -> Result<SyncResult, String> {
    let _ = db::with_db(|conn| store::cleanup_expired_deletes(conn))?;

    ensure_root_cached(store, &identity.vault_id).await?;

    let first_success = db::with_db(|conn| {
        store::load_state(conn).map(|state| state.last_success_at.is_none())
    })?;

    // 先看门口牌子：没有变化就不列举 ops，省掉每轮的多次 PROPFIND。
    let remote_state = read_remote_state(store, &identity.vault_id).await?;
    let last_remote_seq = db::with_db(|conn| {
        store::get_meta(conn, META_REMOTE_SEQ).and_then(|value| value.parse::<i64>().ok())
    });
    let last_full_scan = db::with_db(|conn| {
        store::get_meta(conn, META_FULL_SCAN).and_then(|value| value.parse::<i64>().ok())
    });
    let now = chrono::Utc::now().timestamp_millis();
    let remote_changed = match &remote_state {
        Some(state) => last_remote_seq != Some(state.seq),
        None => true,
    };
    // 旧版本设备不会写 state.json，每 10 分钟做一次全量扫描兼容。
    let full_scan_due = first_success
        || last_full_scan
            .map(|at| now.saturating_sub(at) >= FULL_SCAN_INTERVAL_MS)
            .unwrap_or(true);

    let mut pushed = 0i64;
    for _ in 0..PUSH_ROUNDS {
        let count = push_round(store, identity).await?;
        pushed += count as i64;
        if count < PUSH_BATCH {
            break;
        }
    }

    let pulled = if remote_changed || full_scan_due {
        let count = pull_remote_ops(store, identity).await? as i64;
        if let Some(state) = remote_state.as_ref() {
            db::with_db(|conn| store::set_meta(conn, META_REMOTE_SEQ, &state.seq.to_string()))?;
        }
        if full_scan_due {
            db::with_db(|conn| store::set_meta(conn, META_FULL_SCAN, &now.to_string()))?;
        }
        count
    } else {
        0
    };
    resolve_deferred(identity)?;

    // 新设备首次拉取完成后，把远端没有的本机独有文稿补传上去。
    if first_success {
        let queued = db::with_db(|conn| store::enqueue_local_missing(conn))?;
        if queued > 0 {
            for _ in 0..PUSH_ROUNDS {
                let count = push_round(store, identity).await?;
                pushed += count as i64;
                if count < PUSH_BATCH {
                    break;
                }
            }
        }
    }

    // 推送后更新门口牌子，让其他设备知道有新内容；旧空间第一次同步时也补上。
    if pushed > 0 || remote_state.is_none() {
        let local_max = db::with_db(|conn| store::load_state(conn).map(|state| state.lamport).unwrap_or(0));
        match write_remote_state(store, identity, remote_state.as_ref(), local_max).await {
            Ok(state) => {
                let _ = db::with_db(|conn| {
                    store::set_meta(conn, META_REMOTE_SEQ, &state.seq.to_string())
                });
            }
            Err(error) => log::warn!("[WebDAV] 更新空间状态失败：{error}"),
        }
    }

    // 心跳只用于设备列表展示，6 小时写一次足够。
    let last_heartbeat = db::with_db(|conn| {
        store::get_meta(conn, META_HEARTBEAT).and_then(|value| value.parse::<i64>().ok())
    });
    if last_heartbeat
        .map(|at| now.saturating_sub(at) >= HEARTBEAT_INTERVAL_MS)
        .unwrap_or(true)
    {
        match devices::write_heartbeat(store, identity).await {
            Ok(()) => {
                if let Err(error) =
                    db::with_db(|conn| store::set_meta(conn, META_HEARTBEAT, &now.to_string()))
                {
                    log::warn!("[WebDAV] 记录心跳时间失败：{error}");
                }
            }
            Err(error) => log::warn!("[WebDAV] 写入设备心跳失败：{}", error),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shard_and_file_names_round_trip() {
        for op_id in [
            "chunk-123e4567-e89b-12d3-a456-426614174000",
            "chunkop:123e4567-e89b-12d3-a456-426614174000",
            "abc",
        ] {
            let name = op_file_name(op_id);
            assert!(name.ends_with(".json"));
            assert_eq!(op_id_from_file_name(&name).as_deref(), Some(op_id));
            assert!(shard_for(op_id).len() == 2);
        }
    }

    #[test]
    fn rejects_path_unsafe_operation_names() {
        assert!(op_id_from_file_name("../evil.json").is_none());
        assert!(op_id_from_file_name("a/b.json").is_none());
    }
}

#[cfg(test)]
mod integration_tests {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use super::*;
    use crate::services::webdav_space::client::test_support::MemoryRemote;
    use crate::services::webdav_space::identity;
    use crate::services::vault_sync::store as sync_store;
    use woo_sync_core::crypto::{base64url_encode, random_vault_key};

    fn test_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn identity(vault_id: &str, device_id: &str, key: &str) -> WebDavIdentity {
        WebDavIdentity {
            vault_id: vault_id.to_string(),
            device_id: device_id.to_string(),
            vault_key: key.to_string(),
            device_name: device_id.to_string(),
            platform: "macos".to_string(),
            bound_user: None,
        }
    }

    #[tokio::test]
    async fn push_pull_round_trip_and_corrupt_operation_is_skipped() {
        let _guard = test_lock();
        let original_dir = crate::db::data_dir();
        let dir_a = std::env::temp_dir().join(format!("woo-sync-a-{}", uuid::Uuid::new_v4()));
        let dir_b = std::env::temp_dir().join(format!("woo-sync-b-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::create_dir_all(&dir_b).unwrap();

        let vault_id = uuid::Uuid::new_v4().to_string();
        let device_a = uuid::Uuid::new_v4().to_string();
        let device_b = uuid::Uuid::new_v4().to_string();
        let key = base64url_encode(&random_vault_key().unwrap());
        let id_a = identity(&vault_id, &device_a, &key);
        let id_b = identity(&vault_id, &device_b, &key);
        let remote = MemoryRemote::new();

        crate::db::close_db();
        crate::db::set_data_dir(dir_a.clone());
        crate::db::with_db(|conn| {
            conn.execute(
                "INSERT INTO note_document(
                   id, folder_id, title, content, branch_name, sort_order,
                   create_time, update_time, deleted, is_locked
                 ) VALUES ('doc-1', 'folder-1', '标题', '<p>hello</p>', NULL, 0,
                   '2026-11-24T10:00:00', '2026-11-24T10:00:00', 0, 0)",
                [],
            )
            .unwrap();
            sync_store::set_binding(conn, &vault_id, &device_a, true).unwrap();
            sync_store::set_active(conn, true).unwrap();
            sync_store::enqueue_all_local(conn).unwrap();
        });

        let pushed = sync_now(&remote, &id_a).await.unwrap();
        assert!(pushed.pushed_count > 0);
        assert!(!remote.files.lock().unwrap().is_empty());
        let max_lamport = remote_max_lamport(&remote, &vault_id).await.unwrap();
        assert!(max_lamport > 0);

        crate::db::close_db();
        crate::db::set_data_dir(dir_b.clone());
        crate::db::with_db(|conn| {
            sync_store::set_binding(conn, &vault_id, &device_b, true).unwrap();
            sync_store::set_active(conn, true).unwrap();
        });
        let pulled = sync_now(&remote, &id_b).await.unwrap();
        assert!(pulled.pulled_count > 0);
        crate::db::with_db(|conn| {
            let content: String = conn
                .query_row(
                    "SELECT content FROM note_document WHERE id = 'doc-1'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(content, "<p>hello</p>");
        });

        // 损坏的单条操作必须被记录为已处理，而不是让整个空间永久失败。
        remote.insert_file(
            &format!("v1/{vault_id}/ops/00/bad-op.json"),
            b"{not valid json",
        );
        // 模拟旧版本设备直接写入操作（不会更新 state.json），触发一次全量扫描。
        crate::db::with_db(|conn| sync_store::set_meta(conn, META_FULL_SCAN, "0")).unwrap();
        let result = sync_now(&remote, &id_a).await.unwrap();
        assert_eq!(result.pulled_count, 0);
        crate::db::with_db(|conn| {
            assert!(sync_store::is_operation_applied(conn, "bad-op").unwrap());
        });

        crate::db::close_db();
        crate::db::set_data_dir(original_dir);
        let _ = std::fs::remove_dir_all(dir_a);
        let _ = std::fs::remove_dir_all(dir_b);
        let _ = identity::space_fingerprint(&vault_id);
    }

    #[tokio::test]
    async fn remote_state_keeps_max_lamport_and_increments_seq() {
        let vault_id = uuid::Uuid::new_v4().to_string();
        let device_id = uuid::Uuid::new_v4().to_string();
        let key = base64url_encode(&random_vault_key().unwrap());
        let id = identity(&vault_id, &device_id, &key);
        let remote = MemoryRemote::new();

        let state = write_remote_state(&remote, &id, None, 10).await.unwrap();
        assert_eq!(state.seq, 1);
        assert_eq!(state.max_lamport, 10);

        let read = read_remote_state(&remote, &vault_id).await.unwrap().unwrap();
        assert_eq!(read.max_lamport, 10);

        let state = write_remote_state(&remote, &id, Some(&read), 5).await.unwrap();
        assert_eq!(state.seq, 2);
        assert_eq!(state.max_lamport, 10);
    }
}

#[cfg(test)]
mod put_tests {
    use super::*;
    use crate::services::webdav_space::client::test_support::MemoryRemote;
    use crate::services::webdav_space::identity::create_identity;
    use woo_sync_core::crypto::base64url_encode;

    #[tokio::test]
    async fn put_operation_is_idempotent_but_rejects_different_content() {
        let remote = MemoryRemote::new();
        let identity = create_identity("Device").unwrap();
        let operation = SyncPushOperation {
            op_id: "op-1".to_string(),
            entity_id: "doc:doc-1".to_string(),
            kind: OperationKind::Upsert,
            lamport: 7,
            ciphertext: base64url_encode(&[1u8; 32]),
            nonce: base64url_encode(&[2u8; 12]),
        };
        ensure_root_cached(&remote, &identity.vault_id).await.unwrap();
        put_operation(&remote, &identity, &operation)
            .await
            .unwrap();
        put_operation(&remote, &identity, &operation)
            .await
            .unwrap();

        let path = op_path(&identity.vault_id, &operation.op_id);
        let mut changed = WebdavOperationFile::from_push(&identity, &operation);
        changed.ciphertext = base64url_encode(&[3u8; 32]);
        remote
            .files
            .lock()
            .unwrap()
            .insert(path, serde_json::to_vec(&changed).unwrap());
        let error = put_operation(&remote, &identity, &operation)
            .await
            .unwrap_err();
        assert!(error.contains("冲突"));
    }
}

#[cfg(test)]
mod local_webdav_smoke {
    use std::time::Duration;

    use super::*;
    use crate::services::webdav_space::client::WebdavClient;
    use woo_sync_core::crypto::{base64url_encode, random_vault_key};

    #[tokio::test]
    #[ignore]
    async fn smoke_against_local_webdav() {
        let root = "http://127.0.0.1:18080";
        let client = WebdavClient::new(root, "", "", true, Duration::ZERO).unwrap();
        let vault_id = uuid::Uuid::new_v4().to_string();
        let key = base64url_encode(&random_vault_key().unwrap());
        let device_a = uuid::Uuid::new_v4().to_string();
        let device_b = uuid::Uuid::new_v4().to_string();

        let dir_a = std::env::temp_dir().join(format!("woo-webdav-a-{}", uuid::Uuid::new_v4()));
        let dir_b = std::env::temp_dir().join(format!("woo-webdav-b-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::create_dir_all(&dir_b).unwrap();
        let original_dir = crate::db::data_dir();

        let id_a = WebDavIdentity {
            vault_id: vault_id.clone(),
            device_id: device_a.clone(),
            vault_key: key.clone(),
            device_name: "A".to_string(),
            platform: "macos".to_string(),
            bound_user: None,
        };
        let id_b = WebDavIdentity {
            vault_id: vault_id.clone(),
            device_id: device_b.clone(),
            vault_key: key.clone(),
            device_name: "B".to_string(),
            platform: "macos".to_string(),
            bound_user: None,
        };

        crate::db::close_db();
        crate::db::set_data_dir(dir_a.clone());
        crate::db::with_db(|conn| {
            conn.execute(
                "INSERT INTO note_document(
                   id, folder_id, title, content, branch_name, sort_order,
                   create_time, update_time, deleted, is_locked
                 ) VALUES ('doc-1', 'folder-1', '标题', '<p>hello</p>', NULL, 0,
                   '2026-11-24T10:00:00', '2026-11-24T10:00:00', 0, 0)",
                [],
            )
            .unwrap();
            store::set_binding(conn, &vault_id, &device_a, true).unwrap();
            store::set_active(conn, true).unwrap();
            store::enqueue_all_local(conn).unwrap();
        });
        let pushed = sync_now(&client, &id_a).await.unwrap();
        assert!(pushed.pushed_count > 0);
        let devices = devices::list_devices(&client, &id_a).await.unwrap();
        assert!(devices.iter().any(|device| device.device_id == id_a.device_id));
        assert!(devices.iter().all(|device| device.is_current));

        crate::db::close_db();
        crate::db::set_data_dir(dir_b.clone());
        crate::db::with_db(|conn| {
            store::set_binding(conn, &vault_id, &device_b, true).unwrap();
            store::set_active(conn, true).unwrap();
        });
        let pulled = sync_now(&client, &id_b).await.unwrap();
        assert!(pulled.pulled_count > 0);
        let devices = devices::list_devices(&client, &id_b).await.unwrap();
        assert_eq!(devices.len(), 2);
        assert!(devices.iter().any(|device| device.device_id == id_b.device_id && device.is_current));
        crate::db::with_db(|conn| {
            let content: String = conn
                .query_row("SELECT content FROM note_document WHERE id = 'doc-1'", [], |row| row.get(0))
                .unwrap();
            assert_eq!(content, "<p>hello</p>");
        });

        crate::db::with_db(|conn| {
            conn.execute("DELETE FROM note_document WHERE id = 'doc-1'", []).unwrap();
        });
        sync_now(&client, &id_b).await.unwrap();

        crate::db::close_db();
        crate::db::set_data_dir(dir_a.clone());
        sync_now(&client, &id_a).await.unwrap();
        crate::db::with_db(|conn| {
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM note_document WHERE id = 'doc-1'", [], |row| row.get(0))
                .unwrap();
            assert_eq!(count, 0);
        });

        crate::db::close_db();
        crate::db::set_data_dir(original_dir);
        let _ = std::fs::remove_dir_all(dir_a);
        let _ = std::fs::remove_dir_all(dir_b);
        let _ = id_b;
    }
}
