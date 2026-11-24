//! WebDAV 设备心跳与只读设备列表。
//!
//! 每台设备同步时把加密心跳写到 `v1/<vaultId>/devices/<deviceId>.json`。
//! 外层只有格式、版本、deviceId、nonce 和密文；设备名、平台、最近同步时间
//! 和应用版本都在 ciphertext 中。WebDAV 供应商只能看到设备数量和时间。

use std::collections::HashSet;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use woo_sync_core::crypto::{
    aes256_gcm_open, aes256_gcm_seal, base64url_decode, AES256_NONCE_BYTES,
    AES256_TAG_BYTES,
};
use woo_sync_core::wire::EncryptedEnvelope;

use super::client::{ensure_collection, RemoteStore};
use super::identity::{decode_vault_key, WebDavIdentity};

pub const FORMAT: &str = "woo-notes-webdav-device";
pub const PROTOCOL_VERSION: u32 = 1;
const AAD_PREFIX: &str = "woo-notes-webdav-device-v1";
const MAX_NAME_CHARS: usize = 80;
const MAX_APP_VERSION_CHARS: usize = 32;

/// 设备目录创建成功后在进程内缓存，减少 WebDAV 请求数。
static READY_DEVICE_ROOTS: Lazy<Mutex<HashSet<String>>> =
    Lazy::new(|| Mutex::new(HashSet::new()));

