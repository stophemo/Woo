//! WebDAV 账号与同步身份的本地加密存储。
//!
//! 账号（含应用密码）和身份（含 vaultKey）分别写入独立的 AES-256-GCM
//! 加密文件。主密钥优先放在系统钥匙串；不支持钥匙串的平台退化为
//! 应用私有目录下 0600 的随机密钥文件。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use woo_sync_core::crypto::{
    aes256_gcm_open, aes256_gcm_seal, base64url_decode, base64url_encode, random_vault_key,
};
use woo_sync_core::wire::EncryptedEnvelope;

use crate::db;

const MASTER_KEY_BYTES: usize = 32;
const MASTER_KEY_FILE: &str = "webdav-master.key";
const KEYRING_SERVICE: &str = "com.nonegonotes.woo.tauri.webdav";
const KEYRING_USER: &str = "master-key-v1";
const FILE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EncryptedFile {
    version: u32,
    nonce: String,
    ciphertext: String,
}

fn master_key_path() -> PathBuf {
    db::data_dir().join(MASTER_KEY_FILE)
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension("tmp");
    {
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp)
            .map_err(|error| format!("写入本机同步密钥失败：{error}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
        }
        file.write_all(bytes)
            .map_err(|error| format!("写入本机同步密钥失败：{error}"))?;
        file.sync_all()
            .map_err(|error| format!("写入本机同步密钥失败：{error}"))?;
    }
    std::fs::rename(&temp, path).map_err(|error| {
        let _ = std::fs::remove_file(&temp);
        format!("保存本机同步密钥失败：{error}")
    })
}

fn read_key_file(path: &Path) -> Result<Option<[u8; MASTER_KEY_BYTES]>, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("读取本机同步密钥失败：{error}")),
    };
    <[u8; MASTER_KEY_BYTES]>::try_from(bytes.as_slice())
        .map(Some)
        .map_err(|_| "本机同步密钥长度无效".to_string())
}

fn file_master_key() -> Result<[u8; MASTER_KEY_BYTES], String> {
    let path = master_key_path();
    if let Some(key) = read_key_file(&path)? {
        return Ok(key);
    }
    let key = random_vault_key()?;
    write_private(&path, &key)?;
    Ok(key)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn keyring_master_key() -> Result<[u8; MASTER_KEY_BYTES], String> {
    use keyring::Entry;

    let entry = Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|error| format!("无法访问系统钥匙串：{error}"))?;
    match entry.get_password() {
        Ok(encoded) => {
            let bytes = base64url_decode(&encoded)
                .map_err(|_| "系统钥匙串中的同步密钥格式无效".to_string())?;
            <[u8; MASTER_KEY_BYTES]>::try_from(bytes.as_slice())
                .map_err(|_| "系统钥匙串中的同步密钥长度无效".to_string())
        }
        Err(keyring::Error::NoEntry) => {
            let key = random_vault_key()?;
            entry
                .set_password(&base64url_encode(&key))
                .map_err(|error| format!("写入系统钥匙串失败：{error}"))?;
            Ok(key)
        }
        Err(error) => Err(format!("读取系统钥匙串失败：{error}")),
    }
}

/// 读取本机主密钥；不存在时创建。钥匙串不可用时退回应用私有密钥文件。
fn load_master_key() -> Result<[u8; MASTER_KEY_BYTES], String> {
    // 一旦退化为文件密钥，就继续使用该文件，避免钥匙串恢复后换用另一把
    // 密钥，导致既有同步凭据无法解密。
    if let Some(key) = read_key_file(&master_key_path())? {
        return Ok(key);
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        match keyring_master_key() {
            Ok(key) => return Ok(key),
            Err(error) => {
                log::warn!(
                    "[WebDAV] 系统钥匙串不可用，改用应用私有密钥文件：{}",
                    error
                );
            }
        }
    }
    file_master_key()
}

pub fn open_file(path: &Path, purpose: &str) -> Result<Option<Vec<u8>>, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("读取同步凭据失败：{error}")),
    };
    let file: EncryptedFile = serde_json::from_slice(&bytes)
        .map_err(|_| "同步凭据文件格式损坏".to_string())?;
    if file.version != FILE_VERSION {
        return Err("同步凭据来自更新版本的应用，请先升级".to_string());
    }
    let key = load_master_key()?;
    let envelope = EncryptedEnvelope {
        ciphertext: file.ciphertext,
        nonce: file.nonce,
    };
    let aad = format!("woo-webdav-store-v1|{purpose}");
    let plaintext = aes256_gcm_open(&envelope, &key, aad.as_bytes()).map_err(|_| {
        "同步凭据无法解密：系统钥匙串可能已重置，请重新配置 WebDAV".to_string()
    })?;
    Ok(Some(plaintext.to_vec()))
}

pub fn save_file(path: &Path, purpose: &str, plaintext: &[u8]) -> Result<(), String> {
    let key = load_master_key()?;
    let aad = format!("woo-webdav-store-v1|{purpose}");
    let envelope = aes256_gcm_seal(plaintext, &key, None, aad.as_bytes())?;
    let file = EncryptedFile {
        version: FILE_VERSION,
        nonce: envelope.nonce,
        ciphertext: envelope.ciphertext,
    };
    let bytes = serde_json::to_vec_pretty(&file)
        .map_err(|error| format!("序列化同步凭据失败：{error}"))?;
    write_private(path, &bytes)
}

pub fn delete_file(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("删除同步凭据失败：{error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("woo-secure-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn encrypted_files_round_trip_and_bind_purpose() {
        let path = temp_path("account.enc.json");
        let key = random_vault_key().unwrap();
        let aad = b"woo-webdav-store-v1|account";
        let envelope =
            aes256_gcm_seal(br#"{"password":"secret"}"#, &key, None, aad).unwrap();
        let file = EncryptedFile {
            version: 1,
            nonce: envelope.nonce,
            ciphertext: envelope.ciphertext,
        };
        std::fs::write(&path, serde_json::to_vec(&file).unwrap()).unwrap();

        let raw = std::fs::read(&path).unwrap();
        assert!(!String::from_utf8_lossy(&raw).contains("secret"));

        let parsed: EncryptedFile = serde_json::from_slice(&raw).unwrap();
        let opened = aes256_gcm_open(
            &EncryptedEnvelope {
                ciphertext: parsed.ciphertext,
                nonce: parsed.nonce,
            },
            &key,
            aad,
        )
        .unwrap();
        assert_eq!(opened.as_slice(), br#"{"password":"secret"}"#);

        let parsed: EncryptedFile = serde_json::from_slice(&raw).unwrap();
        assert!(aes256_gcm_open(
            &EncryptedEnvelope {
                ciphertext: parsed.ciphertext,
                nonce: parsed.nonce,
            },
            &key,
            b"woo-webdav-store-v1|identity",
        )
        .is_err());

        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn key_file_is_created_with_private_permissions() {
        let path = temp_path("master.key");
        let key = random_vault_key().unwrap();
        write_private(&path, &key).unwrap();
        assert_eq!(read_key_file(&path).unwrap(), Some(key));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
