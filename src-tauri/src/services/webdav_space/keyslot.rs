//! 用 WebDAV 应用密码派生密钥，包裹 vaultKey 写入 `space.json`。
//!
//! 这样每台设备只要填写同一个 WebDAV 账号就能加入空间，不再需要六位码配对。
//! 安全边界：云端仍是 AES-256-GCM 密文，但持有 WebDAV 应用密码的人可以解开
//! vaultKey；需要更强隔离时关闭密钥槽，改用设备配对。

use serde::{Deserialize, Serialize};
use woo_sync_core::crypto::{
    aes256_gcm_open, aes256_gcm_seal, base64url_decode, base64url_encode, random_bytes,
};
use woo_sync_core::wire::EncryptedEnvelope;

pub const KDF_NAME: &str = "pbkdf2-sha256";
/// 移动端也能接受的迭代次数；改高会让每次加入多等几百毫秒。
pub const KDF_ITERATIONS: u32 = 210_000;
const MIN_ITERATIONS: u32 = 50_000;
const MAX_ITERATIONS: u32 = 1_000_000;
const SALT_BYTES: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeySlot {
    pub kdf: String,
    pub iterations: u32,
    pub salt: String,
    pub wrapped: EncryptedEnvelope,
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    const BLOCK_SIZE: usize = 64;
    let mut block = [0u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        let digest = Sha256::digest(key);
        block[..32].copy_from_slice(&digest);
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK_SIZE];
    let mut opad = [0x5cu8; BLOCK_SIZE];
    for index in 0..BLOCK_SIZE {
        ipad[index] ^= block[index];
        opad[index] ^= block[index];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(data);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_digest);
    let mut result = [0u8; 32];
    result.copy_from_slice(&outer.finalize());
    result
}

/// PBKDF2-HMAC-SHA256，dkLen 固定 32 字节（单个块）。
fn derive(password: &str, salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut salted = salt.to_vec();
    salted.extend_from_slice(&1u32.to_be_bytes());
    let mut block = hmac_sha256(password.as_bytes(), &salted);
    let mut out = block;
    for _ in 1..iterations {
        block = hmac_sha256(password.as_bytes(), &block);
        for (dst, src) in out.iter_mut().zip(block.iter()) {
            *dst ^= *src;
        }
    }
    out
}

fn aad(vault_id: &str, salt: &str, iterations: u32) -> Vec<u8> {
    format!("woo-notes-key-slot-v1|{vault_id}|{salt}|{iterations}").into_bytes()
}

/// 用应用密码包裹 vaultKey。密码只用于派生，不写入云端。
pub fn create(vault_key: &[u8; 32], vault_id: &str, password: &str) -> Result<KeySlot, String> {
    if password.is_empty() {
        return Err("请填写 WebDAV 应用密码".to_string());
    }
    let salt = random_bytes::<SALT_BYTES>()?;
    let salt_b64 = base64url_encode(&salt);
    let key = derive(password, &salt, KDF_ITERATIONS);
    let wrapped = aes256_gcm_seal(
        vault_key,
        &key,
        None,
        &aad(vault_id, &salt_b64, KDF_ITERATIONS),
    )?;
    Ok(KeySlot {
        kdf: KDF_NAME.to_string(),
        iterations: KDF_ITERATIONS,
        salt: salt_b64,
        wrapped,
    })
}

/// 用应用密码解出 vaultKey；密码不对或云端 slot 被替换时返回错误。
pub fn unwrap_key(slot: &KeySlot, vault_id: &str, password: &str) -> Result<[u8; 32], String> {
    if slot.kdf != KDF_NAME {
        return Err("云端密钥槽使用了不支持的派生方式".to_string());
    }
    if slot.iterations < MIN_ITERATIONS || slot.iterations > MAX_ITERATIONS {
        return Err("云端密钥槽参数无效".to_string());
    }
    let salt = base64url_decode(&slot.salt).map_err(|_| "云端密钥槽盐值无效".to_string())?;
    let key = derive(password, &salt, slot.iterations);
    let plain = aes256_gcm_open(
        &slot.wrapped,
        &key,
        &aad(vault_id, &slot.salt, slot.iterations),
    )
    .map_err(|_| "应用密码不匹配，或云端同步密钥已更新".to_string())?;
    <[u8; 32]>::try_from(plain.as_slice()).map_err(|_| "云端同步密钥长度无效".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex32(value: &str) -> [u8; 32] {
        let mut out = [0u8; 32];
        for index in 0..32 {
            out[index] = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap();
        }
        out
    }

    #[test]
    fn matches_pbkdf2_sha256_test_vectors() {
        assert_eq!(
            derive("password", b"salt", 1),
            hex32("120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b")
        );
        assert_eq!(
            derive("password", b"salt", 2),
            hex32("ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43")
        );
    }

    #[test]
    fn round_trip_with_same_password() {
        let vault_key = [7u8; 32];
        let slot = create(&vault_key, "vault-1", "app-password").unwrap();
        assert_eq!(
            unwrap_key(&slot, "vault-1", "app-password").unwrap(),
            vault_key
        );
    }

    #[test]
    fn wrong_password_or_vault_is_rejected() {
        let slot = create(&[7u8; 32], "vault-1", "app-password").unwrap();
        assert!(unwrap_key(&slot, "vault-1", "wrong").is_err());
        assert!(unwrap_key(&slot, "vault-2", "app-password").is_err());
    }

    #[test]
    fn tampered_slot_is_rejected() {
        let mut slot = create(&[7u8; 32], "vault-1", "app-password").unwrap();
        slot.wrapped.ciphertext = base64url_encode(&[0u8; 40]);
        assert!(unwrap_key(&slot, "vault-1", "app-password").is_err());
    }
}
