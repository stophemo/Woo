//! 基于 WebDAV 配对文件的六位码配对。
//!
//! 六位码只负责定位，安全依赖 X25519 + HKDF + AES-GCM 以及两端人工
//! 核对的六位核对码。临时私钥、vaultKey 和 joinSecret 只存在内存中。

use std::collections::HashSet;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use woo_sync_core::crypto::{
    base64url_decode, base64url_encode, open_pairing_vault_key, pairing_verification_code,
    random_bytes, seal_pairing_vault_key, PairingKeyPair,
};
use woo_sync_core::wire::EncryptedEnvelope;

use crate::db;

use super::account::WebDavAccount;
use super::client::{ensure_collection, RemoteStore};
use super::identity::{
    self, decode_vault_key, space_fingerprint, WebDavIdentity,
};

pub const PAIRING_ROOT: &str = "v1/pairing";
pub const PAIRING_TTL_SECONDS: i64 = 300;
const MIN_TTL_SECONDS: i64 = 60;
const MAX_TTL_SECONDS: i64 = 3600;

static SESSIONS: Lazy<Mutex<Vec<PairingSession>>> = Lazy::new(|| Mutex::new(Vec::new()));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PairingKind {
    Offer,
    Claim,
    Join,
    Resolve,
    Approval,
}

