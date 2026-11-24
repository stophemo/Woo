//! 同步身份：vaultId、deviceId 与 32 字节 vaultKey。
//!
//! 身份与 WebDAV 账号分开加密保存；vaultKey 只在本机和解密后的内存中
//! 出现，不进入日志或远端明文。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use woo_sync_core::crypto::{base64url_decode, base64url_encode, random_vault_key};
use woo_sync_core::wire::validate_identifier;

use crate::db;
use crate::services::utils::sha256;
use crate::services::vault_sync::config::{default_device_name, platform_name};

use super::secure;

const IDENTITY_FILE: &str = "webdav-identity.enc.json";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WebDavIdentity {
    pub vault_id: String,
    pub device_id: String,
    /// Base64URL 编码的 32 字节 vaultKey。
    pub vault_key: String,
    pub device_name: String,
    pub platform: String,
    pub bound_user: Option<String>,
}

impl Default for WebDavIdentity {
    fn default() -> Self {
        Self {
            vault_id: String::new(),
            device_id: String::new(),
            vault_key: String::new(),
            device_name: default_device_name(),
            platform: platform_name().to_string(),
            bound_user: None,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavIdentityView {
    pub configured: bool,
    pub active: bool,
    pub vault_id: String,
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub space_fingerprint: String,
}

pub fn identity_path() -> PathBuf {
    db::data_dir().join(IDENTITY_FILE)
}

pub fn space_fingerprint(vault_id: &str) -> String {
    let digest = sha256(&format!("woo-space-id-v1|{vault_id}"));
    digest.chars().take(8).collect::<String>().to_uppercase()
}

pub fn sanitize_device_name(value: &str) -> String {
    let trimmed = value.trim();
    let cleaned: String = trimmed.chars().take(80).collect();
    if cleaned.is_empty() {
        default_device_name()
    } else {
        cleaned
    }
}

pub fn create_identity(device_name: &str) -> Result<WebDavIdentity, String> {
    let vault_key = random_vault_key()?;
    Ok(WebDavIdentity {
        vault_id: uuid::Uuid::new_v4().to_string(),
        device_id: uuid::Uuid::new_v4().to_string(),
        vault_key: base64url_encode(&vault_key),
        device_name: sanitize_device_name(device_name),
        platform: platform_name().to_string(),
        bound_user: db::current_user_key(),
    })
}

pub fn decode_vault_key(identity: &WebDavIdentity) -> Result<[u8; 32], String> {
    let bytes = base64url_decode(&identity.vault_key)
        .map_err(|_| "同步空间密钥编码无效".to_string())?;
    <[u8; 32]>::try_from(bytes.as_slice())
        .map_err(|_| "同步空间密钥长度无效".to_string())
}

pub fn identity_is_valid(identity: &WebDavIdentity) -> bool {
    !identity.vault_id.is_empty()
        && !identity.device_id.is_empty()
        && validate_identifier(&identity.vault_id, "vaultId").is_ok()
        && validate_identifier(&identity.device_id, "deviceId").is_ok()
        && decode_vault_key(identity).is_ok()
}

pub fn load_identity() -> Result<Option<WebDavIdentity>, String> {
    let Some(bytes) = secure::open_file(&identity_path(), "identity")? else {
        return Ok(None);
    };
    let identity: WebDavIdentity = serde_json::from_slice(&bytes)
        .map_err(|_| "同步身份配置格式损坏".to_string())?;
    if !identity_is_valid(&identity) {
        return Err("同步身份配置无效，请重新加入同步空间".to_string());
    }
    Ok(Some(identity))
}

pub fn configured_identity() -> Result<Option<WebDavIdentity>, String> {
    let identity = load_identity()?;
    Ok(identity.filter(identity_is_active))
}

pub fn identity_is_active(identity: &WebDavIdentity) -> bool {
    identity.bound_user == db::current_user_key() && identity_is_valid(identity)
}

pub fn identity_view(identity: Option<&WebDavIdentity>) -> WebDavIdentityView {
    match identity {
        Some(identity) => WebDavIdentityView {
            configured: true,
            active: identity_is_active(identity),
            vault_id: identity.vault_id.clone(),
            device_id: identity.device_id.clone(),
            device_name: identity.device_name.clone(),
            platform: identity.platform.clone(),
            space_fingerprint: space_fingerprint(&identity.vault_id),
        },
        None => WebDavIdentityView {
            configured: false,
            active: false,
            vault_id: String::new(),
            device_id: String::new(),
            device_name: default_device_name(),
            platform: platform_name().to_string(),
            space_fingerprint: String::new(),
        },
    }
}

pub fn save_identity(identity: &WebDavIdentity) -> Result<(), String> {
    if !identity_is_valid(identity) {
        return Err("拒绝保存无效的同步身份".to_string());
    }
    let bytes = serde_json::to_vec(identity)
        .map_err(|error| format!("序列化同步身份失败：{error}"))?;
    secure::save_file(&identity_path(), "identity", &bytes)?;
    log::info!(
        "[WebDAV] 同步身份已保存 (vault={}, device={})",
        identity.vault_id,
        identity.device_id
    );
    Ok(())
}

pub fn delete_identity() -> Result<(), String> {
    secure::delete_file(&identity_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_identity_is_valid_and_fingerprint_is_stable() {
        let identity = create_identity("测试设备").unwrap();
        assert!(identity_is_valid(&identity));
        assert_eq!(decode_vault_key(&identity).unwrap().len(), 32);
        assert_eq!(
            space_fingerprint(&identity.vault_id),
            space_fingerprint(&identity.vault_id)
        );
        assert_eq!(space_fingerprint(&identity.vault_id).len(), 8);
        assert_ne!(
            space_fingerprint(&identity.vault_id),
            space_fingerprint("another-vault")
        );
    }

    #[test]
    fn rejects_invalid_key_length() {
        let mut identity = create_identity("测试设备").unwrap();
        identity.vault_key = base64url_encode(b"short");
        assert!(!identity_is_valid(&identity));
    }
}
