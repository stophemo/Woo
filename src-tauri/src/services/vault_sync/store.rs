use rusqlite::{params, Connection, OptionalExtension};
use woo_sync_core::chunker::{
    chunk_bytes, chunk_content, decompress_chunk, verify_chunk, verify_chunk_ref, Chunk,
};
use woo_sync_core::crypto::{
    aes256_gcm_open, aes256_gcm_seal, chunk_aad_canonical, sync_aad_canonical,
};
use woo_sync_core::wire::{
    decode_entity, document_entity_id, encode_entity, folder_entity_id, ChunkRef,
    DocumentPayload, EncryptedEnvelope, FolderPayload, OperationKind, SyncEntity,
    SyncPulledOperation, SyncPushOperation, TombstonePayload, MAX_DOCUMENT_BYTES,
};
use woo_sync_core::SyncData;

use crate::services::utils::now_str;

pub const ENTITY_DOCUMENT: &str = "document";
pub const ENTITY_FOLDER: &str = "folder";
/// 每个内容清单分片包含的块引用数。约 120 * 140 字节 ≈ 17 KiB，
/// 压缩后仍低于单条密文 32 KiB 上限。
const MANIFEST_FANOUT: usize = 120;

#[derive(Debug, Clone)]
pub struct SyncIdentity<'a> {
    pub vault_id: &'a str,
    pub device_id: &'a str,
    pub vault_key: &'a [u8; 32],
}

#[derive(Debug, Clone, Default)]
pub struct LocalSyncState {
    pub vault_id: String,
    pub device_id: String,
    pub cursor: i64,
    pub lamport: i64,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EntityVersion {
    pub lamport: i64,
    pub device_id: String,
    pub is_deleted: bool,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeferredDocument {
    pub entity_id: String,
    pub op_id: String,
    pub kind: OperationKind,
    pub ciphertext: String,
    pub nonce: String,
    pub lamport: i64,
    pub device_id: String,
    pub server_seq: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyOutcome {
    Applied,
    Skipped,
    Deferred,
}

pub fn load_state(conn: &Connection) -> Result<LocalSyncState, String> {
    conn.query_row(
        "SELECT vault_id, device_id, cursor, lamport, last_success_at, last_error
         FROM vault_sync_state WHERE singleton = 1",
        [],
        |row| {
            Ok(LocalSyncState {
                vault_id: row.get(0)?,
                device_id: row.get(1)?,
                cursor: row.get(2)?,
                lamport: row.get(3)?,
                last_success_at: row.get(4)?,
                last_error: row.get(5)?,
            })
        },
    )
    .map_err(|error| format!("读取同步状态失败：{error}"))
}

pub fn set_binding(
    conn: &Connection,
    vault_id: &str,
    device_id: &str,
    reset_cursor: bool,
) -> Result<(), String> {
    let cursor = if reset_cursor {
        "0"
    } else {
        "cursor"
    };
    conn.execute(
        &format!(
            "UPDATE vault_sync_state
             SET vault_id = ?1, device_id = ?2, cursor = {cursor}, last_error = NULL
             WHERE singleton = 1"
        ),
        params![vault_id, device_id],
    )
    .map_err(|error| format!("保存同步身份失败：{error}"))?;
    Ok(())
}

pub fn set_cursor(conn: &Connection, cursor: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE vault_sync_state SET cursor = ?1 WHERE singleton = 1",
        params![cursor],
    )
    .map_err(|error| format!("保存同步游标失败：{error}"))?;
    Ok(())
}

pub fn set_last_success(conn: &Connection, at: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE vault_sync_state
         SET last_success_at = ?1, last_error = NULL WHERE singleton = 1",
        params![at],
    )
    .map_err(|error| format!("保存同步成功时间失败：{error}"))?;
    Ok(())
}

pub fn set_last_error(conn: &Connection, message: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE vault_sync_state SET last_error = ?1 WHERE singleton = 1",
        params![message],
    )
    .map_err(|error| format!("保存同步错误失败：{error}"))?;
    Ok(())
}

pub fn next_lamport(conn: &Connection) -> Result<i64, String> {
    let current: i64 = conn
        .query_row(
            "SELECT lamport FROM vault_sync_state WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("读取 Lamport 时钟失败：{error}"))?;
    let next = current.saturating_add(1).max(1);
    conn.execute(
        "UPDATE vault_sync_state SET lamport = ?1 WHERE singleton = 1",
        params![next],
    )
    .map_err(|error| format!("推进 Lamport 时钟失败：{error}"))?;
    Ok(next)
}

/// 加入已有空间前把本地 Lamport 抬到远端水位，避免本地新操作反而更旧。
pub fn set_lamport_at_least(conn: &Connection, lamport: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE vault_sync_state
         SET lamport = MAX(lamport, ?1) WHERE singleton = 1",
        params![lamport.max(0)],
    )
    .map_err(|error| format!("设置 Lamport 水位失败：{error}"))?;
    Ok(())
}

/// WebDAV 没有服务端游标；用本地 applied 表分配单调序号，复用
/// `apply_remote_page` 的幂等与 LWW 逻辑。
pub fn next_operation_sequence(conn: &Connection) -> Result<i64, String> {
    let cursor: i64 = conn
        .query_row(
            "SELECT cursor FROM vault_sync_state WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("读取同步序号失败：{error}"))?;
    let applied: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(server_seq), 0) FROM vault_sync_applied_ops",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("读取已处理序号失败：{error}"))?;
    Ok(cursor.max(applied).saturating_add(1))
}

pub fn is_operation_applied(conn: &Connection, op_id: &str) -> Result<bool, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM vault_sync_applied_ops WHERE op_id = ?1",
            params![op_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("查询已应用操作失败：{error}"))?;
    Ok(count > 0)
}

/// 把无法解码/校验失败的单条远端操作标记为已处理，防止一个坏对象
/// 让整个空间永久卡在同一条操作上。
pub fn mark_operation_processed(conn: &Connection, op_id: &str) -> Result<(), String> {
    let next: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(server_seq), 0) + 1 FROM vault_sync_applied_ops",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("分配已处理操作序号失败：{error}"))?;
    conn.execute(
        "INSERT OR IGNORE INTO vault_sync_applied_ops(op_id, server_seq, applied_at)
         VALUES (?1, ?2, ?3)",
        params![op_id, next, now_str()],
    )
    .map_err(|error| format!("记录已处理操作失败：{error}"))?;
    Ok(())
}

pub fn set_active(conn: &Connection, active: bool) -> Result<(), String> {
    set_meta(conn, "vault_sync_active", if active { "1" } else { "0" })
}

pub fn is_active(conn: &Connection) -> bool {
    get_meta(conn, "vault_sync_active").as_deref() == Some("1")
}

pub fn clear_sync_data(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "DELETE FROM vault_sync_outbox;
         DELETE FROM vault_sync_entity_versions;
         DELETE FROM vault_sync_applied_ops;
         DELETE FROM vault_sync_pending_entities;
         DELETE FROM vault_sync_pending_deletions;
         DELETE FROM vault_sync_chunk_refs;
         DELETE FROM vault_sync_known_chunks;
         DELETE FROM vault_sync_incoming_chunks;
         DELETE FROM vault_sync_deferred_documents;
         DELETE FROM webdav_pairing_observations;
         INSERT OR REPLACE INTO sync_meta(key, value)
           VALUES ('vault_sync_applying', '0');
         DELETE FROM sync_meta WHERE key IN (
           'last_sync_time', 'webdav_last_sync_time',
           'webdav_known_documents', 'webdav_known_folders',
           'webdav_backoff_until', 'empty_db_full_sync_v1',
           'vault_sync_last_cleanup'
         );
         UPDATE vault_sync_state
         SET vault_id = '', device_id = '', cursor = 0, lamport = 0,
             last_success_at = NULL, last_error = NULL
         WHERE singleton = 1;",
    )
    .map_err(|error| format!("清空同步状态失败：{error}"))?;
    set_active(conn, false)?;
    Ok(())
}