impl PairingKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Offer => "offer",
            Self::Claim => "claim",
            Self::Join => "join",
            Self::Resolve => "resolve",
            Self::Approval => "approval",
        }
    }

}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct PairingFile {
    protocol_version: u32,
    kind: String,
    pairing_id: String,
    short_code: String,
    created_at: i64,
    ttl_seconds: i64,
    device_id: String,
    device_name: String,
    device_platform: String,
    vault_id: String,
    initiator_public_key: String,
    pairing_secret: String,
    claimed_public_key: String,
    joiner_public_key: String,
    join_secret: String,
    approver_public_key: String,
    claimed_device_id: String,
    envelope: Option<EncryptedEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedName {
    kind: PairingKind,
    pairing_id: String,
    short_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PeerView {
    pub device_id: String,
    pub name: String,
    pub platform: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingStartView {
    pub pairing_id: String,
    pub short_code: String,
    pub expires_at: i64,
    pub verification_code: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingStatusView {
    pub pairing_id: String,
    pub status: String,
    pub expires_at: i64,
    pub verification_code: Option<String>,
    pub peer: Option<PeerView>,
    pub vault_id: Option<String>,
    pub space_fingerprint: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinConfirmView {
    pub configured: bool,
    pub vault_id: String,
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub space_fingerprint: String,
}

pub struct ConfirmedJoin {
    pub identity: WebDavIdentity,
    pub view: JoinConfirmView,
}

#[derive(Debug, Clone)]
struct ResolveInfo {
    approver_public: [u8; 32],
    vault_id: String,
    approver: PeerView,
    verification_code: String,
}

#[derive(Clone)]
enum PairingSession {
    OfferHost {
        pairing_id: String,
        short_code: String,
        identity: WebDavIdentity,
        pairing_secret: [u8; 32],
        keypair: PairingKeyPair,
        expires_at: i64,
        written: Vec<String>,
        claimed_public: Option<[u8; 32]>,
        claimed_device: Option<PeerView>,
        verification_code: Option<String>,
    },
    OfferGuest {
        pairing_id: String,
        keypair: PairingKeyPair,
        offer_public: [u8; 32],
        offer_vault_id: String,
        offer_device: PeerView,
        pairing_secret: [u8; 32],
        verification_code: String,
        device_id: String,
        device_name: String,
        platform: String,
        expires_at: i64,
        written: Vec<String>,
        pending_identity: Option<WebDavIdentity>,
    },
    JoinHost {
        pairing_id: String,
        short_code: String,
        identity: WebDavIdentity,
        keypair: PairingKeyPair,
        join_public: [u8; 32],
        join_secret: [u8; 32],
        join_device: PeerView,
        verification_code: String,
        expires_at: i64,
        written: Vec<String>,
    },
    JoinGuest {
        pairing_id: String,
        keypair: PairingKeyPair,
        join_secret: [u8; 32],
        device_id: String,
        device_name: String,
        platform: String,
        expires_at: i64,
        written: Vec<String>,
        resolve: Option<ResolveInfo>,
        pending_identity: Option<WebDavIdentity>,
    },
}

impl PairingSession {
    fn pairing_id(&self) -> &str {
        match self {
            Self::OfferHost { pairing_id, .. }
            | Self::OfferGuest { pairing_id, .. }
            | Self::JoinHost { pairing_id, .. }
            | Self::JoinGuest { pairing_id, .. } => pairing_id,
        }
    }

    fn expires_at(&self) -> i64 {
        match self {
            Self::OfferHost { expires_at, .. }
            | Self::OfferGuest { expires_at, .. }
            | Self::JoinHost { expires_at, .. }
            | Self::JoinGuest { expires_at, .. } => *expires_at,
        }
    }

    fn role(&self) -> u8 {
        match self {
            Self::OfferHost { .. } => 1,
            Self::OfferGuest { .. } => 2,
            Self::JoinHost { .. } => 3,
            Self::JoinGuest { .. } => 4,
        }
    }

    fn written(&self) -> &[String] {
        match self {
            Self::OfferHost { written, .. }
            | Self::OfferGuest { written, .. }
            | Self::JoinHost { written, .. }
            | Self::JoinGuest { written, .. } => written,
        }
    }
}

pub trait ObservationStore: Send + Sync {
    fn first_seen(&self, path: &str, now_ms: i64) -> Result<i64, String>;
    fn forget(&self, path: &str) -> Result<(), String>;
}

pub struct DbObservations;

impl ObservationStore for DbObservations {
    fn first_seen(&self, path: &str, now_ms: i64) -> Result<i64, String> {
        db::with_db(|conn| {
            conn.execute(
                "INSERT OR IGNORE INTO webdav_pairing_observations(path, first_seen_ms)
                 VALUES (?1, ?2)",
                rusqlite::params![path, now_ms],
            )
            .map_err(|error| format!("记录配对文件观察时间失败：{error}"))?;
            conn.query_row(
                "SELECT first_seen_ms FROM webdav_pairing_observations WHERE path = ?1",
                rusqlite::params![path],
                |row| row.get(0),
            )
            .map_err(|error| format!("读取配对文件观察时间失败：{error}"))
        })
    }

    fn forget(&self, path: &str) -> Result<(), String> {
        db::with_db(|conn| {
            conn.execute(
                "DELETE FROM webdav_pairing_observations WHERE path = ?1",
                rusqlite::params![path],
            )
            .map_err(|error| format!("清理配对文件观察时间失败：{error}"))?;
            Ok(())
        })
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn random_short_code() -> Result<String, String> {
    Ok(format!("{:06}", rand::random::<u32>() % 1_000_000))
}

fn pairing_path(name: &str) -> String {
    format!("{PAIRING_ROOT}/{name}")
}

fn offer_name(code: &str, pairing_id: &str) -> String {
    format!("offer-{code}-{pairing_id}.json")
}

fn claim_name(pairing_id: &str) -> String {
    format!("claim-{pairing_id}.json")
}

fn join_name(code: &str, pairing_id: &str) -> String {
    format!("join-{code}-{pairing_id}.json")
}

fn resolve_name(pairing_id: &str) -> String {
    format!("resolve-{pairing_id}.json")
}

fn approval_name(pairing_id: &str) -> String {
    format!("approval-{pairing_id}.json")
}

fn is_six_digits(value: &str) -> bool {
    value.len() == 6 && value.chars().all(|ch| ch.is_ascii_digit())
}

fn parse_name(name: &str) -> Option<ParsedName> {
    let stem = name.strip_suffix(".json")?;
    for kind in [
        PairingKind::Offer,
        PairingKind::Claim,
        PairingKind::Join,
        PairingKind::Resolve,
        PairingKind::Approval,
    ] {
        let prefix = format!("{}-", kind.as_str());
        let Some(rest) = stem.strip_prefix(&prefix) else {
            continue;
        };
        return match kind {
            PairingKind::Offer | PairingKind::Join => {
                if rest.len() < 8 {
                    return None;
                }
                let code = &rest[..6];
                if !is_six_digits(code) || rest.as_bytes().get(6) != Some(&b'-') {
                    return None;
                }
                let pairing_id = &rest[7..];
                uuid::Uuid::parse_str(pairing_id).ok()?;
                Some(ParsedName {
                    kind,
                    pairing_id: pairing_id.to_string(),
                    short_code: Some(code.to_string()),
                })
            }
            _ => {
                uuid::Uuid::parse_str(rest).ok()?;
                Some(ParsedName {
                    kind,
                    pairing_id: rest.to_string(),
                    short_code: None,
                })
            }
        };
    }
    None
}

fn validate_public(value: &str, field: &str) -> Result<[u8; 32], String> {
    let bytes = base64url_decode(value).map_err(|_| format!("{field} 不是有效的 Base64URL"))?;
    <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| format!("{field} 必须是 32 字节"))
}

fn validate_secret(value: &str, field: &str) -> Result<[u8; 32], String> {
    validate_public(value, field)
}

fn ttl(file: &PairingFile) -> i64 {
    file.ttl_seconds.clamp(MIN_TTL_SECONDS, MAX_TTL_SECONDS)
}

fn expired(first_seen_ms: i64, ttl_seconds: i64, now: i64) -> bool {
    now >= first_seen_ms.saturating_add(ttl_seconds.saturating_mul(1000))
}

fn validate_file(
    file: &PairingFile,
    kind: PairingKind,
    pairing_id: &str,
    short_code: Option<&str>,
) -> Result<(), String> {
    if file.protocol_version != 1 {
        return Err("未知的配对协议版本".to_string());
    }
    if file.kind != kind.as_str() {
        return Err("配对文件类型不匹配".to_string());
    }
    if file.pairing_id != pairing_id {
        return Err("配对文件 ID 不匹配".to_string());
    }
    if !is_six_digits(&file.short_code) {
        return Err("配对文件缺少有效的六位码".to_string());
    }
    if let Some(code) = short_code {
        if file.short_code != code {
            return Err("配对文件六位码不匹配".to_string());
        }
    }
    if file.ttl_seconds < MIN_TTL_SECONDS || file.ttl_seconds > MAX_TTL_SECONDS {
        return Err("配对文件有效期无效".to_string());
    }
    Ok(())
}

async fn read_file(
    store: &dyn RemoteStore,
    path: &str,
) -> Result<Option<PairingFile>, String> {
    let Some(bytes) = store.get(path).await.map_err(|error| error.to_string())? else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| "配对文件格式损坏".to_string())
}

async fn write_file(
    store: &dyn RemoteStore,
    path: &str,
    file: &PairingFile,
    if_none_match: bool,
) -> Result<(), String> {
    let bytes = serde_json::to_vec(file)
        .map_err(|error| format!("序列化配对文件失败：{error}"))?;
    ensure_collection(store, PAIRING_ROOT)
        .await
        .map_err(|error| error.to_string())?;
    store
        .put(path, &bytes, if_none_match)
        .await
        .map_err(|error| error.to_string())
}

async fn delete_paths(
    store: &dyn RemoteStore,
    obs: &dyn ObservationStore,
    paths: &[String],
) {
    let mut seen = HashSet::new();
    for path in paths {
        if !seen.insert(path.clone()) {
            continue;
        }
        let _ = store.delete(path).await;
        let _ = obs.forget(path);
    }
}

/// 清理某个 pairingId 的已知协议文件；keep 中的路径保留。
async fn cleanup_pairing_id(
    store: &dyn RemoteStore,
    obs: &dyn ObservationStore,
    pairing_id: &str,
    written: &[String],
    keep: &[String],
) {
    let mut paths: Vec<String> = written.to_vec();
    if let Ok(entries) = store.list(PAIRING_ROOT).await {
        for entry in entries {
            if entry.is_dir {
                continue;
            }
            let Some(parsed) = parse_name(&entry.name) else {
                continue;
            };
            if parsed.pairing_id != pairing_id {
                continue;
            }
            let path = pairing_path(&entry.name);
            if keep.contains(&path) {
                continue;
            }
            // 只删除能确认是 v1 的文件；未知版本/损坏文件保持原样。
            match read_file(store, &path).await {
                Ok(Some(file)) if file.protocol_version == 1 => paths.push(path),
                _ => {}
            }
        }
    }
    delete_paths(store, obs, &paths).await;
}

/// 删除本机写入的配对文件（取消时使用，不碰对端文件）。
pub async fn cancel_pairing(
    store: &dyn RemoteStore,
    obs: &dyn ObservationStore,
) -> Result<(), String> {
    let sessions = SESSIONS
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    for session in sessions {
        let written = session.written().to_vec();
        let role = session.role();
        delete_paths(store, obs, &written).await;
        drop_session(session.pairing_id(), role);
    }
    Ok(())
}

fn session_for<F>(pairing_id: &str, predicate: F) -> Option<PairingSession>
where
    F: Fn(&PairingSession) -> bool,
{
    SESSIONS.lock().ok().and_then(|guard| {
        guard
            .iter()
            .find(|session| session.pairing_id() == pairing_id && predicate(session))
            .cloned()
    })
}

fn offer_host_session(pairing_id: &str) -> Option<PairingSession> {
    session_for(pairing_id, |session| matches!(session, PairingSession::OfferHost { .. }))
}

fn guest_session(pairing_id: &str) -> Option<PairingSession> {
    session_for(pairing_id, |session| {
        matches!(
            session,
            PairingSession::OfferGuest { .. } | PairingSession::JoinGuest { .. }
        )
    })
}

fn join_host_session(pairing_id: &str) -> Option<PairingSession> {
    session_for(pairing_id, |session| matches!(session, PairingSession::JoinHost { .. }))
}

fn set_session(session: PairingSession) -> Result<(), String> {
    let mut guard = SESSIONS
        .lock()
        .map_err(|_| "配对状态锁已损坏".to_string())?;
    // 每台设备同一时间只保留一个同角色配对会话；测试中可同时存在
    // 发起方与新设备两种角色。
    guard.retain(|current| current.role() != session.role());
    guard.push(session);
    Ok(())
}

fn update_session(session: PairingSession) {
    if let Ok(mut guard) = SESSIONS.lock() {
        if let Some(current) = guard.iter_mut().find(|current| {
            current.pairing_id() == session.pairing_id() && current.role() == session.role()
        }) {
            *current = session;
        }
    }
}

fn drop_session(pairing_id: &str, role: u8) {
    if let Ok(mut guard) = SESSIONS.lock() {
        guard.retain(|current| current.pairing_id() != pairing_id || current.role() != role);
    }
}

fn peer_from_file(file: &PairingFile) -> PeerView {
    PeerView {
        device_id: file.device_id.clone(),
        name: file.device_name.clone(),
        platform: file.device_platform.clone(),
    }
}

fn status_view(
    pairing_id: &str,
    status: &str,
    expires_at: i64,
    verification_code: Option<String>,
    peer: Option<PeerView>,
    vault_id: Option<String>,
    message: Option<String>,
) -> PairingStatusView {
    PairingStatusView {
        pairing_id: pairing_id.to_string(),
        status: status.to_string(),
        expires_at,
        verification_code,
        peer,
        space_fingerprint: vault_id.as_deref().map(space_fingerprint),
        vault_id,
        message,
    }
}

async fn find_unique_pairing_file(
    store: &dyn RemoteStore,
    obs: &dyn ObservationStore,
    kind: PairingKind,
    code: &str,
) -> Result<(PairingFile, i64), String> {
    if !is_six_digits(code) {
        return Err("请输入六位配对码".to_string());
    }
    let entries = store
        .list(PAIRING_ROOT)
        .await
        .map_err(|error| error.to_string())?;
    let now = now_ms();
    let mut candidates = Vec::new();
    for entry in entries {
        if entry.is_dir {
            continue;
        }
        let Some(parsed) = parse_name(&entry.name) else {
            continue;
        };
        if parsed.kind != kind || parsed.short_code.as_deref() != Some(code) {
            continue;
        }
        let path = pairing_path(&entry.name);
        let file = match read_file(store, &path).await {
            Ok(Some(file)) => file,
            Ok(None) => continue,
            Err(_) => continue,
        };
        if file.protocol_version != 1 {
            continue;
        }
        if validate_file(&file, kind, &parsed.pairing_id, Some(code)).is_err() {
            continue;
        }
        let first_seen = obs.first_seen(&path, now)?;
        if expired(first_seen, ttl(&file), now) {
            let _ = store.delete(&path).await;
            let _ = obs.forget(&path);
            continue;
        }
        let expires_at = first_seen.saturating_add(ttl(&file).saturating_mul(1000));
        candidates.push((file, expires_at));
    }
    match candidates.len() {
        0 => Err("没有找到对应的配对码；请确认六位码，或让对端重新生成".to_string()),
        1 => Ok(candidates.remove(0)),
        _ => Err("配对码冲突：同一六位码对应多个配对请求，请重新生成配对码".to_string()),
    }
}

async fn observe_single_file(
    store: &dyn RemoteStore,
    obs: &dyn ObservationStore,
    path: &str,
) -> Result<Option<(PairingFile, i64)>, String> {
    let Some(file) = read_file(store, path).await? else {
        return Ok(None);
    };
    if file.protocol_version != 1 {
        return Ok(None);
    }
    let now = now_ms();
    let first_seen = obs.first_seen(path, now)?;
    if expired(first_seen, ttl(&file), now) {
        let _ = store.delete(path).await;
        let _ = obs.forget(path);
        return Ok(None);
    }
    let expires_at = first_seen.saturating_add(ttl(&file).saturating_mul(1000));
    Ok(Some((file, expires_at)))
}

// ============================================================
// Path A：已有设备显示配对码
// ============================================================

pub async fn start_offer(
    _account: &WebDavAccount,
    identity: &WebDavIdentity,
    store: &dyn RemoteStore,
) -> Result<PairingStartView, String> {
    let pairing_id = uuid::Uuid::new_v4().to_string();
    let short_code = random_short_code()?;
    let pairing_secret = random_bytes::<32>()?;
    let keypair = PairingKeyPair::generate()?;
    let now = now_ms();
    let expires_at = now.saturating_add(PAIRING_TTL_SECONDS * 1000);
    let name = offer_name(&short_code, &pairing_id);
    let path = pairing_path(&name);
    let file = PairingFile {
        protocol_version: 1,
        kind: PairingKind::Offer.as_str().to_string(),
        pairing_id: pairing_id.clone(),
        short_code: short_code.clone(),
        created_at: now,
        ttl_seconds: PAIRING_TTL_SECONDS,
        device_id: identity.device_id.clone(),
        device_name: identity.device_name.clone(),
        device_platform: identity.platform.clone(),
        vault_id: identity.vault_id.clone(),
        initiator_public_key: keypair.public_key_base64url(),
        pairing_secret: base64url_encode(&pairing_secret),
        ..PairingFile::default()
    };
    write_file(store, &path, &file, true).await?;
    set_session(PairingSession::OfferHost {
        pairing_id: pairing_id.clone(),
        short_code: short_code.clone(),
        identity: identity.clone(),
        pairing_secret,
        keypair,
        expires_at,
        written: vec![path],
        claimed_public: None,
        claimed_device: None,
        verification_code: None,
    })?;
    Ok(PairingStartView {
        pairing_id,
        short_code,
        expires_at,
        verification_code: None,
    })
}

pub async fn offer_status(
    pairing_id: &str,
    store: &dyn RemoteStore,
    obs: &dyn ObservationStore,
) -> Result<PairingStatusView, String> {
    let mut session = offer_host_session(pairing_id)
        .ok_or_else(|| "配对会话不存在，请重新生成配对码".to_string())?;
    let expires_at = session.expires_at();
    if now_ms() >= expires_at {
        let written = session.written().to_vec();
        let role = session.role();
        delete_paths(store, obs, &written).await;
        drop_session(pairing_id, role);
        return Ok(status_view(
            pairing_id,
            "expired",
            expires_at,
            None,
            None,
            None,
            Some("配对码已过期，请重新生成".to_string()),
        ));
    }
    let PairingSession::OfferHost {
        short_code,
        pairing_secret,
        keypair,
        identity,
        claimed_public,
        claimed_device,
        verification_code,
        ..
    } = &mut session
    else {
        return Err("当前配对角色不支持该操作".to_string());
    };
    let path = pairing_path(&claim_name(pairing_id));
    let claim = observe_single_file(store, obs, &path).await?;
    if let Some((file, _)) = claim {
        if validate_file(&file, PairingKind::Claim, pairing_id, Some(short_code)).is_err() {
            return Err("新设备的认领文件校验失败，请重新生成配对码".to_string());
        }
        let public = validate_public(&file.claimed_public_key, "新设备公钥")?;
        if file.device_id.is_empty() || file.device_name.is_empty() {
            return Err("认领文件缺少设备信息".to_string());
        }
        if let Some(existing) = claimed_public {
            if *existing != public || claimed_device.as_ref().map(|p| &p.device_id) != Some(&file.device_id) {
                return Err("配对码已被另一台设备认领，请重新生成".to_string());
            }
        }
        let session_key = keypair.session_key(pairing_secret, pairing_id, &public)?;
        let code = pairing_verification_code(
            &session_key,
            &keypair.public_key_bytes(),
            &public,
        );
        *claimed_public = Some(public);
        *claimed_device = Some(peer_from_file(&file));
        *verification_code = Some(code);
        let vault_id = identity.vault_id.clone();
        let view = status_view(
            pairing_id,
            "claimed",
            expires_at,
            verification_code.clone(),
            claimed_device.clone(),
            Some(vault_id),
            Some("请核对两台设备显示的六位码一致后，再批准新设备".to_string()),
        );
        update_session(session);
        return Ok(view);
    }
    let view = status_view(
        pairing_id,
        "waiting",
        expires_at,
        None,
        None,
        None,
        Some("等待新设备输入配对码".to_string()),
    );
    update_session(session);
    Ok(view)
}

pub async fn approve_offer(
    pairing_id: &str,
    store: &dyn RemoteStore,
    obs: &dyn ObservationStore,
) -> Result<PairingStatusView, String> {
    let session = offer_host_session(pairing_id)
        .ok_or_else(|| "配对会话不存在".to_string())?;
    if now_ms() >= session.expires_at() {
        return Err("配对码已过期，请重新生成".to_string());
    }
    let PairingSession::OfferHost {
        short_code,
        identity,
        pairing_secret,
        keypair,
        claimed_public,
        claimed_device,
        verification_code,
        written,
        ..
    } = &session
    else {
        return Err("当前配对角色不支持该操作".to_string());
    };
    let public = claimed_public.ok_or_else(|| "新设备还没有输入配对码".to_string())?;
    let peer = claimed_device.clone().ok_or_else(|| "认领文件缺少设备信息".to_string())?;
    let claim_path = pairing_path(&claim_name(pairing_id));
    let claim = read_file(store, &claim_path)
        .await?
        .ok_or_else(|| "新设备的认领文件已消失，请重新生成配对码".to_string())?;
    validate_file(&claim, PairingKind::Claim, pairing_id, Some(short_code))?;
    if validate_public(&claim.claimed_public_key, "新设备公钥")? != public {
        return Err("认领文件在确认过程中发生变化，请重新生成配对码".to_string());
    }
    let vault_key = decode_vault_key(identity)?;
    let session_key = keypair.session_key(pairing_secret, pairing_id, &public)?;
    let envelope = seal_pairing_vault_key(
        &vault_key,
        &session_key,
        pairing_id,
        &claim.device_id,
    )?;
    let now = now_ms();
    let approval_path = pairing_path(&approval_name(pairing_id));
    let approval = PairingFile {
        protocol_version: 1,
        kind: PairingKind::Approval.as_str().to_string(),
        pairing_id: pairing_id.to_string(),
        short_code: short_code.clone(),
        created_at: now,
        ttl_seconds: PAIRING_TTL_SECONDS,
        device_id: identity.device_id.clone(),
        device_name: identity.device_name.clone(),
        device_platform: identity.platform.clone(),
        vault_id: identity.vault_id.clone(),
        approver_public_key: keypair.public_key_base64url(),
        claimed_device_id: claim.device_id.clone(),
        envelope: Some(envelope),
        ..PairingFile::default()
    };
    write_file(store, &approval_path, &approval, false).await?;
    // 保留 approval 给新设备读取；清理 offer/claim。
    cleanup_pairing_id(
        store,
        obs,
        pairing_id,
        written,
        &[approval_path.clone()],
    )
    .await;
    drop_session(pairing_id, 1);
    Ok(status_view(
        pairing_id,
        "approved",
        session.expires_at(),
        verification_code.clone(),
        Some(peer),
        Some(identity.vault_id.clone()),
        Some("已批准新设备，等待新设备确认".to_string()),
    ))
}

// ============================================================
// Path A：新设备输入配对码
// ============================================================

pub async fn find_offer_and_claim(
    _account: &WebDavAccount,
    code: &str,
    device_name: &str,
    store: &dyn RemoteStore,
    obs: &dyn ObservationStore,
) -> Result<PairingStatusView, String> {
    let (offer, expires_at) =
        find_unique_pairing_file(store, obs, PairingKind::Offer, code).await?;
    let pairing_id = offer.pairing_id.clone();
    let host_public = validate_public(&offer.initiator_public_key, "发起设备公钥")?;
    let pairing_secret = validate_secret(&offer.pairing_secret, "配对密钥")?;
    if offer.vault_id.is_empty() {
        return Err("配对文件缺少同步空间 ID".to_string());
    }
    let keypair = PairingKeyPair::generate()?;
    let device_id = uuid::Uuid::new_v4().to_string();
    let device_name = identity::sanitize_device_name(device_name);
    let platform = crate::services::vault_sync::config::platform_name().to_string();
    let session_key = keypair.session_key(&pairing_secret, &pairing_id, &host_public)?;
    let verification_code = pairing_verification_code(
        &session_key,
        &host_public,
        &keypair.public_key_bytes(),
    );
    let claim_path = pairing_path(&claim_name(&pairing_id));
    let claim = PairingFile {
        protocol_version: 1,
        kind: PairingKind::Claim.as_str().to_string(),
        pairing_id: pairing_id.clone(),
        short_code: code.to_string(),
        created_at: now_ms(),
        ttl_seconds: PAIRING_TTL_SECONDS,
        device_id: device_id.clone(),
        device_name: device_name.clone(),
        device_platform: platform.clone(),
        claimed_public_key: keypair.public_key_base64url(),
        ..PairingFile::default()
    };
    write_file(store, &claim_path, &claim, true)
        .await
        .map_err(|error| {
            if error.contains("已存在") {
                "配对码已被另一台设备认领，请让已有设备重新生成".to_string()
            } else {
                error
            }
        })?;
    set_session(PairingSession::OfferGuest {
        pairing_id: pairing_id.clone(),
        keypair,
        offer_public: host_public,
        offer_vault_id: offer.vault_id.clone(),
        offer_device: peer_from_file(&offer),
        pairing_secret,
        verification_code: verification_code.clone(),
        device_id,
        device_name,
        platform,
        expires_at,
        written: vec![claim_path],
        pending_identity: None,
    })?;
    Ok(status_view(
        &pairing_id,
        "claimed",
        expires_at,
        Some(verification_code),
        Some(peer_from_file(&offer)),
        Some(offer.vault_id),
        Some("等待已有设备核对并批准".to_string()),
    ))
}

/// 新设备轮询配对结果。Path A 读取 approval；Path B 必须先看到 resolve。
pub async fn join_status(
    pairing_id: &str,
    store: &dyn RemoteStore,
    obs: &dyn ObservationStore,
) -> Result<PairingStatusView, String> {
    let mut session = guest_session(pairing_id)
        .ok_or_else(|| "配对会话不存在，请重新输入配对码".to_string())?;
    let expires_at = session.expires_at();
    if now_ms() >= expires_at {
        let written = session.written().to_vec();
        let role = session.role();
        delete_paths(store, obs, &written).await;
        drop_session(pairing_id, role);
        return Ok(status_view(
            pairing_id,
            "expired",
            expires_at,
            None,
            None,
            None,
            Some("配对码已过期，请重新生成".to_string()),
        ));
    }

    match &mut session {
        PairingSession::OfferGuest {
            keypair,
            offer_public,
            offer_vault_id,
            offer_device,
            pairing_secret,
            verification_code,
            device_id,
            device_name,
            platform,
            pending_identity,
            ..
        } => {
            if pending_identity.is_some() {
                return Ok(status_view(
                    pairing_id,
                    "ready",
                    expires_at,
                    Some(verification_code.clone()),
                    Some(offer_device.clone()),
                    Some(offer_vault_id.clone()),
                    Some("请核对六位码和空间 ID 后点击「核对一致，完成加入」".to_string()),
                ));
            }
            let approval_path = pairing_path(&approval_name(pairing_id));
            let Some((approval, _)) = observe_single_file(store, obs, &approval_path).await? else {
                return Ok(status_view(
                    pairing_id,
                    "claimed",
                    expires_at,
                    Some(verification_code.clone()),
                    Some(offer_device.clone()),
                    Some(offer_vault_id.clone()),
                    Some("等待已有设备批准".to_string()),
                ));
            };
            validate_file(&approval, PairingKind::Approval, pairing_id, None)?;
            if approval.vault_id != *offer_vault_id {
                return Err("批准文件中的同步空间 ID 与配对码不一致，已拒绝".to_string());
            }
            if approval.claimed_device_id != *device_id {
                return Err("批准文件不是发给本设备的，已拒绝".to_string());
            }
            let approver_public =
                validate_public(&approval.approver_public_key, "批准设备公钥")?;
            if approver_public != *offer_public {
                return Err("批准设备公钥与配对发起设备不一致，已拒绝".to_string());
            }
            let envelope = approval
                .envelope
                .clone()
                .ok_or_else(|| "批准文件缺少同步空间密钥".to_string())?;
            let session_key = keypair.session_key(pairing_secret, pairing_id, &approver_public)?;
            let vault_key = open_pairing_vault_key(&envelope, &session_key, pairing_id, device_id)?;
            let identity = WebDavIdentity {
                vault_id: offer_vault_id.clone(),
                device_id: device_id.clone(),
                vault_key: base64url_encode(&vault_key),
                device_name: device_name.clone(),
                platform: platform.clone(),
                bound_user: db::current_user_key(),
            };
            *pending_identity = Some(identity);
            let view = status_view(
                pairing_id,
                "ready",
                expires_at,
                Some(verification_code.clone()),
                Some(offer_device.clone()),
                Some(offer_vault_id.clone()),
                Some("请核对六位码和空间 ID 后点击「核对一致，完成加入」".to_string()),
            );
            update_session(session);
            let _ = approval;
            return Ok(view);
        }
        PairingSession::JoinGuest {
            keypair,
            join_secret,
            device_id,
            device_name,
            platform,
            resolve,
            pending_identity,
            ..
        } => {
            if pending_identity.is_some() {
                let info = resolve.as_ref().ok_or_else(|| "缺少批准设备信息".to_string())?;
                return Ok(status_view(
                    pairing_id,
                    "ready",
                    expires_at,
                    Some(info.verification_code.clone()),
                    Some(info.approver.clone()),
                    Some(info.vault_id.clone()),
                    Some("请核对六位码和空间 ID 后点击「核对一致，完成加入」".to_string()),
                ));
            }
            if resolve.is_none() {
                // 关键安全不变量：只有 approval、没有 resolve 时直接忽略。
                let resolve_path = pairing_path(&resolve_name(pairing_id));
                let Some((resolve_file, _)) =
                    observe_single_file(store, obs, &resolve_path).await?
                else {
                    return Ok(status_view(
                        pairing_id,
                        "waiting",
                        expires_at,
                        None,
                        None,
                        None,
                        Some("等待已有设备输入配对码".to_string()),
                    ));
                };
                validate_file(&resolve_file, PairingKind::Resolve, pairing_id, None)?;
                let approver_public =
                    validate_public(&resolve_file.approver_public_key, "批准设备公钥")?;
                if resolve_file.vault_id.is_empty() {
                    return Err("resolve 文件缺少同步空间 ID".to_string());
                }
                let session_key =
                    keypair.session_key(join_secret, pairing_id, &approver_public)?;
                let code = pairing_verification_code(
                    &session_key,
                    &approver_public,
                    &keypair.public_key_bytes(),
                );
                let approver = peer_from_file(&resolve_file);
                *resolve = Some(ResolveInfo {
                    approver_public,
                    vault_id: resolve_file.vault_id,
                    approver,
                    verification_code: code,
                });
            }
            let info = resolve.as_ref().ok_or_else(|| "缺少批准设备信息".to_string())?;
            let approval_path = pairing_path(&approval_name(pairing_id));
            let Some((approval, _)) = observe_single_file(store, obs, &approval_path).await?
            else {
                let view = status_view(
                    pairing_id,
                    "resolved",
                    expires_at,
                    Some(info.verification_code.clone()),
                    Some(info.approver.clone()),
                    Some(info.vault_id.clone()),
                    Some("等待已有设备批准".to_string()),
                );
                update_session(session);
                return Ok(view);
            };
            validate_file(&approval, PairingKind::Approval, pairing_id, None)?;
            if approval.vault_id != info.vault_id {
                return Err("批准文件中的同步空间 ID 与 resolve 不一致，已拒绝".to_string());
            }
            if approval.claimed_device_id != *device_id {
                return Err("批准文件不是发给本设备的，已拒绝".to_string());
            }
            let approver_public =
                validate_public(&approval.approver_public_key, "批准设备公钥")?;
            if approver_public != info.approver_public {
                return Err("批准设备公钥与 resolve 不一致，已拒绝".to_string());
            }
            let envelope = approval
                .envelope
                .clone()
                .ok_or_else(|| "批准文件缺少同步空间密钥".to_string())?;
            let session_key = keypair.session_key(join_secret, pairing_id, &approver_public)?;
            let vault_key = open_pairing_vault_key(&envelope, &session_key, pairing_id, device_id)?;
            let identity = WebDavIdentity {
                vault_id: info.vault_id.clone(),
                device_id: device_id.clone(),
                vault_key: base64url_encode(&vault_key),
                device_name: device_name.clone(),
                platform: platform.clone(),
                bound_user: db::current_user_key(),
            };
            *pending_identity = Some(identity);
            let view = status_view(
                pairing_id,
                "ready",
                expires_at,
                Some(info.verification_code.clone()),
                Some(info.approver.clone()),
                Some(info.vault_id.clone()),
                Some("请核对六位码和空间 ID 后点击「核对一致，完成加入」".to_string()),
            );
            update_session(session);
            return Ok(view);
        }
        _ => return Err("当前配对角色不支持该操作".to_string()),
    }
}

/// 用户人工核对后才会走到这里：保存身份并清理配对文件。
pub async fn confirm_join(
    pairing_id: &str,
    store: &dyn RemoteStore,
    obs: &dyn ObservationStore,
) -> Result<ConfirmedJoin, String> {
    let session = guest_session(pairing_id)
        .ok_or_else(|| "配对会话不存在".to_string())?;
    if now_ms() >= session.expires_at() {
        return Err("配对码已过期，请重新生成".to_string());
    }
    let (identity, written) = match &session {
        PairingSession::OfferGuest {
            pending_identity,
            written,
            ..
        } => (
            pending_identity
                .clone()
                .ok_or_else(|| "还没有收到可确认的配对结果".to_string())?,
            written.clone(),
        ),
        PairingSession::JoinGuest {
            pending_identity,
            written,
            ..
        } => (
            pending_identity
                .clone()
                .ok_or_else(|| "还没有收到可确认的配对结果".to_string())?,
            written.clone(),
        ),
        _ => return Err("当前配对角色不支持该操作".to_string()),
    };
    identity::save_identity(&identity)?;
    cleanup_pairing_id(store, obs, pairing_id, &written, &[]).await;
    drop_session(pairing_id, 2);
    Ok(ConfirmedJoin {
        view: JoinConfirmView {
            configured: true,
            vault_id: identity.vault_id.clone(),
            device_id: identity.device_id.clone(),
            device_name: identity.device_name.clone(),
            platform: identity.platform.clone(),
            space_fingerprint: space_fingerprint(&identity.vault_id),
        },
        identity,
    })
}

// ============================================================
// Path B：新设备显示配对码，已有设备输入并批准
// ============================================================

pub async fn start_join(
    _account: &WebDavAccount,
    device_name: &str,
    store: &dyn RemoteStore,
) -> Result<PairingStartView, String> {
    let pairing_id = uuid::Uuid::new_v4().to_string();
    let short_code = random_short_code()?;
    let keypair = PairingKeyPair::generate()?;
    let join_secret = random_bytes::<32>()?;
    let device_id = uuid::Uuid::new_v4().to_string();
    let device_name = identity::sanitize_device_name(device_name);
    let platform = crate::services::vault_sync::config::platform_name().to_string();
    let now = now_ms();
    let expires_at = now.saturating_add(PAIRING_TTL_SECONDS * 1000);
    let path = pairing_path(&join_name(&short_code, &pairing_id));
    let file = PairingFile {
        protocol_version: 1,
        kind: PairingKind::Join.as_str().to_string(),
        pairing_id: pairing_id.clone(),
        short_code: short_code.clone(),
        created_at: now,
        ttl_seconds: PAIRING_TTL_SECONDS,
        device_id: device_id.clone(),
        device_name: device_name.clone(),
        device_platform: platform.clone(),
        joiner_public_key: keypair.public_key_base64url(),
        join_secret: base64url_encode(&join_secret),
        ..PairingFile::default()
    };
    write_file(store, &path, &file, true).await?;
    set_session(PairingSession::JoinGuest {
        pairing_id: pairing_id.clone(),
        keypair,
        join_secret,
        device_id,
        device_name,
        platform,
        expires_at,
        written: vec![path],
        resolve: None,
        pending_identity: None,
    })?;
    Ok(PairingStartView {
        pairing_id,
        short_code,
        expires_at,
        verification_code: None,
    })
}

pub async fn find_join_and_resolve(
    _account: &WebDavAccount,
    identity: &WebDavIdentity,
    code: &str,
    store: &dyn RemoteStore,
    obs: &dyn ObservationStore,
) -> Result<PairingStatusView, String> {
    let (join, expires_at) =
        find_unique_pairing_file(store, obs, PairingKind::Join, code).await?;
    let pairing_id = join.pairing_id.clone();
    let join_public = validate_public(&join.joiner_public_key, "新设备公钥")?;
    let join_secret = validate_secret(&join.join_secret, "加入密钥")?;
    if join.device_id.is_empty() || join.device_name.is_empty() {
        return Err("配对文件缺少新设备信息".to_string());
    }
    let keypair = PairingKeyPair::generate()?;
    let session_key = keypair.session_key(&join_secret, &pairing_id, &join_public)?;
    let verification_code = pairing_verification_code(
        &session_key,
        &keypair.public_key_bytes(),
        &join_public,
    );
    let path = pairing_path(&resolve_name(&pairing_id));
    let resolve = PairingFile {
        protocol_version: 1,
        kind: PairingKind::Resolve.as_str().to_string(),
        pairing_id: pairing_id.clone(),
        short_code: code.to_string(),
        created_at: now_ms(),
        ttl_seconds: PAIRING_TTL_SECONDS,
        device_id: identity.device_id.clone(),
        device_name: identity.device_name.clone(),
        device_platform: identity.platform.clone(),
        vault_id: identity.vault_id.clone(),
        approver_public_key: keypair.public_key_base64url(),
        ..PairingFile::default()
    };
    write_file(store, &path, &resolve, false).await?;
    set_session(PairingSession::JoinHost {
        pairing_id: pairing_id.clone(),
        short_code: code.to_string(),
        identity: identity.clone(),
        keypair,
        join_public,
        join_secret,
        join_device: peer_from_file(&join),
        verification_code: verification_code.clone(),
        expires_at,
        written: vec![path],
    })?;
    Ok(status_view(
        &pairing_id,
        "claimed",
        expires_at,
        Some(verification_code),
        Some(peer_from_file(&join)),
        Some(identity.vault_id.clone()),
        Some("请核对两台设备显示的六位码一致后，再批准新设备".to_string()),
    ))
}

pub async fn approve_join(
    pairing_id: &str,
    store: &dyn RemoteStore,
    obs: &dyn ObservationStore,
) -> Result<PairingStatusView, String> {
    let session = join_host_session(pairing_id)
        .ok_or_else(|| "配对会话不存在".to_string())?;
    if now_ms() >= session.expires_at() {
        return Err("配对码已过期，请重新输入".to_string());
    }
    let PairingSession::JoinHost {
        short_code,
        identity,
        keypair,
        join_public,
        join_secret,
        join_device,
        verification_code,
        written,
        ..
    } = &session
    else {
        return Err("当前配对角色不支持该操作".to_string());
    };
    let vault_key = decode_vault_key(identity)?;
    let session_key = keypair.session_key(join_secret, pairing_id, join_public)?;
    let envelope = seal_pairing_vault_key(
        &vault_key,
        &session_key,
        pairing_id,
        &join_device.device_id,
    )?;
    let approval_path = pairing_path(&approval_name(pairing_id));
    let approval = PairingFile {
        protocol_version: 1,
        kind: PairingKind::Approval.as_str().to_string(),
        pairing_id: pairing_id.to_string(),
        short_code: short_code.clone(),
        created_at: now_ms(),
        ttl_seconds: PAIRING_TTL_SECONDS,
        device_id: identity.device_id.clone(),
        device_name: identity.device_name.clone(),
        device_platform: identity.platform.clone(),
        vault_id: identity.vault_id.clone(),
        approver_public_key: keypair.public_key_base64url(),
        claimed_device_id: join_device.device_id.clone(),
        envelope: Some(envelope),
        ..PairingFile::default()
    };
    write_file(store, &approval_path, &approval, false).await?;
    cleanup_pairing_id(
        store,
        obs,
        pairing_id,
        written,
        &[approval_path],
    )
    .await;
    drop_session(pairing_id, 3);
    Ok(status_view(
        pairing_id,
        "approved",
        session.expires_at(),
        Some(verification_code.clone()),
        Some(join_device.clone()),
        Some(identity.vault_id.clone()),
        Some("已批准新设备，等待新设备确认".to_string()),
    ))
}

/// 清理超过本机观察窗口的已知配对文件；未知版本/损坏文件不删除。
pub async fn cleanup_expired(
    store: &dyn RemoteStore,
    obs: &dyn ObservationStore,
) -> Result<usize, String> {
    let entries = store
        .list(PAIRING_ROOT)
        .await
        .map_err(|error| error.to_string())?;
    let now = now_ms();
    let mut removed = 0usize;
    for entry in entries {
        if entry.is_dir {
            continue;
        }
        let Some(parsed) = parse_name(&entry.name) else {
            continue;
        };
        let path = pairing_path(&entry.name);
        let file = match read_file(store, &path).await {
            Ok(Some(file)) if file.protocol_version == 1 => file,
            _ => continue,
        };
        if validate_file(&file, parsed.kind, &parsed.pairing_id, parsed.short_code.as_deref()).is_err() {
            continue;
        }
        let first_seen = obs.first_seen(&path, now)?;
        if expired(first_seen, ttl(&file), now) {
            let _ = store.delete(&path).await;
            let _ = obs.forget(&path);
            removed += 1;
        }
    }
    Ok(removed)
}

pub fn clear_session() {
    if let Ok(mut guard) = SESSIONS.lock() {
        guard.clear();
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::HashMap;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use super::*;
    use crate::services::webdav_space::client::test_support::MemoryRemote;

    fn test_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[derive(Default)]
    pub(crate) struct MemoryObservations {
        seen: Mutex<HashMap<String, i64>>,
    }

    impl ObservationStore for MemoryObservations {
        fn first_seen(&self, path: &str, now_ms: i64) -> Result<i64, String> {
            let mut seen = self.seen.lock().unwrap();
            Ok(*seen.entry(path.to_string()).or_insert(now_ms))
        }

        fn forget(&self, path: &str) -> Result<(), String> {
            self.seen.lock().unwrap().remove(path);
            Ok(())
        }
    }

    pub(crate) fn account() -> WebDavAccount {
        WebDavAccount {
            endpoint: "https://dav.example.com".to_string(),
            username: "user".to_string(),
            app_password: "app-password".to_string(),
            bound_user: db::current_user_key(),
        }
    }

    fn offer_file(code: &str, pairing_id: &str) -> PairingFile {
        PairingFile {
            protocol_version: 1,
            kind: "offer".to_string(),
            pairing_id: pairing_id.to_string(),
            short_code: code.to_string(),
            created_at: 1,
            ttl_seconds: 300,
            device_id: "host-device".to_string(),
            device_name: "Host".to_string(),
            device_platform: "macos".to_string(),
            vault_id: "vault-1".to_string(),
            initiator_public_key: "invalid".to_string(),
            pairing_secret: "invalid".to_string(),
            ..PairingFile::default()
        }
    }

    #[tokio::test]
    async fn path_a_full_flow_and_does_not_save_before_confirm() {
        let _guard = test_lock();
        clear_session();
        let remote = MemoryRemote::new();
        let obs = MemoryObservations::default();
        let account = account();
        let host = identity::create_identity("Host").unwrap();

        let offer = start_offer(&account, &host, &remote).await.unwrap();
        let guest = find_offer_and_claim(
            &account,
            &offer.short_code,
            "Guest",
            &remote,
            &obs,
        )
        .await
        .unwrap();
        assert_eq!(guest.status, "claimed");
        assert!(guest.verification_code.is_some());

        let host_status = offer_status(&offer.pairing_id, &remote, &obs).await.unwrap();
        assert_eq!(host_status.status, "claimed");
        assert_eq!(host_status.verification_code, guest.verification_code);
        assert_eq!(host_status.vault_id.as_deref(), Some(host.vault_id.as_str()));

        let waiting = join_status(&offer.pairing_id, &remote, &obs).await.unwrap();
        assert_eq!(waiting.status, "claimed");

        approve_offer(&offer.pairing_id, &remote, &obs).await.unwrap();
        let files = remote.files.lock().unwrap();
        assert!(files.keys().any(|path| path.ends_with(&approval_name(&offer.pairing_id))));
        assert!(!files.keys().any(|path| path.ends_with(&offer_name(&offer.short_code, &offer.pairing_id))));
        drop(files);
        let ready = join_status(&offer.pairing_id, &remote, &obs).await.unwrap();
        assert_eq!(ready.status, "ready");
        assert_eq!(ready.verification_code, guest.verification_code);
        assert_eq!(ready.vault_id.as_deref(), Some(host.vault_id.as_str()));
        assert_eq!(
            ready.space_fingerprint.as_deref(),
            Some(space_fingerprint(&host.vault_id).as_str())
        );

        let session = guest_session(&offer.pairing_id).unwrap();
        let PairingSession::OfferGuest {
            pending_identity: Some(pending),
            ..
        } = session
        else {
            panic!("expected pending guest identity");
        };
        assert_eq!(pending.vault_id, host.vault_id);
        assert_eq!(
            identity::decode_vault_key(&pending).unwrap(),
            identity::decode_vault_key(&host).unwrap()
        );
        assert!(!identity::identity_path().exists());
        clear_session();
    }

    #[tokio::test]
    async fn path_b_requires_resolve_and_ignores_approval_only() {
        let _guard = test_lock();
        clear_session();
        let remote = MemoryRemote::new();
        let obs = MemoryObservations::default();
        let account = account();
        let host = identity::create_identity("Host").unwrap();

        let join = start_join(&account, "Guest", &remote).await.unwrap();
        // approval-only：没有 resolve 时必须忽略。
        let approval_path = pairing_path(&approval_name(&join.pairing_id));
        let fake_approval = PairingFile {
            protocol_version: 1,
            kind: "approval".to_string(),
            pairing_id: join.pairing_id.clone(),
            short_code: join.short_code.clone(),
            ttl_seconds: 300,
            vault_id: host.vault_id.clone(),
            approver_public_key: "invalid".to_string(),
            claimed_device_id: "invalid".to_string(),
            ..PairingFile::default()
        };
        write_file(&remote, &approval_path, &fake_approval, false)
            .await
            .unwrap();
        let waiting = join_status(&join.pairing_id, &remote, &obs).await.unwrap();
        assert_eq!(waiting.status, "waiting");
        assert!(waiting.verification_code.is_none());
        remote.delete(&approval_path).await.unwrap();

        let host_view = find_join_and_resolve(
            &account,
            &host,
            &join.short_code,
            &remote,
            &obs,
        )
        .await
        .unwrap();
        assert_eq!(host_view.status, "claimed");
        let resolved = join_status(&join.pairing_id, &remote, &obs).await.unwrap();
        assert_eq!(resolved.status, "resolved");
        assert_eq!(resolved.verification_code, host_view.verification_code);

        approve_join(&join.pairing_id, &remote, &obs).await.unwrap();
        let ready = join_status(&join.pairing_id, &remote, &obs).await.unwrap();
        assert_eq!(ready.status, "ready");
        assert_eq!(ready.verification_code, host_view.verification_code);
        assert_eq!(ready.vault_id.as_deref(), Some(host.vault_id.as_str()));
        clear_session();
    }

    #[tokio::test]
    async fn same_code_with_two_offers_is_a_conflict() {
        let _guard = test_lock();
        clear_session();
        let remote = MemoryRemote::new();
        let obs = MemoryObservations::default();
        let account = account();
        let code = "123456";
        let id_a = uuid::Uuid::new_v4().to_string();
        let id_b = uuid::Uuid::new_v4().to_string();
        write_file(
            &remote,
            &pairing_path(&offer_name(code, &id_a)),
            &offer_file(code, &id_a),
            false,
        )
        .await
        .unwrap();
        write_file(
            &remote,
            &pairing_path(&offer_name(code, &id_b)),
            &offer_file(code, &id_b),
            false,
        )
        .await
        .unwrap();
        let error = find_offer_and_claim(&account, code, "Guest", &remote, &obs)
            .await
            .unwrap_err();
        assert!(error.contains("冲突"));
    }

    #[tokio::test]
    async fn approval_with_wrong_vault_id_is_rejected() {
        let _guard = test_lock();
        clear_session();
        let remote = MemoryRemote::new();
        let obs = MemoryObservations::default();
        let account = account();
        let host = identity::create_identity("Host").unwrap();
        let offer = start_offer(&account, &host, &remote).await.unwrap();
        find_offer_and_claim(&account, &offer.short_code, "Guest", &remote, &obs)
            .await
            .unwrap();
        offer_status(&offer.pairing_id, &remote, &obs).await.unwrap();

        let approval = PairingFile {
            protocol_version: 1,
            kind: "approval".to_string(),
            pairing_id: offer.pairing_id.clone(),
            short_code: offer.short_code.clone(),
            ttl_seconds: 300,
            vault_id: "different-vault".to_string(),
            ..PairingFile::default()
        };
        write_file(
            &remote,
            &pairing_path(&approval_name(&offer.pairing_id)),
            &approval,
            false,
        )
        .await
        .unwrap();
        let error = join_status(&offer.pairing_id, &remote, &obs)
            .await
            .unwrap_err();
        assert!(error.contains("空间 ID"));
        clear_session();
    }

    #[tokio::test]
    async fn cancel_removes_files_written_by_this_device() {
        let _guard = test_lock();
        clear_session();
        let remote = MemoryRemote::new();
        let obs = MemoryObservations::default();
        let account = account();
        let host = identity::create_identity("Host").unwrap();
        let offer = start_offer(&account, &host, &remote).await.unwrap();
        let offer_path = pairing_path(&offer_name(&offer.short_code, &offer.pairing_id));
        assert!(remote.files.lock().unwrap().contains_key(&offer_path));
        cancel_pairing(&remote, &obs).await.unwrap();
        assert!(!remote.files.lock().unwrap().contains_key(&offer_path));
        assert!(remote.dirs.lock().unwrap().contains(PAIRING_ROOT));
    }

    #[tokio::test]
    async fn expired_offer_is_removed_after_observation_window() {
        let _guard = test_lock();
        clear_session();
        let remote = MemoryRemote::new();
        let obs = MemoryObservations::default();
        let account = account();
        let code = "654321";
        let pairing_id = uuid::Uuid::new_v4().to_string();
        let path = pairing_path(&offer_name(code, &pairing_id));
        write_file(&remote, &path, &offer_file(code, &pairing_id), false)
            .await
            .unwrap();
        obs.seen
            .lock()
            .unwrap()
            .insert(path.clone(), now_ms() - 301_000);
        let error = find_offer_and_claim(&account, code, "Guest", &remote, &obs)
            .await
            .unwrap_err();
        assert!(error.contains("没有找到"));
        assert!(!remote.files.lock().unwrap().contains_key(&path));
    }

    #[test]
    fn expiry_uses_first_observation_and_ttl() {
        let now = 10_000_000i64;
        assert!(!expired(now - 60_000, 300, now));
        assert!(expired(now - 301_000, 300, now));
    }
}

#[cfg(test)]
mod local_webdav_smoke {
    use std::time::Duration;

    use super::tests::{account, MemoryObservations};
    use super::*;
    use crate::services::webdav_space::client::WebdavClient;

    #[tokio::test]
    #[ignore]
    async fn smoke_pairing_against_local_webdav() {
        let client = WebdavClient::new("http://127.0.0.1:18080", "", "", true, Duration::ZERO)
            .unwrap();
        let obs = MemoryObservations::default();
        let account = account();
        let host = identity::create_identity("Host").unwrap();

        clear_session();
        let offer = start_offer(&account, &host, &client).await.unwrap();
        let guest = find_offer_and_claim(
            &account,
            &offer.short_code,
            "Guest",
            &client,
            &obs,
        )
        .await
        .unwrap();
        let host_view = offer_status(&offer.pairing_id, &client, &obs).await.unwrap();
        assert_eq!(guest.verification_code, host_view.verification_code);
        approve_offer(&offer.pairing_id, &client, &obs).await.unwrap();
        let ready = join_status(&offer.pairing_id, &client, &obs).await.unwrap();
        assert_eq!(ready.status, "ready");
        cleanup_pairing_id(&client, &obs, &offer.pairing_id, &[], &[]).await;
        clear_session();

        let join = start_join(&account, "Guest", &client).await.unwrap();
        let host_view = find_join_and_resolve(
            &account,
            &host,
            &join.short_code,
            &client,
            &obs,
        )
        .await
        .unwrap();
        let resolved = join_status(&join.pairing_id, &client, &obs).await.unwrap();
        assert_eq!(resolved.status, "resolved");
        assert_eq!(resolved.verification_code, host_view.verification_code);
        approve_join(&join.pairing_id, &client, &obs).await.unwrap();
        let ready = join_status(&join.pairing_id, &client, &obs).await.unwrap();
        assert_eq!(ready.status, "ready");
        cleanup_pairing_id(&client, &obs, &join.pairing_id, &[], &[]).await;
        clear_session();
    }
}
