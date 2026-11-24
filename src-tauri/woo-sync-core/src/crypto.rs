use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::wire::EncryptedEnvelope;

pub const AES256_KEY_BYTES: usize = 32;
pub const AES256_NONCE_BYTES: usize = 12;
pub const AES256_TAG_BYTES: usize = 16;

const SYNC_AAD_PREFIX: &str = "woo-notes-sync-v1";
const CHUNK_AAD_PREFIX: &str = "woo-notes-chunk-v1";
const PAIRING_AAD_PREFIX: &str = "woo-notes-pair-v1";
const PAIRING_HKDF_PREFIX: &str = "woo-notes-pairing-v1";
const PAIRING_CODE_PREFIX: &str = "woo-notes-pairing-code-v1";

pub type SyncResult<T> = Result<T, String>;

pub fn base64url_encode(data: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

pub fn base64url_decode(source: &str) -> SyncResult<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(source)
        .map_err(|_| "Base64URL 编码无效".to_string())
}

pub fn random_bytes<const N: usize>() -> SyncResult<[u8; N]> {
    let mut bytes = [0u8; N];
    getrandom::getrandom(&mut bytes).map_err(|error| format!("随机数生成失败：{error}"))?;
    Ok(bytes)
}

/// 同步操作正文的 AAD。字段顺序和分隔符必须与客户端、服务端和其他平台一致。
pub fn sync_aad_canonical(
    vault_id: &str,
    op_id: &str,
    entity_id: &str,
    kind: &str,
    lamport: i64,
    device_id: &str,
) -> String {
    format!(
        "{SYNC_AAD_PREFIX}|{vault_id}|{op_id}|{entity_id}|{kind}|{lamport}|{device_id}"
    )
}

/// 内容块使用独立 AAD，不包含 deviceId。这样两台设备推送同一个内容块时
/// 会得到完全相同的 opId/lamport/nonce/密文，服务端可直接按重复操作处理。
pub fn chunk_aad_canonical(vault_id: &str, entity_id: &str, lamport: i64) -> String {
    format!("{CHUNK_AAD_PREFIX}|{vault_id}|{entity_id}|{lamport}")
}

/// 从块哈希确定性地派生 lamport 和 AES-GCM nonce。
///
/// 同一块明文在同一 vault key 下总是得到相同密文；不同块使用不同 nonce。
/// 这不是通用加密建议，只适用于内容寻址且明文完全相同的块。
pub fn chunk_lamport_and_nonce(hash_hex: &str) -> SyncResult<(i64, [u8; AES256_NONCE_BYTES])> {
    let mut hasher = Sha256::new();
    hasher.update(CHUNK_AAD_PREFIX.as_bytes());
    hasher.update(b"|");
    hasher.update(hash_hex.as_bytes());
    let digest = hasher.finalize();
    let raw = u64::from_be_bytes(
        digest[..8]
            .try_into()
            .expect("SHA-256 输出至少 8 字节"),
    );
    // 保留 63 位，确保是正数且不超过 i64::MAX。
    let lamport = ((raw & 0x7fff_ffff_ffff_ffff) as i64).max(1);
    let mut nonce = [0u8; AES256_NONCE_BYTES];
    nonce.copy_from_slice(&digest[8..20]);
    Ok((lamport, nonce))
}

pub fn pairing_envelope_aad(pairing_id: &str, claimed_device_id: &str) -> String {
    format!("{PAIRING_AAD_PREFIX}|{pairing_id}|{claimed_device_id}")
}

pub fn pairing_hkdf_info(pairing_id: &str) -> Vec<u8> {
    format!("{PAIRING_HKDF_PREFIX}|{pairing_id}").into_bytes()
}

pub fn aes256_gcm_seal(
    plaintext: &[u8],
    key: &[u8; AES256_KEY_BYTES],
    nonce: Option<&[u8]>,
    aad: &[u8],
) -> SyncResult<EncryptedEnvelope> {
    let nonce_bytes = match nonce {
        Some(value) if value.len() == AES256_NONCE_BYTES => value.to_vec(),
        Some(_) => return Err("AES-GCM nonce 必须是 12 字节".to_string()),
        None => random_bytes::<AES256_NONCE_BYTES>()?.to_vec(),
    };
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| "AES-256-GCM 密钥无效".to_string())?;
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| "加密失败".to_string())?;
    Ok(EncryptedEnvelope {
        ciphertext: base64url_encode(&ciphertext),
        nonce: base64url_encode(&nonce_bytes),
    })
}

