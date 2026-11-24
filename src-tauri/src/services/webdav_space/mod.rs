//! WebDAV 同步空间：账号 + 端到端加密操作日志 + 六位码配对。
//!
//! 远端布局：
//! `v1/<vaultId>/ops/<shard>/<opId>.json`
//! `v1/<vaultId>/space.json`
//! `v1/pairing/<offer|claim|join|resolve|approval>-....json`

pub mod account;
pub mod client;
pub mod identity;
pub mod pairing;
pub mod secure;
pub mod sync;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::db;
use crate::services::sync_engine::{SyncResult, SyncStatus};
use crate::services::vault_sync::store;

use self::account::{WebDavAccount, WebDavAccountView};
use self::client::{ensure_collection, RemoteStore, WebdavClient};
use self::identity::{
    identity_is_active, space_fingerprint, WebDavIdentity, WebDavIdentityView,
};
use self::pairing::{DbObservations, PairingStatusView};

static CONFIG_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebdavSpaceConfigView {
    pub account: WebDavAccountView,
    pub identity: WebDavIdentityView,
    pub space_configured: bool,
    pub active: bool,
    pub last_sync_time: Option<String>,
    pub pending_changes: i32,
    pub notice: Option<String>,
    pub legacy_notice: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceCreatedView {
    pub vault_id: String,
    pub device_id: String,
    pub device_name: String,
    pub space_fingerprint: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpaceMetadata {
    protocol_version: u32,
    vault_id: String,
    created_at: i64,
}

pub fn is_enabled() -> bool {
    match (
        account::configured_account(),
        identity::configured_identity(),
    ) {
        (Ok(Some(account)), Ok(Some(identity))) => {
            account::account_is_active(&account) && identity_is_active(&identity)
        }
        _ => false,
    }
}

pub fn status() -> SyncStatus {
    let state = db::with_db(|conn| store::load_state(conn)).unwrap_or_default();
    SyncStatus {
        is_syncing: false,
        last_sync_time: state.last_success_at,
        pending_changes: db::with_db(|conn| store::count_pending_changes(conn)) as i32,
        backend: "webdav".to_string(),
    }
}

fn client_for(account: &WebDavAccount) -> Result<WebdavClient, String> {
    WebdavClient::from_url(&account.endpoint, &account.username, &account.app_password)
}

fn active_account() -> Result<WebDavAccount, String> {
    account::configured_account()?
        .filter(account::account_is_active)
        .ok_or_else(|| "请先配置并测试 WebDAV 账号".to_string())
}

fn active_identity() -> Result<WebDavIdentity, String> {
    identity::configured_identity()?
        .filter(identity_is_active)
        .ok_or_else(|| "请先建立同步空间或输入配对码加入".to_string())
}

fn legacy_notice() -> Option<String> {
    let dir = db::data_dir();
    let has_worker = dir.join("vault-sync-config.json").exists();
    let has_lan = dir.join("lan-sync-config.json").exists();
    if has_worker || has_lan {
        Some(
            "检测到旧版 Worker / 局域网同步配置，已停止使用。请按 WebDAV 账号重新配置。"
                .to_string(),
        )
    } else {
        None
    }
}

fn migrate_legacy_best_effort() -> bool {
    match account::migrate_legacy_account() {
        Ok(migrated) => migrated,
        Err(error) => {
            log::warn!("[WebDAV] 旧账号迁移失败：{}", error);
            false
        }
    }
}

async fn has_existing_space(store: &dyn RemoteStore) -> Result<bool, String> {
    let entries = store
        .list("v1")
        .await
        .map_err(|error| error.to_string())?;
    Ok(entries.into_iter().any(|entry| {
        entry.is_dir && uuid::Uuid::parse_str(&entry.name).is_ok()
    }))
}

async fn ensure_space_metadata(
    store: &dyn RemoteStore,
    identity: &WebDavIdentity,
) -> Result<(), String> {
    let root = format!("v1/{}", identity.vault_id);
    ensure_collection(store, &root)
        .await
        .map_err(|error| error.to_string())?;
    ensure_collection(store, &sync::ops_root(&identity.vault_id))
        .await
        .map_err(|error| error.to_string())?;
    let metadata = SpaceMetadata {
        protocol_version: 1,
        vault_id: identity.vault_id.clone(),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bytes = serde_json::to_vec(&metadata)
        .map_err(|error| format!("序列化空间信息失败：{error}"))?;
    let path = format!("{root}/space.json");
    match store.put(&path, &bytes, true).await {
        Ok(()) => Ok(()),
        Err(client::RemoteError::Conflict) => {
            let existing = store
                .get(&path)
                .await
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "空间信息写入冲突，请稍后重试".to_string())?;
            let existing: SpaceMetadata = serde_json::from_slice(&existing)
                .map_err(|_| "云端空间信息格式损坏".to_string())?;
            if existing.vault_id == identity.vault_id {
                Ok(())
            } else {
                Err("云端空间信息与当前空间 ID 不一致，已停止".to_string())
            }
        }
        Err(error) => Err(error.to_string()),
    }
}

fn prepare_binding(
    identity: &WebDavIdentity,
    seed_local: bool,
    lamport_floor: Option<i64>,
) -> Result<(), String> {
    let _ = identity::decode_vault_key(identity)?;
    db::with_db(|conn| {
        store::clear_sync_data(conn)?;
        store::set_binding(conn, &identity.vault_id, &identity.device_id, true)?;
        store::set_active(conn, true)?;
        if let Some(floor) = lamport_floor {
            store::set_lamport_at_least(conn, floor)?;
        }
        if seed_local {
            store::enqueue_all_local(conn)?;
        }
        Ok::<(), String>(())
    })?;
    Ok(())
}

pub async fn get_config() -> Result<WebdavSpaceConfigView, String> {
    let _guard = CONFIG_LOCK.lock().await;
    let migrated_legacy = migrate_legacy_best_effort();
    let mut notice = None;
    let account = account::load_account()?;
    let mut identity = identity::load_identity()?;

    if let Some(account) = account.as_ref().filter(|account| account::account_is_active(account)) {
        let client = client_for(account)?;
        let _ = pairing::cleanup_expired(&client, &DbObservations).await;
        if identity.is_none() {
            match has_existing_space(&client).await {
                Ok(true) => {
                    notice = Some(
                        "云端已有同步空间。请输入已有设备显示的六位配对码加入；若这是旧设备，请勿再建立新空间。"
                            .to_string(),
                    );
                }
                Ok(false) if migrated_legacy => {
                    let created = identity::create_identity(
                        &crate::services::vault_sync::config::default_device_name(),
                    )?;
                    ensure_space_metadata(&client, &created).await?;
                    identity::save_identity(&created)?;
                    if let Err(error) = prepare_binding(&created, true, None) {
                        let _ = identity::delete_identity();
                        return Err(error);
                    }
                    identity = Some(created);
                    notice = Some(
                        "已根据旧版 WebDAV 配置建立同步空间。其他设备需要用六位配对码重新加入。"
                            .to_string(),
                    );
                }
                Ok(false) => {}
                Err(error) => {
                    notice = Some(format!("暂时无法检查云端同步空间：{}", error));
                }
            }
        }
    }

    let account_view = account::account_view(account.as_ref());
    let identity_view = identity::identity_view(identity.as_ref());
    let state = db::with_db(|conn| store::load_state(conn)).unwrap_or_default();
    let active = account_view.active && identity_view.active;
    Ok(WebdavSpaceConfigView {
        account: account_view,
        identity: identity_view,
        space_configured: identity.is_some(),
        active,
        last_sync_time: state.last_success_at,
        pending_changes: db::with_db(|conn| store::count_pending_changes(conn)) as i32,
        notice,
        legacy_notice: legacy_notice(),
    })
}

/// 测试连接并保存账号。应用密码只在内存中用于测试，落盘前加密。
pub async fn connect_account(
    endpoint: &str,
    username: &str,
    password: Option<String>,
) -> Result<WebDavAccountView, String> {
    let endpoint = account::normalize_endpoint(endpoint)?;
    let username = username.trim().to_string();
    if username.is_empty() {
        return Err("请填写 WebDAV 用户名".to_string());
    }
    let existing = account::load_account()?;
    if let Some(existing) = existing.as_ref() {
        let has_identity = identity::load_identity()?.is_some();
        if has_identity
            && (existing.endpoint != endpoint || existing.username != username)
        {
            return Err("更换 WebDAV 账号前请先退出同步空间，避免把身份带到错误的空间".to_string());
        }
    }
    let can_reuse = existing.as_ref().is_some_and(|account| {
        account.bound_user == db::current_user_key()
            && account.endpoint == endpoint
            && account.username == username
    });
    let password = match password {
        Some(value) if !value.is_empty() => value,
        _ if can_reuse => existing
            .as_ref()
            .map(|account| account.app_password.clone())
            .unwrap_or_default(),
        _ => String::new(),
    };
    if password.is_empty() {
        return Err("请填写 WebDAV 应用密码".to_string());
    }
    let client = WebdavClient::new(
        &endpoint,
        &username,
        &password,
        false,
        std::time::Duration::from_secs(1),
    )?;
    client.test_connection().await?;
    account::save_account(&endpoint, &username, Some(password))
}

pub async fn create_space(device_name: &str) -> Result<SpaceCreatedView, String> {
    let account = active_account()?;
    if identity::load_identity()?.is_some() {
        return Err("当前设备已经在同步空间中；如需更换请先退出".to_string());
    }
    let client = client_for(&account)?;
    let identity = identity::create_identity(device_name)?;
    ensure_space_metadata(&client, &identity).await?;
    identity::save_identity(&identity)?;
    if let Err(error) = prepare_binding(&identity, true, None) {
        let _ = identity::delete_identity();
        return Err(error);
    }
    log::info!("[WebDAV] 已建立同步空间 {}", identity.vault_id);
    Ok(SpaceCreatedView {
        vault_id: identity.vault_id.clone(),
        device_id: identity.device_id.clone(),
        device_name: identity.device_name.clone(),
        space_fingerprint: space_fingerprint(&identity.vault_id),
    })
}

pub async fn leave_space() -> Result<(), String> {
    if let Some(account) = account::configured_account()? {
        if let Ok(client) = client_for(&account) {
            let _ = pairing::cancel_pairing(&client, &DbObservations).await;
        }
    }
    identity::delete_identity()?;
    db::with_db(|conn| store::clear_sync_data(conn))?;
    pairing::clear_session();
    log::info!("[WebDAV] 已退出同步空间，仅清除本机身份");
    Ok(())
}

pub async fn sync_now() -> Result<SyncResult, String> {
    let account = active_account()?;
    let identity = active_identity()?;
    let client = client_for(&account)?;
    match sync::sync_now(&client, &identity).await {
        Ok(result) => Ok(result),
        Err(error) => {
            let _ = db::with_db(|conn| store::set_last_error(conn, &error));
            Err(error)
        }
    }
}

pub async fn start_offer() -> Result<pairing::PairingStartView, String> {
    let account = active_account()?;
    let identity = active_identity()?;
    let client = client_for(&account)?;
    pairing::start_offer(&account, &identity, &client).await
}

pub async fn start_join(device_name: &str) -> Result<pairing::PairingStartView, String> {
    let account = active_account()?;
    let client = client_for(&account)?;
    pairing::start_join(&account, device_name, &client).await
}

pub async fn find_offer(
    code: &str,
    device_name: &str,
) -> Result<PairingStatusView, String> {
    let account = active_account()?;
    let client = client_for(&account)?;
    pairing::find_offer_and_claim(
        &account,
        code,
        device_name,
        &client,
        &DbObservations,
    )
    .await
}

pub async fn find_join(code: &str) -> Result<PairingStatusView, String> {
    let account = active_account()?;
    let identity = active_identity()?;
    let client = client_for(&account)?;
    pairing::find_join_and_resolve(&account, &identity, code, &client, &DbObservations).await
}

pub async fn offer_status(pairing_id: &str) -> Result<PairingStatusView, String> {
    let account = active_account()?;
    let client = client_for(&account)?;
    pairing::offer_status(pairing_id, &client, &DbObservations).await
}

pub async fn join_status(pairing_id: &str) -> Result<PairingStatusView, String> {
    let account = active_account()?;
    let client = client_for(&account)?;
    pairing::join_status(pairing_id, &client, &DbObservations).await
}

pub async fn approve_offer(pairing_id: &str) -> Result<PairingStatusView, String> {
    let account = active_account()?;
    let client = client_for(&account)?;
    pairing::approve_offer(pairing_id, &client, &DbObservations).await
}

pub async fn approve_join(pairing_id: &str) -> Result<PairingStatusView, String> {
    let account = active_account()?;
    let client = client_for(&account)?;
    pairing::approve_join(pairing_id, &client, &DbObservations).await
}

pub async fn confirm_join(pairing_id: &str) -> Result<pairing::JoinConfirmView, String> {
    let account = active_account()?;
    let client = client_for(&account)?;
    let confirmed = pairing::confirm_join(pairing_id, &client, &DbObservations).await?;
    let floor = match sync::remote_max_lamport(&client, &confirmed.identity.vault_id).await {
        Ok(floor) => floor,
        Err(error) => {
            let _ = identity::delete_identity();
            return Err(format!("读取远端 Lamport 水位失败，请重新配对：{}", error));
        }
    };
    if let Err(error) = prepare_binding(&confirmed.identity, false, Some(floor)) {
        let _ = identity::delete_identity();
        return Err(error);
    }
    Ok(confirmed.view)
}

pub async fn cancel_pairing() -> Result<(), String> {
    let account = account::configured_account()?;
    match account {
        Some(account) => {
            let client = client_for(&account)?;
            pairing::cancel_pairing(&client, &DbObservations).await
        }
        None => {
            pairing::clear_session();
            Ok(())
        }
    }
}
