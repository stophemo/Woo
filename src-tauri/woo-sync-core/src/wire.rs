use serde::{Deserialize, Serialize};

use crate::crypto::{base64url_decode, AES256_NONCE_BYTES, AES256_TAG_BYTES};

pub const MAX_CIPHERTEXT_BYTES: usize = 32 * 1024;
pub const MAX_IDENTIFIER_CHARS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationKind {
    Upsert,
    Delete,
    Reorder,
}

impl OperationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Upsert => "upsert",
            Self::Delete => "delete",
            Self::Reorder => "reorder",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EncryptedEnvelope {
    pub ciphertext: String,
    pub nonce: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncPushOperation {
    pub op_id: String,
    pub entity_id: String,
    pub kind: OperationKind,
    pub lamport: i64,
    pub ciphertext: String,
    pub nonce: String,
}

impl SyncPushOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_identifier(&self.op_id, "opId")?;
        validate_identifier(&self.entity_id, "entityId")?;
        if self.lamport < 1 {
            return Err("lamport 必须大于等于 1".to_string());
        }
        let ciphertext = base64url_decode(&self.ciphertext)?;
        if ciphertext.len() < AES256_TAG_BYTES || ciphertext.len() > MAX_CIPHERTEXT_BYTES {
            return Err(format!(
                "密文长度必须在 {AES256_TAG_BYTES} 到 {MAX_CIPHERTEXT_BYTES} 字节之间"
            ));
        }
        if base64url_decode(&self.nonce)?.len() != AES256_NONCE_BYTES {
            return Err("nonce 必须是 12 字节".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncPulledOperation {
    pub server_seq: i64,
    pub op_id: String,
    pub device_id: String,
    pub entity_id: String,
    pub kind: OperationKind,
    pub lamport: i64,
    pub ciphertext: String,
    pub nonce: String,
    pub created_at: i64,
}

impl SyncPulledOperation {
    pub fn validate(&self) -> Result<(), String> {
        if self.server_seq < 1 {
            return Err("serverSeq 必须大于等于 1".to_string());
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncRequest {
    pub cursor: i64,
    #[serde(default)]
    pub ack: i64,
    pub pull_limit: i64,
    #[serde(default)]
    pub push: Vec<SyncPushOperation>,
}

impl SyncRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.cursor < 0 || self.ack < 0 {
            return Err("cursor 与 ack 必须大于等于 0".to_string());
        }
        if self.ack > self.cursor {
            return Err("ack 不得大于 cursor".to_string());
        }
        if !(1..=100).contains(&self.pull_limit) {
            return Err("pullLimit 必须在 1 到 100 之间".to_string());
        }
        if self.push.len() > 50 {
            return Err("单批 push 不得超过 50 条".to_string());
        }
        for operation in &self.push {
            operation.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncPushSummary {
    pub received: i64,
    pub inserted: i64,
    pub duplicates: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncData {
    pub push: SyncPushSummary,
    pub pull: Vec<SyncPulledOperation>,
    pub cursor: i64,
    pub has_more: bool,
    pub server_time: i64,
}

impl SyncData {
    pub fn validate(&self) -> Result<(), String> {
        if self.cursor < 0 {
            return Err("cursor 必须大于等于 0".to_string());
        }
        if self.push.received < 0 || self.push.inserted < 0 || self.push.duplicates < 0 {
            return Err("push 计数不得为负".to_string());
        }
        if self.push.inserted + self.push.duplicates != self.push.received {
            return Err("push 计数不一致".to_string());
        }
        if self.pull.len() > 100 {
            return Err("单页 pull 不得超过 100 条".to_string());
        }
        let mut previous = 0;
        for operation in &self.pull {
            operation.validate()?;
            if operation.server_seq <= previous {
                return Err("pull 的 serverSeq 必须严格递增".to_string());
            }
            previous = operation.server_seq;
        }
        if let Some(last) = self.pull.last() {
            if last.server_seq != self.cursor {
                return Err("页尾 serverSeq 必须等于 cursor".to_string());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChunkRef {
    /// 明文块的 SHA-256 十六进制摘要；同时也是远端 chunk 实体 ID 的一部分。
    pub hash: String,
    pub plain_bytes: u32,
    pub stored_bytes: u32,
    pub compressed: bool,
}

impl ChunkRef {
    pub fn validate(&self) -> Result<(), String> {
        if self.hash.len() != 64 || !self.hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("chunk hash 必须是 64 位十六进制".to_string());
        }
        if self.plain_bytes == 0 || self.stored_bytes == 0 {
            return Err("chunk 大小不得为 0".to_string());
        }
        if self.stored_bytes > MAX_CIPHERTEXT_BYTES as u32 {
            return Err("chunk 存储大小超过单条密文上限".to_string());
        }
        Ok(())
    }

    pub fn entity_id(&self) -> String {
        format!("chunk:{}", self.hash)
    }
}

/// 单篇正文的同步上限。超过后拒绝同步并提示用户拆分/导出。
pub const MAX_DOCUMENT_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentPayload {
    pub id: String,
    pub folder_id: String,
    pub title: String,
    pub branch_name: Option<String>,
    pub sort_order: i64,
    pub create_time: String,
    pub update_time: String,
    /// 与数据库一致：0 正常，1 回收站，2 待清理。
    pub deleted: i64,
    pub is_locked: bool,
    /// 内容清单分片。每个分片的明文是若干 ChunkRef 的 JSON 数组；
    /// 这样几万块的长文也不会把单条同步操作撑过 32 KiB。
    pub manifest: Vec<ChunkRef>,
    pub content_plain_bytes: u64,
}

impl DocumentPayload {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() || self.folder_id.is_empty() {
            return Err("document id 与 folderId 不得为空".to_string());
        }
        if !(0..=2).contains(&self.deleted) {
            return Err("document deleted 只能是 0、1 或 2".to_string());
        }
        if self.content_plain_bytes as usize > MAX_DOCUMENT_BYTES {
            return Err("单篇文稿超过同步上限".to_string());
        }
        for block in &self.manifest {
            block.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FolderPayload {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub create_time: String,
    pub update_time: String,
    pub deleted: i64,
    pub is_locked: bool,
}

impl FolderPayload {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() || self.name.is_empty() {
            return Err("folder id 与 name 不得为空".to_string());
        }
        if !(0..=2).contains(&self.deleted) {
            return Err("folder deleted 只能是 0、1 或 2".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TombstonePayload {
    /// document 或 folder。
    pub entity_type: String,
    pub id: String,
    pub deleted_at: String,
}

impl TombstonePayload {
    pub fn validate(&self) -> Result<(), String> {
        if self.entity_type != "document" && self.entity_type != "folder" {
            return Err("tombstone entityType 只能是 document 或 folder".to_string());
        }
        if self.id.is_empty() {
            return Err("tombstone id 不得为空".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncEntity {
    Document(DocumentPayload),
    Folder(FolderPayload),
    Tombstone(TombstonePayload),
}

impl SyncEntity {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Document(payload) => payload.validate(),
            Self::Folder(payload) => payload.validate(),
            Self::Tombstone(payload) => payload.validate(),
        }
    }

    pub fn entity_id(&self) -> String {
        match self {
            Self::Document(payload) => document_entity_id(&payload.id),
            Self::Folder(payload) => folder_entity_id(&payload.id),
            Self::Tombstone(payload) => match payload.entity_type.as_str() {
                "document" => document_entity_id(&payload.id),
                "folder" => folder_entity_id(&payload.id),
                _ => format!("unknown:{}", payload.id),
            },
        }
    }
}

pub fn document_entity_id(id: &str) -> String {
    format!("doc:{id}")
}

pub fn folder_entity_id(id: &str) -> String {
    format!("folder:{id}")
}

pub fn encode_entity(entity: &SyncEntity) -> Result<Vec<u8>, String> {
    entity.validate()?;
    serde_json::to_vec(entity).map_err(|error| format!("同步实体编码失败：{error}"))
}

pub fn decode_entity(data: &[u8]) -> Result<SyncEntity, String> {
    let entity: SyncEntity =
        serde_json::from_slice(data).map_err(|error| format!("同步实体解码失败：{error}"))?;
    entity.validate()?;
    Ok(entity)
}

pub fn validate_identifier(value: &str, field: &str) -> Result<(), String> {
    if value.is_empty()
        || value.chars().count() > MAX_IDENTIFIER_CHARS
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
    {
        return Err(format!(
            "{field} 只能包含安全标识字符，且长度不得超过 {MAX_IDENTIFIER_CHARS}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk_ref(hash_char: char) -> ChunkRef {
        ChunkRef {
            hash: std::iter::repeat(hash_char).take(64).collect(),
            plain_bytes: 1024,
            stored_bytes: 512,
            compressed: true,
        }
    }

    #[test]
    fn document_entity_round_trips_with_camel_case_fields() {
        let entity = SyncEntity::Document(DocumentPayload {
            id: "doc-1".to_string(),
            folder_id: "folder-1".to_string(),
            title: "标题".to_string(),
            branch_name: Some("目录链".to_string()),
            sort_order: 3,
            create_time: "2026-11-24T10:00:00".to_string(),
            update_time: "2026-11-24T10:00:00".to_string(),
            deleted: 0,
            is_locked: false,
            manifest: vec![chunk_ref('a')],
            content_plain_bytes: 1024,
        });
        let encoded = encode_entity(&entity).unwrap();
        let decoded = decode_entity(&encoded).unwrap();
        assert_eq!(decoded, entity);
        assert_eq!(entity.entity_id(), "doc:doc-1");
        assert!(String::from_utf8_lossy(&encoded).contains("\"folderId\""));
    }

    #[test]
    fn tombstone_entity_id_matches_its_target() {
        let entity = SyncEntity::Tombstone(TombstonePayload {
            entity_type: "folder".to_string(),
            id: "f-1".to_string(),
            deleted_at: "2026-11-24T10:00:00".to_string(),
        });
        assert_eq!(entity.entity_id(), "folder:f-1");
    }

    #[test]
    fn sync_data_requires_strictly_increasing_server_sequence() {
        let operation = |server_seq: i64| SyncPulledOperation {
            server_seq,
            op_id: format!("op-{server_seq}"),
            device_id: "device-1".to_string(),
            entity_id: "doc:1".to_string(),
            kind: OperationKind::Upsert,
            lamport: 1,
            ciphertext: base64url_stub(),
            nonce: base64url_stub_nonce(),
            created_at: 1,
        };
        let data = SyncData {
            push: SyncPushSummary {
                received: 0,
                inserted: 0,
                duplicates: 0,
            },
            pull: vec![operation(1), operation(2)],
            cursor: 2,
            has_more: false,
            server_time: 1,
        };
        assert!(data.validate().is_ok());

        let invalid = SyncData {
            pull: vec![operation(2), operation(1)],
            ..data
        };
        assert!(invalid.validate().is_err());
    }

    fn base64url_stub() -> String {
        crate::crypto::base64url_encode(&[7u8; 32])
    }

    fn base64url_stub_nonce() -> String {
        crate::crypto::base64url_encode(&[8u8; 12])
    }
}
