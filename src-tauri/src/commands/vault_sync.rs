use crate::commands::CommandResult;
use crate::services::vault_sync;

#[tauri::command(rename_all = "camelCase", rename = "vaultSyncGetConfig")]
pub fn vault_sync_get_config() -> CommandResult<vault_sync::config::VaultSyncConfigView> {
    CommandResult::success(vault_sync::get_config_view())
}

#[tauri::command(rename_all = "camelCase", rename = "vaultSyncTest")]
pub async fn vault_sync_test(endpoint: String) -> CommandResult<String> {
    match vault_sync::test_connection(&endpoint).await {
        Ok(message) => CommandResult::success(message),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "vaultSyncCreateVault")]
pub async fn vault_sync_create_vault(
    endpoint: String,
    invite_code: String,
    device_name: String,
) -> CommandResult<vault_sync::config::VaultSyncConfigView> {
    match vault_sync::create_vault(&endpoint, &invite_code, &device_name).await {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "vaultSyncStartPairing")]
pub async fn vault_sync_start_pairing() -> CommandResult<vault_sync::PairingCodeView> {
    match vault_sync::start_pairing_code().await {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "vaultSyncPairingStatus")]
pub async fn vault_sync_pairing_status(
    pairing_id: String,
) -> CommandResult<vault_sync::PairingStatusView> {
    match vault_sync::pairing_status(&pairing_id).await {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "vaultSyncConfirmPairing")]
pub async fn vault_sync_confirm_pairing(pairing_id: String) -> CommandResult<()> {
    match vault_sync::confirm_pairing(&pairing_id).await {
        Ok(()) => CommandResult::success(()),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "vaultSyncBeginJoin")]
pub async fn vault_sync_begin_join(
    endpoint: String,
    code: String,
    device_name: String,
) -> CommandResult<vault_sync::JoinStartView> {
    match vault_sync::begin_join(&endpoint, &code, &device_name).await {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "vaultSyncPollJoin")]
pub async fn vault_sync_poll_join(
    pairing_id: String,
) -> CommandResult<vault_sync::JoinPollView> {
    match vault_sync::poll_join(&pairing_id).await {
        Ok(view) => CommandResult::success(view),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "vaultSyncLeave")]
pub async fn vault_sync_leave() -> CommandResult<()> {
    match vault_sync::leave_vault().await {
        Ok(()) => CommandResult::success(()),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "vaultSyncSeedLocal")]
pub fn vault_sync_seed_local() -> CommandResult<usize> {
    match vault_sync::seed_local_missing() {
        Ok(count) => CommandResult::success(count),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "vaultSyncListDevices")]
pub async fn vault_sync_list_devices() -> CommandResult<Vec<vault_sync::DeviceView>> {
    match vault_sync::list_devices().await {
        Ok(devices) => CommandResult::success(devices),
        Err(message) => CommandResult::error(&message),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "vaultSyncRevokeDevice")]
pub async fn vault_sync_revoke_device(device_id: String) -> CommandResult<()> {
    match vault_sync::revoke_device(&device_id).await {
        Ok(()) => CommandResult::success(()),
        Err(message) => CommandResult::error(&message),
    }
}