async fn ensure_devices_root(
    store: &dyn RemoteStore,
    vault_id: &str,
) -> Result<String, String> {
    let root = devices_root(vault_id);
    if READY_DEVICE_ROOTS
        .lock()
        .map(|set| set.contains(&root))
        .unwrap_or(false)
    {
        return Ok(root);
    }
    ensure_collection(store, &root)
        .await
        .map_err(|error| error.to_string())?;
    if let Ok(mut set) = READY_DEVICE_ROOTS.lock() {
        set.insert(root.clone());
    }
    Ok(root)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeviceEnvelope {
    format: String,
    protocol_version: u32,
    device_id: String,
    nonce: String,
    ciphertext: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeviceHeartbeat {
    name: String,
    platform: String,
    last_seen_at: i64,
    app_version: String,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub last_seen_at: i64,
    pub app_version: String,
    pub is_current: bool,
}

pub fn devices_root(vault_id: &str) -> String {
    format!("v1/{vault_id}/devices")
}

fn device_path(vault_id: &str, device_id: &str) -> String {
    format!("{}/{}.json", devices_root(vault_id), device_id)
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn aad(vault_id: &str, device_id: &str) -> String {
    format!("{AAD_PREFIX}|{vault_id}|{device_id}")
}

fn valid_device_id(value: &str) -> bool {
    let count = value.chars().count();
    (8..=128).contains(&count)
        && value
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphanumeric())
        && value.chars().all(|ch| {
            ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | ':' | '-')
        })
}

fn valid_platform(value: &str) -> bool {
    matches!(
        value,
        "macos" | "windows" | "linux" | "android" | "ios"
    )
}

fn validate_heartbeat(heartbeat: &DeviceHeartbeat) -> Result<(), String> {
    let name_len = heartbeat.name.chars().count();
    if name_len == 0
        || name_len > MAX_NAME_CHARS
        || heartbeat.name.chars().any(char::is_control)
    {
        return Err("设备心跳名称无效".to_string());
    }
    if !valid_platform(&heartbeat.platform) {
        return Err("设备心跳平台无效".to_string());
    }
    if heartbeat.last_seen_at <= 0 {
        return Err("设备心跳时间无效".to_string());
    }
    let version_len = heartbeat.app_version.chars().count();
    if version_len == 0
        || version_len > MAX_APP_VERSION_CHARS
        || heartbeat.app_version.chars().any(char::is_control)
    {
        return Err("设备心跳版本无效".to_string());
    }
    Ok(())
}

fn validate_envelope(envelope: &DeviceEnvelope, expected_device_id: &str) -> Result<(), String> {
    if envelope.format != FORMAT {
        return Err("设备心跳格式不支持".to_string());
    }
    if envelope.protocol_version != PROTOCOL_VERSION {
        return Err("设备心跳协议版本不支持".to_string());
    }
    if envelope.device_id != expected_device_id || !valid_device_id(&envelope.device_id) {
        return Err("设备心跳 deviceId 与文件名不一致".to_string());
    }
    let nonce = base64url_decode(&envelope.nonce)
        .map_err(|_| "设备心跳 nonce 无效".to_string())?;
    if nonce.len() != AES256_NONCE_BYTES {
        return Err("设备心跳 nonce 长度无效".to_string());
    }
    let ciphertext = base64url_decode(&envelope.ciphertext)
        .map_err(|_| "设备心跳密文无效".to_string())?;
    if ciphertext.len() < AES256_TAG_BYTES || ciphertext.len() > 32 * 1024 {
        return Err("设备心跳密文长度无效".to_string());
    }
    Ok(())
}

fn decode_envelope(
    bytes: &[u8],
    vault_id: &str,
    device_id: &str,
    vault_key: &[u8; 32],
) -> Result<DeviceView, String> {
    let envelope: DeviceEnvelope = serde_json::from_slice(bytes)
        .map_err(|_| "设备心跳信封格式损坏".to_string())?;
    validate_envelope(&envelope, device_id)?;
    let plaintext = aes256_gcm_open(
        &EncryptedEnvelope {
            ciphertext: envelope.ciphertext,
            nonce: envelope.nonce,
        },
        vault_key,
        aad(vault_id, device_id).as_bytes(),
    )
    .map_err(|_| "设备心跳无法解密".to_string())?;
    let heartbeat: DeviceHeartbeat = serde_json::from_slice(&plaintext)
        .map_err(|_| "设备心跳明文格式损坏".to_string())?;
    validate_heartbeat(&heartbeat)?;
    Ok(DeviceView {
        device_id: device_id.to_string(),
        name: heartbeat.name,
        platform: heartbeat.platform,
        last_seen_at: heartbeat.last_seen_at,
        app_version: heartbeat.app_version,
        is_current: false,
    })
}

fn device_id_from_file_name(name: &str) -> Option<String> {
    let device_id = name.strip_suffix(".json")?;
    valid_device_id(device_id).then(|| device_id.to_string())
}

/// 写入本机心跳。失败只应告警，不应阻塞同步。
pub async fn write_heartbeat(
    store: &dyn RemoteStore,
    identity: &WebDavIdentity,
) -> Result<(), String> {
    let vault_key = decode_vault_key(identity)?;
    let heartbeat = DeviceHeartbeat {
        name: identity.device_name.clone(),
        platform: identity.platform.clone(),
        last_seen_at: now_ms(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    validate_heartbeat(&heartbeat)?;
    let plaintext = serde_json::to_vec(&heartbeat)
        .map_err(|error| format!("序列化设备心跳失败：{error}"))?;
    let sealed = aes256_gcm_seal(
        &plaintext,
        &vault_key,
        None,
        aad(&identity.vault_id, &identity.device_id).as_bytes(),
    )?;
    let envelope = DeviceEnvelope {
        format: FORMAT.to_string(),
        protocol_version: PROTOCOL_VERSION,
        device_id: identity.device_id.clone(),
        nonce: sealed.nonce,
        ciphertext: sealed.ciphertext,
    };
    let body = serde_json::to_vec(&envelope)
        .map_err(|error| format!("序列化设备心跳信封失败：{error}"))?;
    let root = ensure_devices_root(store, &identity.vault_id).await?;
    let path = device_path(&identity.vault_id, &identity.device_id);
    match store.put(&path, &body, false).await {
        Ok(()) => Ok(()),
        Err(super::client::RemoteError::Conflict) => {
            // 父目录可能在远端被删除；清缓存后重试一次。
            if let Ok(mut set) = READY_DEVICE_ROOTS.lock() {
                set.remove(&root);
            }
            ensure_devices_root(store, &identity.vault_id).await?;
            store
                .put(&path, &body, false)
                .await
                .map_err(|error| error.to_string())
        }
        Err(error) => Err(error.to_string()),
    }
}

/// 列出设备心跳。坏文件、未知版本、文件名不匹配的文件只跳过，不删除。
pub async fn list_devices(
    store: &dyn RemoteStore,
    identity: &WebDavIdentity,
) -> Result<Vec<DeviceView>, String> {
    let vault_key = decode_vault_key(identity)?;
    let root = devices_root(&identity.vault_id);
    // 只读列表不创建远端目录；目录不存在时按空列表处理。
    let entries = store.list(&root).await.map_err(|error| error.to_string())?;
    let mut devices = Vec::new();
    for entry in entries {
        if entry.is_dir {
            continue;
        }
        let Some(device_id) = device_id_from_file_name(&entry.name) else {
            log::warn!("[WebDAV] 跳过无效设备心跳文件名：{}", entry.name);
            continue;
        };
        let path = format!("{root}/{}", entry.name);
        let bytes = match store.get(&path).await {
            Ok(Some(bytes)) => bytes,
            Ok(None) => continue,
            Err(error) => {
                log::warn!("[WebDAV] 跳过无法读取的设备心跳 {}：{}", entry.name, error);
                continue;
            }
        };
        match decode_envelope(&bytes, &identity.vault_id, &device_id, &vault_key) {
            Ok(mut device) => {
                device.is_current = device.device_id == identity.device_id;
                devices.push(device);
            }
            Err(error) => {
                log::warn!("[WebDAV] 跳过无效设备心跳 {}：{}", entry.name, error);
            }
        }
    }
    devices.sort_by(|left, right| {
        right
            .last_seen_at
            .cmp(&left.last_seen_at)
            .then_with(|| left.device_id.cmp(&right.device_id))
    });
    Ok(devices)
}

/// 退出空间时尽力删除自己的心跳；不删除其他设备的心跳。
pub async fn delete_own_heartbeat(
    store: &dyn RemoteStore,
    identity: &WebDavIdentity,
) -> Result<(), String> {
    store
        .delete(&device_path(&identity.vault_id, &identity.device_id))
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::webdav_space::client::test_support::MemoryRemote;
    use crate::services::webdav_space::identity::create_identity;

    fn sample_identity() -> (WebDavIdentity, [u8; 32]) {
        let identity = create_identity("测试设备").unwrap();
        let key = decode_vault_key(&identity).unwrap();
        (identity, key)
    }

    #[tokio::test]
    async fn writes_and_lists_devices_sorted_by_last_seen() {
        let remote = MemoryRemote::new();
        let (first, _) = sample_identity();
        let mut second = create_identity("第二台设备").unwrap();
        second.vault_id = first.vault_id.clone();
        second.platform = "android".to_string();

        write_heartbeat(&remote, &first).await.unwrap();
        // 手动把第二台设备的时间调早，验证排序。
        let key = decode_vault_key(&first).unwrap();
        let heartbeat = DeviceHeartbeat {
            name: second.device_name.clone(),
            platform: second.platform.clone(),
            last_seen_at: now_ms() - 60_000,
            app_version: "0.9.1".to_string(),
        };
        let plaintext = serde_json::to_vec(&heartbeat).unwrap();
        let sealed = aes256_gcm_seal(
            &plaintext,
            &key,
            None,
            aad(&second.vault_id, &second.device_id).as_bytes(),
        )
        .unwrap();
        let envelope = DeviceEnvelope {
            format: FORMAT.to_string(),
            protocol_version: PROTOCOL_VERSION,
            device_id: second.device_id.clone(),
            nonce: sealed.nonce,
            ciphertext: sealed.ciphertext,
        };
        remote.insert_file(
            &device_path(&second.vault_id, &second.device_id),
            &serde_json::to_vec(&envelope).unwrap(),
        );

        let devices = list_devices(&remote, &first).await.unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].device_id, first.device_id);
        assert!(devices[0].is_current);
        assert!(!devices[1].is_current);
        assert_eq!(devices[1].name, "第二台设备");
    }

    #[tokio::test]
    async fn skips_bad_unknown_and_mismatched_heartbeats() {
        let remote = MemoryRemote::new();
        let (identity, key) = sample_identity();
        let root = devices_root(&identity.vault_id);

        // 合法心跳。
        write_heartbeat(&remote, &identity).await.unwrap();

        // 未知版本。
        let unknown = DeviceEnvelope {
            format: FORMAT.to_string(),
            protocol_version: 2,
            device_id: "device-unknown".to_string(),
            nonce: "AAAAAAAAAAAAAAAA".to_string(),
            ciphertext: "AAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
        };
        remote.insert_file(
            &format!("{root}/device-unknown.json"),
            &serde_json::to_vec(&unknown).unwrap(),
        );

        // deviceId 与文件名不匹配。
        let heartbeat = DeviceHeartbeat {
            name: "mismatch".to_string(),
            platform: "macos".to_string(),
            last_seen_at: now_ms(),
            app_version: "0.9.1".to_string(),
        };
        let sealed = aes256_gcm_seal(
            &serde_json::to_vec(&heartbeat).unwrap(),
            &key,
            None,
            aad(&identity.vault_id, "device-actual").as_bytes(),
        )
        .unwrap();
        let mismatch = DeviceEnvelope {
            format: FORMAT.to_string(),
            protocol_version: PROTOCOL_VERSION,
            device_id: "device-actual".to_string(),
            nonce: sealed.nonce,
            ciphertext: sealed.ciphertext,
        };
        remote.insert_file(
            &format!("{root}/device-other.json"),
            &serde_json::to_vec(&mismatch).unwrap(),
        );

        // 损坏 JSON。
        remote.insert_file(&format!("{root}/bad.json"), b"{not-json");

        let devices = list_devices(&remote, &identity).await.unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device_id, identity.device_id);
    }

    #[tokio::test]
    async fn aad_tampering_is_rejected() {
        let (identity, key) = sample_identity();
        let heartbeat = DeviceHeartbeat {
            name: "tamper".to_string(),
            platform: "macos".to_string(),
            last_seen_at: now_ms(),
            app_version: "0.9.1".to_string(),
        };
        let sealed = aes256_gcm_seal(
            &serde_json::to_vec(&heartbeat).unwrap(),
            &key,
            None,
            aad(&identity.vault_id, "other-device").as_bytes(),
        )
        .unwrap();
        let envelope = DeviceEnvelope {
            format: FORMAT.to_string(),
            protocol_version: PROTOCOL_VERSION,
            device_id: identity.device_id.clone(),
            nonce: sealed.nonce,
            ciphertext: sealed.ciphertext,
        };
        let bytes = serde_json::to_vec(&envelope).unwrap();
        assert!(decode_envelope(&bytes, &identity.vault_id, &identity.device_id, &key).is_err());
    }

    #[tokio::test]
    async fn empty_directory_returns_empty_list() {
        let remote = MemoryRemote::new();
        let (identity, _) = sample_identity();
        assert!(list_devices(&remote, &identity).await.unwrap().is_empty());
        assert!(!remote
            .dirs
            .lock()
            .unwrap()
            .contains(&devices_root(&identity.vault_id)));
    }
}