pub fn aes256_gcm_open(
    envelope: &EncryptedEnvelope,
    key: &[u8; AES256_KEY_BYTES],
    aad: &[u8],
) -> SyncResult<Zeroizing<Vec<u8>>> {
    let nonce_bytes = base64url_decode(&envelope.nonce)?;
    if nonce_bytes.len() != AES256_NONCE_BYTES {
        return Err("AES-GCM nonce 必须是 12 字节".to_string());
    }
    let ciphertext = base64url_decode(&envelope.ciphertext)?;
    if ciphertext.len() < AES256_TAG_BYTES {
        return Err("密文长度不足".to_string());
    }
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| "AES-256-GCM 密钥无效".to_string())?;
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: &ciphertext,
                aad,
            },
        )
        .map_err(|_| "密文解密失败：密钥或 AAD 不匹配".to_string())?;
    Ok(Zeroizing::new(plaintext))
}

/// 配对期间使用的临时 X25519 密钥对。私钥只在内存中存在。
pub struct PairingKeyPair {
    secret: StaticSecret,
}

impl Clone for PairingKeyPair {
    fn clone(&self) -> Self {
        Self::from_private_key(self.private_key_bytes())
    }
}

impl PairingKeyPair {
    pub fn generate() -> SyncResult<Self> {
        let bytes = random_bytes::<32>()?;
        Ok(Self {
            secret: StaticSecret::from(bytes),
        })
    }

    pub fn from_private_key(bytes: [u8; 32]) -> Self {
        Self {
            secret: StaticSecret::from(bytes),
        }
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        PublicKey::from(&self.secret).to_bytes()
    }

    /// 仅用于在当前进程内重建临时配对密钥；调用方不得持久化。
    pub fn private_key_bytes(&self) -> [u8; 32] {
        self.secret.to_bytes()
    }

    pub fn public_key_base64url(&self) -> String {
        base64url_encode(&self.public_key_bytes())
    }

    pub fn shared_secret(&self, peer_public_key: &[u8]) -> SyncResult<[u8; 32]> {
        let peer = <[u8; 32]>::try_from(peer_public_key)
            .map_err(|_| "对端 X25519 公钥必须是 32 字节".to_string())?;
        Ok(self.secret.diffie_hellman(&PublicKey::from(peer)).to_bytes())
    }

    /// 用 X25519 共享密钥 + pairingSecret 作为 salt 派生 32 字节会话密钥。
    pub fn session_key(
        &self,
        pairing_secret: &[u8],
        pairing_id: &str,
        peer_public_key: &[u8],
    ) -> SyncResult<[u8; 32]> {
        let shared = self.shared_secret(peer_public_key)?;
        let hkdf = Hkdf::<Sha256>::new(Some(pairing_secret), &shared);
        let mut output = [0u8; 32];
        hkdf.expand(&pairing_hkdf_info(pairing_id), &mut output)
            .map_err(|_| "配对会话密钥派生失败".to_string())?;
        Ok(output)
    }
}

pub fn pairing_verification_input(
    initiator_public_key: &[u8],
    claimed_public_key: &[u8],
) -> String {
    format!(
        "{PAIRING_CODE_PREFIX}|{}|{}",
        base64url_encode(initiator_public_key),
        base64url_encode(claimed_public_key)
    )
}

/// 两端各自计算、由用户人工比对的六位核对码。
pub fn pairing_verification_code(
    session_key: &[u8; 32],
    initiator_public_key: &[u8],
    claimed_public_key: &[u8],
) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(session_key)
        .expect("HMAC 接受任意长度密钥");
    mac.update(
        pairing_verification_input(initiator_public_key, claimed_public_key).as_bytes(),
    );
    let digest = mac.finalize().into_bytes();
    let value = u64::from_be_bytes(
        digest[..8]
            .try_into()
            .expect("HMAC-SHA256 输出至少 8 字节"),
    );
    format!("{:06}", value % 1_000_000)
}

pub fn seal_pairing_vault_key(
    vault_key: &[u8; AES256_KEY_BYTES],
    session_key: &[u8; 32],
    pairing_id: &str,
    claimed_device_id: &str,
) -> SyncResult<EncryptedEnvelope> {
    let aad = pairing_envelope_aad(pairing_id, claimed_device_id);
    aes256_gcm_seal(vault_key, session_key, None, aad.as_bytes())
}

