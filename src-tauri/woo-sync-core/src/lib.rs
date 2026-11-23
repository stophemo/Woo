//! Woo 笔记的端到端加密同步内核。
//!
//! 这个 crate 只包含与平台无关的协议、密码学和内容分块逻辑；
//! SQLite 存储、Tauri 命令与 UI 留在 `woo-tauri` 内。

pub mod chunker;
pub mod crypto;
pub mod wire;

pub use chunker::{chunk_bytes, chunk_content, verify_chunk_ref, Chunk};
pub use crypto::{
    aes256_gcm_open, aes256_gcm_seal, base64url_decode, base64url_encode,
    pairing_verification_code, random_bytes, sync_aad_canonical, PairingKeyPair,
};
pub use wire::{
    ChunkRef, EncryptedEnvelope, OperationKind, SyncData, SyncPushOperation,
    SyncPushSummary, SyncPulledOperation, SyncRequest,
};