/// 把超过清理窗口的 `deleted = 2` 记录物理删除。删除触发器会写入
/// `vault_sync_pending_deletions`，下一轮 prepare 会生成真正的 tombstone。
pub fn cleanup_expired_deletes(conn: &Connection) -> Result<usize, String> {
    let now = chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap());
    if let Some(last) = get_meta(conn, "vault_sync_last_cleanup") {
        if let Ok(last_at) = chrono::NaiveDateTime::parse_from_str(&last, "%Y-%m-%dT%H:%M:%S") {
            if now.naive_local().signed_duration_since(last_at).num_seconds() < 3600 {
                return Ok(0);
            }
        }
    }
    let cutoff = (now - chrono::Duration::seconds(cleanup_seconds()))
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();
    let documents = conn
        .execute(
            "DELETE FROM note_document WHERE deleted = 2 AND update_time < ?1",
            params![cutoff],
        )
        .map_err(|error| format!("清理回收站文稿失败：{error}"))?;
    conn.execute(
        "DELETE FROM note_document_version
         WHERE document_id NOT IN (SELECT id FROM note_document)",
        [],
    )
    .map_err(|error| format!("清理文稿版本失败：{error}"))?;
    let folders = conn
        .execute(
            "DELETE FROM note_folder WHERE deleted = 2 AND update_time < ?1",
            params![cutoff],
        )
        .map_err(|error| format!("清理回收站目录失败：{error}"))?;
    set_meta(conn, "vault_sync_last_cleanup", &now_str())?;
    if documents + folders > 0 {
        log::info!(
            "[VaultSync] 清理到期回收站记录：文稿 {}，目录 {}",
            documents,
            folders
        );
    }
    Ok(documents + folders)
}

fn cleanup_seconds() -> i64 {
    std::env::var("SYNC_CLEANUP_SECONDS")
        .ok()
        .and_then(|value| {
            let mut total = 1i64;
            for part in value.split('*') {
                total = total.saturating_mul(part.trim().parse::<i64>().ok()?);
            }
            Some(total)
        })
        .filter(|seconds| *seconds > 0)
        .unwrap_or(7 * 24 * 60 * 60)
}

pub fn enqueue_all_local(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "INSERT INTO vault_sync_pending_entities(entity_id, entity_type, changed_at)
         SELECT 'doc:' || id, 'document', ?1 FROM note_document WHERE true
         ON CONFLICT(entity_id) DO UPDATE SET changed_at = excluded.changed_at",
        params![now_str()],
    )
    .map_err(|error| format!("登记本地文稿失败：{error}"))?;
    conn.execute(
        "INSERT INTO vault_sync_pending_entities(entity_id, entity_type, changed_at)
         SELECT 'folder:' || id, 'folder', ?1 FROM note_folder WHERE true
         ON CONFLICT(entity_id) DO UPDATE SET changed_at = excluded.changed_at",
        params![now_str()],
    )
    .map_err(|error| format!("登记本地目录失败：{error}"))?;
    Ok(())
}

/// 加入既有空间后，把「远端没有对应实体版本」的本机文稿/目录加入队列。
/// 只上传本机独有内容，不会用旧副本覆盖远端已有数据。
pub fn enqueue_local_missing(conn: &Connection) -> Result<usize, String> {
    let folders = conn
        .execute(
            "INSERT INTO vault_sync_pending_entities(entity_id, entity_type, changed_at)
             SELECT 'folder:' || f.id, 'folder', ?1
             FROM note_folder f
             WHERE NOT EXISTS (
               SELECT 1 FROM vault_sync_entity_versions v
               WHERE v.entity_id = 'folder:' || f.id
             )
             ON CONFLICT(entity_id) DO UPDATE SET changed_at = excluded.changed_at",
            params![now_str()],
        )
        .map_err(|error| format!("登记本机独有目录失败：{error}"))?;
    let documents = conn
        .execute(
            "INSERT INTO vault_sync_pending_entities(entity_id, entity_type, changed_at)
             SELECT 'doc:' || d.id, 'document', ?1
             FROM note_document d
             WHERE NOT EXISTS (
               SELECT 1 FROM vault_sync_entity_versions v
               WHERE v.entity_id = 'doc:' || d.id
             )
             ON CONFLICT(entity_id) DO UPDATE SET changed_at = excluded.changed_at",
            params![now_str()],
        )
        .map_err(|error| format!("登记本机独有文稿失败：{error}"))?;
    Ok(folders + documents)
}

