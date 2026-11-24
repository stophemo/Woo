use crate::commands::CommandResult;
use crate::services::sync_engine::SyncResult;
use crate::services::webdav_space;

#[tauri::command(rename_all = "camelCase", rename = "webdavSpaceGetConfig")]
pub async fn webdav_space_get_config(
) -> CommandResult<webdav_space::WebdavSpaceConfigView> {
    match webdav_space::get_config().await {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "webdavSpaceConnectAccount")]
pub async fn webdav_space_connect_account(
    endpoint: String,
    username: String,
    password: Option<String>,
) -> CommandResult<webdav_space::account::WebDavAccountView> {
    match webdav_space::connect_account(&endpoint, &username, password).await {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "webdavSpaceJoinState")]
pub fn webdav_space_join_state() -> CommandResult<webdav_space::JoinLocalStateView> {
    match webdav_space::join_local_state() {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "webdavSpaceCreate")]
pub async fn webdav_space_create(
    device_name: String,
) -> CommandResult<webdav_space::SpaceCreatedView> {
    match webdav_space::create_space(&device_name).await {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "webdavSpaceLeave")]
pub async fn webdav_space_leave() -> CommandResult<()> {
    match webdav_space::leave_space().await {
        Ok(()) => CommandResult::success(()),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "webdavSpaceSyncNow")]
pub async fn webdav_space_sync_now() -> CommandResult<SyncResult> {
    match webdav_space::sync_now().await {
        Ok(result) => CommandResult::success(result),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "webdavSpaceStartOffer")]
pub async fn webdav_space_start_offer(
) -> CommandResult<webdav_space::pairing::PairingStartView> {
    match webdav_space::start_offer().await {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "webdavSpaceStartJoin")]
pub async fn webdav_space_start_join(
    device_name: String,
    adopt_peer: bool,
) -> CommandResult<webdav_space::pairing::PairingStartView> {
    match webdav_space::start_join(&device_name, adopt_peer).await {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "webdavSpacePreviewOffer")]
pub async fn webdav_space_preview_offer(
    code: String,
) -> CommandResult<webdav_space::pairing::OfferPreviewView> {
    match webdav_space::preview_offer(&code).await {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "webdavSpaceListDevices")]
pub async fn webdav_space_list_devices(
) -> CommandResult<Vec<webdav_space::devices::DeviceView>> {
    match webdav_space::list_devices().await {
        Ok(devices) => CommandResult::success(devices),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "webdavSpaceFindOffer")]
pub async fn webdav_space_find_offer(
    code: String,
    device_name: String,
    adopt_peer: bool,
) -> CommandResult<webdav_space::pairing::PairingStatusView> {
    match webdav_space::find_offer(&code, &device_name, adopt_peer).await {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "webdavSpaceFindJoin")]
pub async fn webdav_space_find_join(
    code: String,
) -> CommandResult<webdav_space::pairing::PairingStatusView> {
    match webdav_space::find_join(&code).await {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "webdavSpaceOfferStatus")]
pub async fn webdav_space_offer_status(
    pairing_id: String,
) -> CommandResult<webdav_space::pairing::PairingStatusView> {
    match webdav_space::offer_status(&pairing_id).await {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "webdavSpaceJoinStatus")]
pub async fn webdav_space_join_status(
    pairing_id: String,
) -> CommandResult<webdav_space::pairing::PairingStatusView> {
    match webdav_space::join_status(&pairing_id).await {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "webdavSpaceApproveOffer")]
pub async fn webdav_space_approve_offer(
    pairing_id: String,
) -> CommandResult<webdav_space::pairing::PairingStatusView> {
    match webdav_space::approve_offer(&pairing_id).await {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "webdavSpaceApproveJoin")]
pub async fn webdav_space_approve_join(
    pairing_id: String,
) -> CommandResult<webdav_space::pairing::PairingStatusView> {
    match webdav_space::approve_join(&pairing_id).await {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "webdavSpaceCancelPairing")]
pub async fn webdav_space_cancel_pairing() -> CommandResult<()> {
    match webdav_space::cancel_pairing().await {
        Ok(()) => CommandResult::success(()),
        Err(message) => CommandResult::error(&message),
    }
}