pub fn open_pairing_vault_key(
    envelope: &EncryptedEnvelope,
    session_key: &[u8; 32],
    pairing_id: &str,
    claimed_device_id: &str,
) -> SyncResult<[u8; AES256_KEY_BYTES]> {
    let aad = pairing_envelope_aad(pairing_id, claimed_device_id);
    let plaintext = aes256_gcm_open(envelope, session_key, aad.as_bytes())?;
    <[u8; AES256_KEY_BYTES]>::try_from(plaintext.as_slice())
        .map_err(|_| "配对返回的 vault key 长度不是 32 字节".to_string())
}

pub fn random_vault_key() -> SyncResult<[u8; AES256_KEY_BYTES]> {
    random_bytes::<AES256_KEY_BYTES>()
}

pub fn random_device_token() -> SyncResult<String> {
    Ok(base64url_encode(&random_bytes::<32>()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aes_gcm_round_trip_and_aad_binding() {
        let key = random_vault_key().unwrap();
        let envelope = aes256_gcm_seal(
            b"<p>hello</p>",
            &key,
            None,
            b"aad-v1",
        )
        .unwrap();
        let opened = aes256_gcm_open(&envelope, &key, b"aad-v1").unwrap();
        assert_eq!(opened.as_slice(), b"<p>hello</p>");
        assert!(aes256_gcm_open(&envelope, &key, b"aad-v2").is_err());
    }

    #[test]
    fn base64url_is_unpadded_and_strict() {
        let encoded = base64url_encode(b"woo");
        assert_eq!(encoded, "d29v");
        assert_eq!(base64url_decode(&encoded).unwrap(), b"woo");
        assert!(base64url_decode("d29v=").is_err());
    }

    #[test]
    fn pairing_derives_matching_session_keys_and_codes() {
        let initiator = PairingKeyPair::generate().unwrap();
        let joiner = PairingKeyPair::generate().unwrap();
        let pairing_secret = random_bytes::<32>().unwrap();
        let pairing_id = "pairing-test";
        let initiator_key = initiator
            .session_key(&pairing_secret, pairing_id, &joiner.public_key_bytes())
            .unwrap();
        let joiner_key = joiner
            .session_key(&pairing_secret, pairing_id, &initiator.public_key_bytes())
            .unwrap();
        assert_eq!(initiator_key, joiner_key);

        let initiator_code = pairing_verification_code(
            &initiator_key,
            &initiator.public_key_bytes(),
            &joiner.public_key_bytes(),
        );
        let joiner_code = pairing_verification_code(
            &joiner_key,
            &initiator.public_key_bytes(),
            &joiner.public_key_bytes(),
        );
        assert_eq!(initiator_code, joiner_code);
        assert_eq!(initiator_code.len(), 6);
        assert!(initiator_code.chars().all(|c| c.is_ascii_digit()));

        let vault_key = random_vault_key().unwrap();
        let envelope = seal_pairing_vault_key(
            &vault_key,
            &initiator_key,
            pairing_id,
            "device-joiner",
        )
        .unwrap();
        let opened = open_pairing_vault_key(
            &envelope,
            &joiner_key,
            pairing_id,
            "device-joiner",
        )
        .unwrap();
        assert_eq!(opened, vault_key);
    }

    #[test]
    fn canonical_aad_matches_expected_shape() {
        assert_eq!(
            sync_aad_canonical("vault", "op", "doc:1", "upsert", 7, "device"),
            "woo-notes-sync-v1|vault|op|doc:1|upsert|7|device",
        );
        assert_eq!(
            chunk_aad_canonical("vault", "chunk:abc", 42),
            "woo-notes-chunk-v1|vault|chunk:abc|42",
        );
    }

    #[test]
    fn chunk_lamport_and_nonce_are_stable_per_hash() {
        let hash = "a".repeat(64);
        let first = chunk_lamport_and_nonce(&hash).unwrap();
        let second = chunk_lamport_and_nonce(&hash).unwrap();
        assert_eq!(first, second);
        assert!(first.0 >= 1);
        let other = chunk_lamport_and_nonce(&"b".repeat(64)).unwrap();
        assert_ne!(first, other);
    }
}