pub fn count_pending_changes(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT
           (SELECT COUNT(*) FROM vault_sync_outbox)
         + (SELECT COUNT(*) FROM vault_sync_pending_entities)
         + (SELECT COUNT(*) FROM vault_sync_pending_deletions)",
        [],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

/// 把脏实体转换成待发送的加密操作。返回本轮准备的操作数。
pub fn prepare_outbox(
    conn: &Connection,
    identity: &SyncIdentity<'_>,
    limit: usize,
) -> Result<usize, String> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|error| format!("开始同步准备事务失败：{error}"))?;
    let mut prepared = 0usize;

    let pending: Vec<(String, String)> = {
        let mut statement = tx
            .prepare(
                "SELECT entity_id, entity_type FROM vault_sync_pending_entities
                 ORDER BY changed_at ASC LIMIT ?1",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params![limit as i64], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };

    for (entity_id, entity_type) in &pending {
        // 同一实体如果在登记更新后又被硬删除，以删除为准。
        let deleted: bool = tx
            .query_row(
                "SELECT 1 FROM vault_sync_pending_deletions WHERE entity_id = ?1",
                params![entity_id],
                |_| Ok(true),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .unwrap_or(false);
        if !deleted {
            let result = match entity_type.as_str() {
                ENTITY_DOCUMENT => prepare_document(&tx, identity, entity_id),
                ENTITY_FOLDER => prepare_folder(&tx, identity, entity_id),
                _ => Ok(0),
            };
            match result {
                Ok(count) => prepared += count,
                Err(error) => {
                    // 单个超大/损坏文稿不应阻塞整个同步队列。记录错误并
                    // 移除脏标记；用户修正内容后再次编辑会重新入队。
                    log::warn!("[VaultSync] 准备 {} 失败：{}", entity_id, error);
                    tx.execute(
                        "UPDATE vault_sync_state SET last_error = ?1 WHERE singleton = 1",
                        params![error],
                    )
                    .map_err(|error| error.to_string())?;
                }
            }
        }
        tx.execute(
            "DELETE FROM vault_sync_pending_entities WHERE entity_id = ?1",
            params![entity_id],
        )
        .map_err(|error| error.to_string())?;
    }

    let deletions: Vec<(String, String, String)> = {
        let mut statement = tx
            .prepare(
                "SELECT entity_id, entity_type, deleted_at
                 FROM vault_sync_pending_deletions
                 ORDER BY deleted_at ASC LIMIT ?1",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params![limit as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };
    for (entity_id, entity_type, deleted_at) in &deletions {
        remove_unsent_entity_ops(&tx, entity_id)?;
        let payload = TombstonePayload {
            entity_type: if entity_type == ENTITY_FOLDER {
                ENTITY_FOLDER.to_string()
            } else {
                ENTITY_DOCUMENT.to_string()
            },
            id: entity_id
                .split_once(':')
                .map(|(_, id)| id.to_string())
                .unwrap_or_default(),
            deleted_at: deleted_at.clone(),
        };
        let entity = SyncEntity::Tombstone(payload);
        enqueue_entity_op(&tx, identity, entity_id, OperationKind::Delete, &entity)?;
        tx.execute(
            "DELETE FROM vault_sync_pending_deletions WHERE entity_id = ?1",
            params![entity_id],
        )
        .map_err(|error| error.to_string())?;
        prepared += 1;
    }

    tx.commit()
        .map_err(|error| format!("提交同步准备事务失败：{error}"))?;
    Ok(prepared)
}

fn prepare_document(
    conn: &Connection,
    identity: &SyncIdentity<'_>,
    entity_id: &str,
) -> Result<usize, String> {
    let id = entity_id
        .strip_prefix("doc:")
        .ok_or_else(|| "文档实体 ID 格式无效".to_string())?;
    let row: Option<DocumentRow> = conn
        .query_row(
            "SELECT id, folder_id, title, content, branch_name, sort_order,
                    create_time, update_time, deleted, is_locked
             FROM note_document WHERE id = ?1",
            params![id],
            |row| {
                Ok(DocumentRow {
                    id: row.get(0)?,
                    folder_id: row.get(1)?,
                    title: row.get(2)?,
                    content: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    branch_name: row.get(4)?,
                    sort_order: row.get(5)?,
                    create_time: row.get(6)?,
                    update_time: row.get(7)?,
                    deleted: row.get(8)?,
                    is_locked: row.get::<_, i64>(9)? != 0,
                })
            },
        )
        .optional()
        .map_err(|error| format!("读取文稿失败：{error}"))?;
    let Some(row) = row else {
        return Ok(0);
    };
    if row.content.len() > MAX_DOCUMENT_BYTES {
        return Err(format!(
            "文稿「{}」超过单篇同步上限 {} MiB，请先拆分或导出",
            row.title,
            MAX_DOCUMENT_BYTES / 1024 / 1024
        ));
    }

    let chunks = chunk_content(&row.content);
    let old_manifest = load_chunk_manifest(conn, id);
    let new_refs: Vec<ChunkRef> = chunks.iter().map(Chunk::to_ref).collect();
    let old_hashes: std::collections::HashSet<&str> =
        old_manifest.iter().map(|chunk| chunk.hash.as_str()).collect();

    let mut enqueued = 0usize;
    for chunk in &chunks {
        if old_hashes.contains(chunk.hash.as_str()) || chunk_op_exists(conn, chunk)? {
            continue;
        }
        enqueue_chunk(conn, identity, chunk)?;
        enqueued += 1;
    }

    // 正文清单本身按 120 个块一组再打包成分片，文档操作只引用少量分片哈希。
    let mut manifest_refs: Vec<ChunkRef> = Vec::new();
    for group in new_refs.chunks(MANIFEST_FANOUT) {
        let json = serde_json::to_vec(group)
            .map_err(|error| format!("编码内容清单失败：{error}"))?;
        let block = chunk_bytes(&json)?;
        if !chunk_op_exists(conn, &block)? {
            enqueue_chunk(conn, identity, &block)?;
            enqueued += 1;
        }
        manifest_refs.push(block.to_ref());
    }

    let payload = DocumentPayload {
        id: row.id.clone(),
        folder_id: row.folder_id,
        title: row.title,
        branch_name: row.branch_name,
        sort_order: row.sort_order,
        create_time: row.create_time,
        update_time: row.update_time,
        deleted: row.deleted,
        is_locked: row.is_locked,
        manifest: manifest_refs,
        content_plain_bytes: row.content.len() as u64,
    };
    let entity = SyncEntity::Document(payload);
    remove_unsent_entity_ops(conn, entity_id)?;
    enqueue_entity_op(conn, identity, entity_id, OperationKind::Upsert, &entity)?;
    save_chunk_manifest(conn, id, &new_refs)?;
    Ok(enqueued + 1)
}

fn prepare_folder(
    conn: &Connection,
    identity: &SyncIdentity<'_>,
    entity_id: &str,
) -> Result<usize, String> {
    let id = entity_id
        .strip_prefix("folder:")
        .ok_or_else(|| "目录实体 ID 格式无效".to_string())?;
    let row: Option<FolderRow> = conn
        .query_row(
            "SELECT id, parent_id, name, sort_order, create_time, update_time,
                    deleted, is_locked
             FROM note_folder WHERE id = ?1",
            params![id],
            |row| {
                Ok(FolderRow {
                    id: row.get(0)?,
                    parent_id: row.get(1)?,
                    name: row.get(2)?,
                    sort_order: row.get(3)?,
                    create_time: row.get(4)?,
                    update_time: row.get(5)?,
                    deleted: row.get(6)?,
                    is_locked: row.get::<_, i64>(7)? != 0,
                })
            },
        )
        .optional()
        .map_err(|error| format!("读取目录失败：{error}"))?;
    let Some(row) = row else {
        return Ok(0);
    };
    let payload = FolderPayload {
        id: row.id,
        parent_id: row.parent_id,
        name: row.name,
        sort_order: row.sort_order,
        create_time: row.create_time,
        update_time: row.update_time,
        deleted: row.deleted,
        is_locked: row.is_locked,
    };
    let entity = SyncEntity::Folder(payload);
    remove_unsent_entity_ops(conn, entity_id)?;
    enqueue_entity_op(conn, identity, entity_id, OperationKind::Upsert, &entity)?;
    Ok(1)
}

fn enqueue_chunk(
    conn: &Connection,
    identity: &SyncIdentity<'_>,
    chunk: &Chunk,
) -> Result<(), String> {
    let entity_id = chunk.entity_id();
    let lamport = next_lamport(conn)?;
    // 内容块使用随机 opId：不同设备推送同一块时不会因 AAD 中的设备信息
    // 或压缩实现差异触发 OP_ID_CONFLICT。接收端按 entityId 的哈希合并。
    // 使用短横线而不是冒号，避免 WebDAV 路径在部分供应商上不可用。
    let op_id = format!("chunk-{}", uuid::Uuid::new_v4());
    let aad = chunk_aad_canonical(identity.vault_id, &entity_id, lamport);
    let envelope = aes256_gcm_seal(&chunk.stored, identity.vault_key, None, aad.as_bytes())?;
    conn.execute(
        "INSERT OR IGNORE INTO vault_sync_outbox(
           op_id, entity_id, kind, lamport, ciphertext, nonce, created_at
         ) VALUES (?1, ?2, 'upsert', ?3, ?4, ?5, ?6)",
        params![
            op_id,
            entity_id,
            lamport,
            envelope.ciphertext,
            envelope.nonce,
            now_str(),
        ],
    )
    .map_err(|error| format!("写入内容块同步队列失败：{error}"))?;
    conn.execute(
        "INSERT OR IGNORE INTO vault_sync_known_chunks(hash, created_at)
         VALUES (?1, ?2)",
        params![chunk.hash, now_str()],
    )
    .map_err(|error| format!("记录内容块失败：{error}"))?;
    Ok(())
}

fn enqueue_entity_op(
    conn: &Connection,
    identity: &SyncIdentity<'_>,
    entity_id: &str,
    kind: OperationKind,
    entity: &SyncEntity,
) -> Result<(), String> {
    let op_id = format!("op:{}", uuid::Uuid::new_v4());
    let lamport = next_lamport(conn)?;
    let aad = sync_aad_canonical(
        identity.vault_id,
        &op_id,
        entity_id,
        kind.as_str(),
        lamport,
        identity.device_id,
    );
    let plaintext = encode_entity(entity)?;
    let envelope = aes256_gcm_seal(&plaintext, identity.vault_key, None, aad.as_bytes())?;
    conn.execute(
        "INSERT INTO vault_sync_outbox(
           op_id, entity_id, kind, lamport, ciphertext, nonce, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            op_id,
            entity_id,
            kind.as_str(),
            lamport,
            envelope.ciphertext,
            envelope.nonce,
            now_str(),
        ],
    )
    .map_err(|error| format!("写入同步队列失败：{error}"))?;
    Ok(())
}

fn chunk_op_exists(conn: &Connection, chunk: &Chunk) -> Result<bool, String> {
    conn.query_row(
        "SELECT 1 FROM vault_sync_outbox WHERE entity_id = ?1
         UNION ALL
         SELECT 1 FROM vault_sync_known_chunks WHERE hash = ?2
         LIMIT 1",
        params![chunk.entity_id(), chunk.hash],
        |_| Ok(true),
    )
    .optional()
    .map(|value| value.unwrap_or(false))
    .map_err(|error| error.to_string())
}

fn remove_unsent_entity_ops(conn: &Connection, entity_id: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM vault_sync_outbox
         WHERE entity_id = ?1 AND entity_id NOT LIKE 'chunk:%'",
        params![entity_id],
    )
    .map_err(|error| format!("清理旧同步操作失败：{error}"))?;
    Ok(())
}

fn load_chunk_manifest(conn: &Connection, document_id: &str) -> Vec<ChunkRef> {
    conn.query_row(
        "SELECT manifest_json FROM vault_sync_chunk_refs WHERE document_id = ?1",
        params![document_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
    .and_then(|json| serde_json::from_str::<Vec<ChunkRef>>(&json).ok())
    .unwrap_or_default()
}

fn save_chunk_manifest(
    conn: &Connection,
    document_id: &str,
    refs: &[ChunkRef],
) -> Result<(), String> {
    let json = serde_json::to_string(refs)
        .map_err(|error| format!("保存内容块清单失败：{error}"))?;
    conn.execute(
        "INSERT INTO vault_sync_chunk_refs(document_id, manifest_json, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(document_id) DO UPDATE SET
           manifest_json = excluded.manifest_json,
           updated_at = excluded.updated_at",
        params![document_id, json, now_str()],
    )
    .map_err(|error| format!("保存内容块清单失败：{error}"))?;
    Ok(())
}

pub fn pending_operations(
    conn: &Connection,
    limit: usize,
) -> Result<Vec<SyncPushOperation>, String> {
    let mut statement = conn
        .prepare(
            "SELECT op_id, entity_id, kind, lamport, ciphertext, nonce
             FROM vault_sync_outbox ORDER BY seq ASC LIMIT ?1",
        )
        .map_err(|error| format!("读取同步队列失败：{error}"))?;
    let rows = statement
        .query_map(params![limit as i64], |row| {
            let kind: String = row.get(2)?;
            Ok(SyncPushOperation {
                op_id: row.get(0)?,
                entity_id: row.get(1)?,
                kind: match kind.as_str() {
                    "delete" => OperationKind::Delete,
                    "reorder" => OperationKind::Reorder,
                    _ => OperationKind::Upsert,
                },
                lamport: row.get(3)?,
                ciphertext: row.get(4)?,
                nonce: row.get(5)?,
            })
        })
        .map_err(|error| format!("读取同步队列失败：{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取同步队列失败：{error}"))?;
    Ok(rows)
}

pub fn acknowledge_operations(
    conn: &Connection,
    device_id: &str,
    op_ids: &[String],
) -> Result<(), String> {
    for op_id in op_ids {
        let row: Option<(String, String, i64)> = conn
            .query_row(
                "SELECT entity_id, kind, lamport FROM vault_sync_outbox WHERE op_id = ?1",
                params![op_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if let Some((entity_id, kind, lamport)) = row {
            if !entity_id.starts_with("chunk:") {
                let entity_type = if entity_id.starts_with("folder:") {
                    ENTITY_FOLDER
                } else {
                    ENTITY_DOCUMENT
                };
                let deleted = kind == "delete";
                let deleted_at = if deleted { Some(now_str()) } else { None };
                // 只有服务端确认后才把实体版本推进到这次推送的 lamport。
                // 被拒绝的操作不会污染 LWW 版本。
                upsert_entity_version(
                    conn,
                    &entity_id,
                    entity_type,
                    lamport,
                    device_id,
                    deleted,
                    deleted_at.as_deref(),
                )?;
            }
        }
        conn.execute(
            "DELETE FROM vault_sync_outbox WHERE op_id = ?1",
            params![op_id],
        )
        .map_err(|error| format!("确认同步操作失败：{error}"))?;
    }
    Ok(())
}

pub fn apply_remote_page(
    conn: &Connection,
    identity: &SyncIdentity<'_>,
    data: &SyncData,
) -> Result<usize, String> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|error| format!("开始应用远端同步事务失败：{error}"))?;
    set_meta(&tx, "vault_sync_applying", "1")?;
    let mut applied_count = 0usize;
    for operation in &data.pull {
        if applied_op_exists(&tx, &operation.op_id)? {
            continue;
        }
        if operation.entity_id.starts_with("chunk:") {
            store_remote_chunk(&tx, identity, operation)?;
            mark_applied(&tx, operation)?;
            continue;
        }
        if operation.device_id == identity.device_id {
            // 服务端会把本设备刚 push 的操作回显回来。通常本地还有未确认的
            // outbox 或已经应用过该版本，直接跳过；但从旧备份恢复数据库时，
            // 本地版本可能落后于服务端，这时要允许自己的旧操作重新应用。
            let current = load_entity_version(&tx, &operation.entity_id)?;
            let pending = has_pending_local_change(&tx, &operation.entity_id)?;
            let already_applied = current
                .as_ref()
                .is_some_and(|version| version.lamport >= operation.lamport);
            if pending || already_applied {
                mark_applied(&tx, operation)?;
                continue;
            }
        }
        // 应用任何远端实体操作前先抬高本地 Lamport 时钟。新设备从 cursor=0
        // 拉完整历史后，下一次本地编辑必须生成比远端更大的 lamport。
        advance_lamport(&tx, operation.lamport)?;
        let entity = decrypt_entity_op(identity, operation)?;
        let outcome = match entity {
            SyncEntity::Document(payload) => {
                apply_document(&tx, identity, operation, &payload)?
            }
            SyncEntity::Folder(payload) => apply_folder(&tx, identity, operation, &payload)?,
            SyncEntity::Tombstone(payload) => {
                apply_tombstone(&tx, identity, operation, &payload)?
            }
        };
        if outcome == ApplyOutcome::Deferred {
            save_deferred_document(&tx, operation)?;
        }
        mark_applied(&tx, operation)?;
        applied_count += 1;
    }
    tx.execute(
        "UPDATE vault_sync_state
         SET cursor = ?1, last_success_at = ?2, last_error = NULL
         WHERE singleton = 1",
        params![data.cursor, now_str()],
    )
    .map_err(|error| format!("保存同步游标失败：{error}"))?;
    set_meta(&tx, "vault_sync_applying", "0")?;
    tx.commit()
        .map_err(|error| format!("提交远端同步事务失败：{error}"))?;
    Ok(applied_count)
}

fn apply_document(
    conn: &Connection,
    identity: &SyncIdentity<'_>,
    operation: &SyncPulledOperation,
    payload: &DocumentPayload,
) -> Result<ApplyOutcome, String> {
    let entity_id = document_entity_id(&payload.id);
    let current = load_entity_version(conn, &entity_id)?;
    if !remote_wins(operation.lamport, &operation.device_id, current.as_ref()) {
        return Ok(ApplyOutcome::Skipped);
    }
    let local_pending = has_pending_local_change(conn, &entity_id)?;
    if local_pending && operation.device_id != identity.device_id {
        create_document_conflict_copy(conn, &entity_id)?;
    }
    let Some((content, content_refs)) = assemble_document_content(conn, payload)? else {
        return Ok(ApplyOutcome::Deferred);
    };
    conn.execute(
        "INSERT INTO note_document(
           id, folder_id, title, content, branch_name, sort_order,
           create_time, update_time, deleted, is_locked
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
           folder_id = excluded.folder_id,
           title = excluded.title,
           content = excluded.content,
           branch_name = excluded.branch_name,
           sort_order = excluded.sort_order,
           create_time = excluded.create_time,
           update_time = excluded.update_time,
           deleted = excluded.deleted,
           is_locked = excluded.is_locked",
        params![
            payload.id,
            payload.folder_id,
            payload.title,
            content,
            payload.branch_name,
            payload.sort_order,
            payload.create_time,
            payload.update_time,
            payload.deleted,
            i64::from(payload.is_locked),
        ],
    )
    .map_err(|error| format!("应用远端文稿失败：{error}"))?;
    upsert_entity_version(
        conn,
        &entity_id,
        ENTITY_DOCUMENT,
        operation.lamport,
        &operation.device_id,
        false,
        None,
    )?;
    save_chunk_manifest(conn, &payload.id, &content_refs)?;
    conn.execute(
        "DELETE FROM vault_sync_deferred_documents WHERE entity_id = ?1",
        params![entity_id],
    )
    .map_err(|error| error.to_string())?;
    Ok(ApplyOutcome::Applied)
}

fn apply_folder(
    conn: &Connection,
    identity: &SyncIdentity<'_>,
    operation: &SyncPulledOperation,
    payload: &FolderPayload,
) -> Result<ApplyOutcome, String> {
    let entity_id = folder_entity_id(&payload.id);
    let current = load_entity_version(conn, &entity_id)?;
    if !remote_wins(operation.lamport, &operation.device_id, current.as_ref()) {
        return Ok(ApplyOutcome::Skipped);
    }
    let local_pending = has_pending_local_change(conn, &entity_id)?;
    if local_pending && operation.device_id != identity.device_id {
        create_folder_conflict_copy(conn, &entity_id)?;
    }
    // 两台设备同时移动目录可能形成 X→Y→X 的环。远端 LWW 胜出时，
    // 如果新 parent 会回到自己或自己的子树，确定性地把 parent 置空。
    let parent_id = match &payload.parent_id {
        Some(value) if folder_parent_would_cycle(conn, &payload.id, value)? => {
            log::warn!("[VaultSync] 目录 {} 的父级会形成环，已置为根目录", payload.id);
            None
        }
        value => value.clone(),
    };
    conn.execute(
        "INSERT INTO note_folder(
           id, parent_id, name, sort_order, create_time, update_time,
           deleted, is_locked
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(id) DO UPDATE SET
           parent_id = excluded.parent_id,
           name = excluded.name,
           sort_order = excluded.sort_order,
           create_time = excluded.create_time,
           update_time = excluded.update_time,
           deleted = excluded.deleted,
           is_locked = excluded.is_locked",
        params![
            payload.id,
            parent_id,
            payload.name,
            payload.sort_order,
            payload.create_time,
            payload.update_time,
            payload.deleted,
            i64::from(payload.is_locked),
        ],
    )
    .map_err(|error| format!("应用远端目录失败：{error}"))?;
    upsert_entity_version(
        conn,
        &entity_id,
        ENTITY_FOLDER,
        operation.lamport,
        &operation.device_id,
        false,
        None,
    )?;
    Ok(ApplyOutcome::Applied)
}

fn apply_tombstone(
    conn: &Connection,
    identity: &SyncIdentity<'_>,
    operation: &SyncPulledOperation,
    payload: &TombstonePayload,
) -> Result<ApplyOutcome, String> {
    let entity_id = match payload.entity_type.as_str() {
        ENTITY_FOLDER => folder_entity_id(&payload.id),
        _ => document_entity_id(&payload.id),
    };
    let current = load_entity_version(conn, &entity_id)?;
    if !remote_wins(operation.lamport, &operation.device_id, current.as_ref()) {
        return Ok(ApplyOutcome::Skipped);
    }
    if has_pending_local_change(conn, &entity_id)? && operation.device_id != identity.device_id {
        if payload.entity_type == ENTITY_FOLDER {
            create_folder_conflict_copy(conn, &entity_id)?;
        } else {
            create_document_conflict_copy(conn, &entity_id)?;
        }
    }
    if payload.entity_type == ENTITY_FOLDER {
        conn.execute("DELETE FROM note_folder WHERE id = ?1", params![payload.id])
            .map_err(|error| format!("删除远端目录失败：{error}"))?;
    } else {
        conn.execute(
            "DELETE FROM note_document WHERE id = ?1",
            params![payload.id],
        )
        .map_err(|error| format!("删除远端文稿失败：{error}"))?;
        conn.execute(
            "DELETE FROM note_document_version WHERE document_id = ?1",
            params![payload.id],
        )
        .map_err(|error| format!("删除远端文稿版本失败：{error}"))?;
        conn.execute(
            "DELETE FROM vault_sync_chunk_refs WHERE document_id = ?1",
            params![payload.id],
        )
        .map_err(|error| error.to_string())?;
    }
    upsert_entity_version(
        conn,
        &entity_id,
        if payload.entity_type == ENTITY_FOLDER {
            ENTITY_FOLDER
        } else {
            ENTITY_DOCUMENT
        },
        operation.lamport,
        &operation.device_id,
        true,
        Some(&payload.deleted_at),
    )?;
    Ok(ApplyOutcome::Applied)
}

fn folder_parent_would_cycle(
    conn: &Connection,
    folder_id: &str,
    parent_id: &str,
) -> Result<bool, String> {
    let mut current = Some(parent_id.to_string());
    let mut depth = 0usize;
    while let Some(id) = current {
        if id == folder_id {
            return Ok(true);
        }
        depth += 1;
        if depth > 256 {
            return Ok(true);
        }
        current = conn
            .query_row(
                "SELECT parent_id FROM note_folder WHERE id = ?1",
                params![id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(|error| format!("检查目录层级失败：{error}"))?
            .flatten();
    }
    Ok(false)
}

fn create_document_conflict_copy(
    conn: &Connection,
    entity_id: &str,
) -> Result<Option<String>, String> {
    let Some(id) = entity_id.strip_prefix("doc:") else {
        return Ok(None);
    };
    let row: Option<DocumentRow> = conn
        .query_row(
            "SELECT id, folder_id, title, content, branch_name, sort_order,
                    create_time, update_time, deleted, is_locked
             FROM note_document WHERE id = ?1",
            params![id],
            |row| {
                Ok(DocumentRow {
                    id: row.get(0)?,
                    folder_id: row.get(1)?,
                    title: row.get(2)?,
                    content: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    branch_name: row.get(4)?,
                    sort_order: row.get(5)?,
                    create_time: row.get(6)?,
                    update_time: row.get(7)?,
                    deleted: row.get(8)?,
                    is_locked: row.get::<_, i64>(9)? != 0,
                })
            },
        )
        .optional()
        .map_err(|error| format!("读取冲突文稿失败：{error}"))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let new_id = uuid::Uuid::new_v4().to_string();
    let now = now_str();
    let title = conflict_title(&row.title);
    conn.execute(
        "INSERT INTO note_document(
           id, folder_id, title, content, branch_name, sort_order,
           create_time, update_time, deleted, is_locked
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, 0)",
        params![
            new_id,
            row.folder_id,
            title,
            row.content,
            row.branch_name,
            row.sort_order,
            now,
            now,
        ],
    )
    .map_err(|error| format!("创建冲突副本失败：{error}"))?;
    mark_pending(conn, &document_entity_id(&new_id), ENTITY_DOCUMENT)?;
    remove_unsent_entity_ops(conn, entity_id)?;
    clear_pending_entity(conn, entity_id)?;
    log::info!("[VaultSync] 文稿 {} 冲突，已生成本地副本 {}", row.id, new_id);
    Ok(Some(new_id))
}

fn create_folder_conflict_copy(
    conn: &Connection,
    entity_id: &str,
) -> Result<Option<String>, String> {
    let Some(id) = entity_id.strip_prefix("folder:") else {
        return Ok(None);
    };
    let row: Option<FolderRow> = conn
        .query_row(
            "SELECT id, parent_id, name, sort_order, create_time, update_time,
                    deleted, is_locked
             FROM note_folder WHERE id = ?1",
            params![id],
            |row| {
                Ok(FolderRow {
                    id: row.get(0)?,
                    parent_id: row.get(1)?,
                    name: row.get(2)?,
                    sort_order: row.get(3)?,
                    create_time: row.get(4)?,
                    update_time: row.get(5)?,
                    deleted: row.get(6)?,
                    is_locked: row.get::<_, i64>(7)? != 0,
                })
            },
        )
        .optional()
        .map_err(|error| format!("读取冲突目录失败：{error}"))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let new_id = uuid::Uuid::new_v4().to_string();
    let now = now_str();
    conn.execute(
        "INSERT INTO note_folder(
           id, parent_id, name, sort_order, create_time, update_time,
           deleted, is_locked
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
        params![
            new_id,
            row.parent_id,
            conflict_title(&row.name),
            row.sort_order,
            now,
            now,
            i64::from(row.is_locked),
        ],
    )
    .map_err(|error| format!("创建冲突目录失败：{error}"))?;
    mark_pending(conn, &folder_entity_id(&new_id), ENTITY_FOLDER)?;
    remove_unsent_entity_ops(conn, entity_id)?;
    clear_pending_entity(conn, entity_id)?;
    Ok(Some(new_id))
}

fn clear_pending_entity(conn: &Connection, entity_id: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM vault_sync_pending_entities WHERE entity_id = ?1",
        params![entity_id],
    )
    .map_err(|error| format!("清理待同步实体失败：{error}"))?;
    conn.execute(
        "DELETE FROM vault_sync_pending_deletions WHERE entity_id = ?1",
        params![entity_id],
    )
    .map_err(|error| format!("清理待同步删除失败：{error}"))?;
    Ok(())
}

fn conflict_title(title: &str) -> String {
    let suffix = format!("（冲突副本 {}）", &now_str()[11..19]);
    let mut result = title.to_string();
    if result.chars().count() + suffix.chars().count() > 500 {
        result = result.chars().take(480).collect();
    }
    result.push_str(&suffix);
    result
}

pub fn has_pending_local_change(conn: &Connection, entity_id: &str) -> Result<bool, String> {
    let pending: bool = conn
        .query_row(
            "SELECT 1 FROM vault_sync_pending_entities WHERE entity_id = ?1
             UNION ALL
             SELECT 1 FROM vault_sync_pending_deletions WHERE entity_id = ?1
             UNION ALL
             SELECT 1 FROM vault_sync_outbox
               WHERE entity_id = ?1 AND entity_id NOT LIKE 'chunk:%'
             LIMIT 1",
            params![entity_id],
            |_| Ok(true),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .unwrap_or(false);
    Ok(pending)
}

fn mark_pending(
    conn: &Connection,
    entity_id: &str,
    entity_type: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO vault_sync_pending_entities(entity_id, entity_type, changed_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(entity_id) DO UPDATE SET changed_at = excluded.changed_at",
        params![entity_id, entity_type, now_str()],
    )
    .map_err(|error| format!("登记同步变更失败：{error}"))?;
    Ok(())
}

fn decrypt_entity_op(
    identity: &SyncIdentity<'_>,
    operation: &SyncPulledOperation,
) -> Result<SyncEntity, String> {
    let aad = sync_aad_canonical(
        identity.vault_id,
        &operation.op_id,
        &operation.entity_id,
        operation.kind.as_str(),
        operation.lamport,
        &operation.device_id,
    );
    let envelope = EncryptedEnvelope {
        ciphertext: operation.ciphertext.clone(),
        nonce: operation.nonce.clone(),
    };
    let plaintext = aes256_gcm_open(&envelope, identity.vault_key, aad.as_bytes())?;
    decode_entity(&plaintext)
}

pub fn store_remote_chunk(
    conn: &Connection,
    identity: &SyncIdentity<'_>,
    operation: &SyncPulledOperation,
) -> Result<(), String> {
    let aad = chunk_aad_canonical(identity.vault_id, &operation.entity_id, operation.lamport);
    let envelope = EncryptedEnvelope {
        ciphertext: operation.ciphertext.clone(),
        nonce: operation.nonce.clone(),
    };
    let stored = aes256_gcm_open(&envelope, identity.vault_key, aad.as_bytes())?;
    let hash = operation
        .entity_id
        .strip_prefix("chunk:")
        .ok_or_else(|| "内容块实体 ID 格式无效".to_string())?;
    conn.execute(
        "INSERT INTO vault_sync_incoming_chunks(hash, stored, compressed, plain_bytes, created_at)
         VALUES (?1, ?2, 0, ?3, ?4)
         ON CONFLICT(hash) DO UPDATE SET
           stored = excluded.stored,
           plain_bytes = excluded.plain_bytes",
        params![hash, stored.to_vec(), stored.len() as i64, now_str()],
    )
    .map_err(|error| format!("保存远端内容块失败：{error}"))?;
    Ok(())
}

fn load_incoming_chunk(conn: &Connection, hash: &str) -> Result<Option<Vec<u8>>, String> {
    conn.query_row(
        "SELECT stored FROM vault_sync_incoming_chunks WHERE hash = ?1",
        params![hash],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| format!("读取内容块失败：{error}"))
}

fn resolve_manifest_refs(
    conn: &Connection,
    payload: &DocumentPayload,
) -> Result<Option<Vec<ChunkRef>>, String> {
    let mut refs = Vec::new();
    for block_ref in &payload.manifest {
        let Some(stored) = load_incoming_chunk(conn, &block_ref.hash)? else {
            return Ok(None);
        };
        verify_chunk_ref(block_ref, &stored)?;
        let json = decompress_chunk(&stored, block_ref.compressed)?;
        let block_refs: Vec<ChunkRef> = serde_json::from_slice(&json)
            .map_err(|error| format!("内容清单分片解码失败：{error}"))?;
        refs.extend(block_refs);
    }
    Ok(Some(refs))
}

fn assemble_document_content(
    conn: &Connection,
    payload: &DocumentPayload,
) -> Result<Option<(String, Vec<ChunkRef>)>, String> {
    let Some(refs) = resolve_manifest_refs(conn, payload)? else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    for chunk_ref in &refs {
        let Some(stored) = load_incoming_chunk(conn, &chunk_ref.hash)? else {
            return Ok(None);
        };
        verify_chunk_ref(chunk_ref, &stored)?;
        let plain = decompress_chunk(&stored, chunk_ref.compressed)?;
        verify_chunk(chunk_ref, &plain)?;
        bytes.extend_from_slice(&plain);
    }
    if bytes.len() as u64 != payload.content_plain_bytes {
        return Err("同步文稿总长度与清单不一致".to_string());
    }
    let content = String::from_utf8(bytes)
        .map_err(|_| "同步文稿不是有效的 UTF-8 文本".to_string())?;
    Ok(Some((content, refs)))
}

fn load_entity_version(
    conn: &Connection,
    entity_id: &str,
) -> Result<Option<EntityVersion>, String> {
    conn.query_row(
        "SELECT lamport, device_id, is_deleted, deleted_at
         FROM vault_sync_entity_versions WHERE entity_id = ?1",
        params![entity_id],
        |row| {
            Ok(EntityVersion {
                lamport: row.get(0)?,
                device_id: row.get(1)?,
                is_deleted: row.get::<_, i64>(2)? != 0,
                deleted_at: row.get(3)?,
            })
        },
    )
    .optional()
    .map_err(|error| format!("读取实体版本失败：{error}"))
}

fn remote_wins(lamport: i64, device_id: &str, current: Option<&EntityVersion>) -> bool {
    match current {
        None => true,
        Some(current) => {
            lamport > current.lamport
                || (lamport == current.lamport && device_id > current.device_id.as_str())
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn upsert_entity_version(
    conn: &Connection,
    entity_id: &str,
    entity_type: &str,
    lamport: i64,
    device_id: &str,
    is_deleted: bool,
    deleted_at: Option<&str>,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO vault_sync_entity_versions(
           entity_id, entity_type, lamport, device_id, is_deleted, deleted_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(entity_id) DO UPDATE SET
           entity_type = excluded.entity_type,
           lamport = excluded.lamport,
           device_id = excluded.device_id,
           is_deleted = excluded.is_deleted,
           deleted_at = excluded.deleted_at",
        params![
            entity_id,
            entity_type,
            lamport,
            device_id,
            i64::from(is_deleted),
            deleted_at,
        ],
    )
    .map_err(|error| format!("保存实体版本失败：{error}"))?;
    Ok(())
}

fn advance_lamport(conn: &Connection, lamport: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE vault_sync_state
         SET lamport = MAX(lamport, ?1) WHERE singleton = 1",
        params![lamport],
    )
    .map_err(|error| format!("推进 Lamport 时钟失败：{error}"))?;
    Ok(())
}

fn applied_op_exists(conn: &Connection, op_id: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT 1 FROM vault_sync_applied_ops WHERE op_id = ?1",
        params![op_id],
        |_| Ok(true),
    )
    .optional()
    .map(|value| value.unwrap_or(false))
    .map_err(|error| error.to_string())
}

fn mark_applied(conn: &Connection, operation: &SyncPulledOperation) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO vault_sync_applied_ops(op_id, server_seq, applied_at)
         VALUES (?1, ?2, ?3)",
        params![operation.op_id, operation.server_seq, now_str()],
    )
    .map_err(|error| format!("记录已应用操作失败：{error}"))?;
    Ok(())
}

fn save_deferred_document(
    conn: &Connection,
    operation: &SyncPulledOperation,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO vault_sync_deferred_documents(
           entity_id, op_id, kind, ciphertext, nonce, lamport, device_id,
           server_seq, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(entity_id) DO UPDATE SET
           op_id = excluded.op_id,
           kind = excluded.kind,
           ciphertext = excluded.ciphertext,
           nonce = excluded.nonce,
           lamport = excluded.lamport,
           device_id = excluded.device_id,
           server_seq = excluded.server_seq,
           created_at = excluded.created_at",
        params![
            operation.entity_id,
            operation.op_id,
            operation.kind.as_str(),
            operation.ciphertext,
            operation.nonce,
            operation.lamport,
            operation.device_id,
            operation.server_seq,
            now_str(),
        ],
    )
    .map_err(|error| format!("保存待补齐文稿失败：{error}"))?;
    Ok(())
}

pub fn deferred_documents(conn: &Connection) -> Result<Vec<DeferredDocument>, String> {
    let mut statement = conn
        .prepare(
            "SELECT entity_id, op_id, kind, ciphertext, nonce, lamport,
                    device_id, server_seq
             FROM vault_sync_deferred_documents ORDER BY server_seq ASC",
        )
        .map_err(|error| format!("读取待补齐文稿失败：{error}"))?;
    let rows = statement
        .query_map([], |row| {
            let kind: String = row.get(2)?;
            Ok(DeferredDocument {
                entity_id: row.get(0)?,
                op_id: row.get(1)?,
                kind: match kind.as_str() {
                    "delete" => OperationKind::Delete,
                    "reorder" => OperationKind::Reorder,
                    _ => OperationKind::Upsert,
                },
                ciphertext: row.get(3)?,
                nonce: row.get(4)?,
                lamport: row.get(5)?,
                device_id: row.get(6)?,
                server_seq: row.get(7)?,
            })
        })
        .map_err(|error| format!("读取待补齐文稿失败：{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取待补齐文稿失败：{error}"))?;
    Ok(rows)
}

pub fn deferred_payload(
    identity: &SyncIdentity<'_>,
    deferred: &DeferredDocument,
) -> Result<DocumentPayload, String> {
    let aad = sync_aad_canonical(
        identity.vault_id,
        &deferred.op_id,
        &deferred.entity_id,
        deferred.kind.as_str(),
        deferred.lamport,
        &deferred.device_id,
    );
    let envelope = EncryptedEnvelope {
        ciphertext: deferred.ciphertext.clone(),
        nonce: deferred.nonce.clone(),
    };
    let plaintext = aes256_gcm_open(&envelope, identity.vault_key, aad.as_bytes())?;
    match decode_entity(&plaintext)? {
        SyncEntity::Document(payload) => Ok(payload),
        _ => Err("待补齐实体不是文稿".to_string()),
    }
}

pub fn missing_chunk_hashes(
    conn: &Connection,
    payload: &DocumentPayload,
) -> Result<Vec<String>, String> {
    let mut missing = Vec::new();
    let mut content_refs = Vec::new();
    for block_ref in &payload.manifest {
        match load_incoming_chunk(conn, &block_ref.hash)? {
            None => missing.push(block_ref.hash.clone()),
            Some(stored) => {
                verify_chunk_ref(block_ref, &stored)?;
                let json = decompress_chunk(&stored, block_ref.compressed)?;
                let block_refs: Vec<ChunkRef> = serde_json::from_slice(&json)
                    .map_err(|error| format!("内容清单分片解码失败：{error}"))?;
                content_refs.extend(block_refs);
            }
        }
    }
    for chunk_ref in &content_refs {
        if load_incoming_chunk(conn, &chunk_ref.hash)?.is_none() {
            missing.push(chunk_ref.hash.clone());
        }
    }
    missing.sort();
    missing.dedup();
    Ok(missing)
}

pub fn apply_deferred_document(
    conn: &Connection,
    identity: &SyncIdentity<'_>,
    deferred: &DeferredDocument,
) -> Result<bool, String> {
    let payload = deferred_payload(identity, deferred)?;
    if !missing_chunk_hashes(conn, &payload)?.is_empty() {
        return Ok(false);
    }
    let tx = conn
        .unchecked_transaction()
        .map_err(|error| format!("开始补齐文稿事务失败：{error}"))?;
    set_meta(&tx, "vault_sync_applying", "1")?;
    let operation = SyncPulledOperation {
        server_seq: deferred.server_seq,
        op_id: deferred.op_id.clone(),
        device_id: deferred.device_id.clone(),
        entity_id: deferred.entity_id.clone(),
        kind: deferred.kind,
        lamport: deferred.lamport,
        ciphertext: deferred.ciphertext.clone(),
        nonce: deferred.nonce.clone(),
        created_at: 0,
    };
    let outcome = apply_document(&tx, identity, &operation, &payload)?;
    set_meta(&tx, "vault_sync_applying", "0")?;
    tx.commit()
        .map_err(|error| format!("提交补齐文稿事务失败：{error}"))?;
    Ok(outcome != ApplyOutcome::Deferred)
}

fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO sync_meta(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|error| format!("写入同步元数据失败：{error}"))?;
    Ok(())
}

fn get_meta(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM sync_meta WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .ok()
    .flatten()
}

#[derive(Debug)]
struct DocumentRow {
    id: String,
    folder_id: String,
    title: String,
    content: String,
    branch_name: Option<String>,
    sort_order: i64,
    create_time: String,
    update_time: String,
    deleted: i64,
    is_locked: bool,
}

#[derive(Debug)]
struct FolderRow {
    id: String,
    parent_id: Option<String>,
    name: String,
    sort_order: i64,
    create_time: String,
    update_time: String,
    deleted: i64,
    is_locked: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use woo_sync_core::wire::{SyncPushSummary, SyncData};

    const KEY: [u8; 32] = [7u8; 32];

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init_schema(&conn).unwrap();
        conn
    }

    fn insert_sample(conn: &Connection) {
        conn.execute(
            "INSERT INTO note_folder(
               id, parent_id, name, sort_order, create_time, update_time,
               deleted, is_locked
             ) VALUES ('folder-1', NULL, '测试目录', 0,
               '2026-11-24T10:00:00', '2026-11-24T10:00:00', 0, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO note_document(
               id, folder_id, title, content, branch_name, sort_order,
               create_time, update_time, deleted, is_locked
             ) VALUES ('doc-1', 'folder-1', '标题', ?1, NULL, 0,
               '2026-11-24T10:00:00', '2026-11-24T10:00:00', 0, 0)",
            params!["<p>正文</p>".repeat(4000)],
        )
        .unwrap();
    }

    fn identity<'a>(vault: &'a str, device: &'a str) -> SyncIdentity<'a> {
        SyncIdentity {
            vault_id: vault,
            device_id: device,
            vault_key: &KEY,
        }
    }

    fn outbox_as_pulled(conn: &Connection, device: &str) -> Vec<SyncPulledOperation> {
        pending_operations(conn, 1000)
            .unwrap()
            .into_iter()
            .enumerate()
            .map(|(index, op)| SyncPulledOperation {
                server_seq: (index + 1) as i64,
                op_id: op.op_id,
                device_id: device.to_string(),
                entity_id: op.entity_id,
                kind: op.kind,
                lamport: op.lamport,
                ciphertext: op.ciphertext,
                nonce: op.nonce,
                created_at: 1,
            })
            .collect()
    }

    #[test]
    fn triggers_only_record_local_changes_while_sync_is_active() {
        let conn = setup();
        conn.execute(
            "INSERT INTO note_document(
               id, folder_id, title, content, branch_name, sort_order,
               create_time, update_time, deleted, is_locked
             ) VALUES ('doc-before', 'folder', '标题', '<p>x</p>', NULL, 0,
               '2026-11-24T10:00:00', '2026-11-24T10:00:00', 0, 0)",
            [],
        )
        .unwrap();
        assert_eq!(count_pending_changes(&conn), 0);

        set_active(&conn, true).unwrap();
        insert_sample(&conn);
        assert_eq!(count_pending_changes(&conn), 2);

        set_meta(&conn, "vault_sync_applying", "1").unwrap();
        conn.execute(
            "UPDATE note_document SET title = '远端更新' WHERE id = 'doc-1'",
            [],
        )
        .unwrap();
        assert_eq!(count_pending_changes(&conn), 2);
        set_meta(&conn, "vault_sync_applying", "0").unwrap();
    }

    #[test]
    fn prepared_document_round_trips_between_two_devices() {
        let source = setup();
        set_binding(&source, "vault-1", "device-a", true).unwrap();
        set_active(&source, true).unwrap();
        insert_sample(&source);
        let source_identity = identity("vault-1", "device-a");
        assert!(prepare_outbox(&source, &source_identity, 100).unwrap() >= 2);
        let pulled = outbox_as_pulled(&source, "device-a");
        assert!(pulled.iter().any(|op| op.entity_id.starts_with("chunk:")));
        assert!(pulled.iter().any(|op| op.entity_id == "doc:doc-1"));

        let target = setup();
        set_binding(&target, "vault-1", "device-b", true).unwrap();
        set_active(&target, true).unwrap();
        let target_identity = identity("vault-1", "device-b");
        let data = SyncData {
            push: SyncPushSummary {
                received: 0,
                inserted: 0,
                duplicates: 0,
            },
            pull: pulled,
            cursor: 10,
            has_more: false,
            server_time: 1,
        };
        apply_remote_page(&target, &target_identity, &data).unwrap();

        let (title, content): (String, String) = target
            .query_row(
                "SELECT title, content FROM note_document WHERE id = 'doc-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(title, "标题");
        assert_eq!(content, "<p>正文</p>".repeat(4000));
        let folder: String = target
            .query_row("SELECT name FROM note_folder WHERE id = 'folder-1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(folder, "测试目录");

        // 应用远端历史后，本地 Lamport 必须高于远端最大版本。
        let remote_max = data
            .pull
            .iter()
            .filter(|op| !op.entity_id.starts_with("chunk:"))
            .map(|op| op.lamport)
            .max()
            .unwrap_or(0);
        let next = next_lamport(&target).unwrap();
        assert!(next > remote_max, "next={next}, remote_max={remote_max}");
    }

    #[test]
    fn own_device_echo_can_restore_a_stale_backup() {
        let source = setup();
        set_binding(&source, "vault-1", "device-a", true).unwrap();
        set_active(&source, true).unwrap();
        insert_sample(&source);
        let identity = identity("vault-1", "device-a");
        prepare_outbox(&source, &identity, 100).unwrap();
        let pulled = outbox_as_pulled(&source, "device-a");

        // 从旧备份恢复后的数据库：cursor/outbox 落后，但服务端有本设备
        // 之前推送的操作。不能因为 deviceId 相同就无条件跳过。
        let restored = setup();
        set_binding(&restored, "vault-1", "device-a", true).unwrap();
        set_active(&restored, true).unwrap();
        let data = SyncData {
            push: SyncPushSummary {
                received: 0,
                inserted: 0,
                duplicates: 0,
            },
            pull: pulled,
            cursor: 10,
            has_more: false,
            server_time: 1,
        };
        apply_remote_page(&restored, &identity, &data).unwrap();
        let content: String = restored
            .query_row(
                "SELECT content FROM note_document WHERE id = 'doc-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(content, "<p>正文</p>".repeat(4000));
    }

    #[test]
    fn large_document_uses_manifest_shards_and_keeps_operation_small() {
        let conn = setup();
        set_binding(&conn, "vault-1", "device-a", true).unwrap();
        set_active(&conn, true).unwrap();
        conn.execute(
            "INSERT INTO note_document(
               id, folder_id, title, content, branch_name, sort_order,
               create_time, update_time, deleted, is_locked
             ) VALUES ('doc-large', 'folder-1', '长文', ?1, NULL, 0,
               '2026-11-24T10:00:00', '2026-11-24T10:00:00', 0, 0)",
            params!["<p>长文内容</p>".repeat(300_000)],
        )
        .unwrap();
        let identity = identity("vault-1", "device-a");
        prepare_outbox(&conn, &identity, 100).unwrap();
        let ops = pending_operations(&conn, 100_000).unwrap();
        let document_op = ops
            .iter()
            .find(|op| op.entity_id == "doc:doc-large")
            .expect("文稿操作应存在");
        let ciphertext_len = woo_sync_core::crypto::base64url_decode(&document_op.ciphertext)
            .unwrap()
            .len();
        assert!(
            ciphertext_len <= 32 * 1024,
            "manifest 操作不应超过单条密文上限，实际 {ciphertext_len}"
        );
        let aad = sync_aad_canonical(
            "vault-1",
            &document_op.op_id,
            &document_op.entity_id,
            document_op.kind.as_str(),
            document_op.lamport,
            "device-a",
        );
        let plaintext = aes256_gcm_open(
            &EncryptedEnvelope {
                ciphertext: document_op.ciphertext.clone(),
                nonce: document_op.nonce.clone(),
            },
            &KEY,
            aad.as_bytes(),
        )
        .unwrap();
        let entity = decode_entity(&plaintext).unwrap();
        let SyncEntity::Document(payload) = entity else {
            panic!("应为文稿实体");
        };
        assert!(
            payload.manifest.len() > 1,
            "long document should be split into multiple manifests"
        );
    }

    #[test]
    fn remote_edit_preserves_local_change_as_conflict_copy() {
        let source = setup();
        set_binding(&source, "vault-1", "device-a", true).unwrap();
        set_active(&source, true).unwrap();
        insert_sample(&source);
        let source_identity = identity("vault-1", "device-a");
        prepare_outbox(&source, &source_identity, 100).unwrap();
        let pulled = outbox_as_pulled(&source, "device-a");

        let target = setup();
        set_binding(&target, "vault-1", "device-b", true).unwrap();
        set_active(&target, true).unwrap();
        target
            .execute(
                "INSERT INTO note_document(
                   id, folder_id, title, content, branch_name, sort_order,
                   create_time, update_time, deleted, is_locked
                 ) VALUES ('doc-1', 'folder-1', '本地标题', '<p>本地正文</p>', NULL, 0,
                   '2026-11-24T10:00:00', '2026-11-24T10:00:00', 0, 0)",
                [],
            )
            .unwrap();
        assert!(count_pending_changes(&target) > 0);

        let target_identity = identity("vault-1", "device-b");
        let data = SyncData {
            push: SyncPushSummary {
                received: 0,
                inserted: 0,
                duplicates: 0,
            },
            pull: pulled,
            cursor: 10,
            has_more: false,
            server_time: 1,
        };
        apply_remote_page(&target, &target_identity, &data).unwrap();

        let original: String = target
            .query_row("SELECT content FROM note_document WHERE id = 'doc-1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(original, "<p>正文</p>".repeat(4000));

        let (copy_title, copy_content): (String, String) = target
            .query_row(
                "SELECT title, content FROM note_document
                 WHERE id <> 'doc-1' AND title LIKE '%冲突副本%' LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(copy_title.contains("冲突副本"));
        assert_eq!(copy_content, "<p>本地正文</p>");
    }
}

