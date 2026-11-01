use crate::commands::CommandResult;
use crate::services::sync_engine;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatusResponse {
    pub is_syncing: bool,
    pub last_sync_time: Option<String>,
    pub pending_changes: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResultResponse {
    pub pushed_count: i32,
    pub pulled_count: i32,
    pub conflict_count: i32,
    pub cleanup_count: i32,
    pub tombstone_count: i32,
    pub sync_time: String,
}

#[tauri::command(rename_all = "camelCase", rename = "syncStatus")]
pub async fn sync_status() -> CommandResult<SyncStatusResponse> {
    // 同步进行中会长时间占用数据库锁，同步命令不能在主线程上等锁，
    // 否则每 60 秒的探活就会卡住 UI。
    let status = tokio::task::spawn_blocking(sync_engine::get_status).await;
    match status {
        Ok(s) => CommandResult::success(SyncStatusResponse {
            is_syncing: s.is_syncing,
            last_sync_time: s.last_sync_time,
            pending_changes: s.pending_changes,
        }),
        Err(error) => CommandResult::error(&format!("读取同步状态失败: {}", error)),
    }
}

#[tauri::command(rename_all = "camelCase", rename = "syncTrigger")]
pub async fn sync_trigger() -> CommandResult<SyncResultResponse> {
    match sync_engine::sync_now_async().await {
        Ok(r) => CommandResult::success(SyncResultResponse {
            pushed_count: r.pushed_count,
            pulled_count: r.pulled_count,
            conflict_count: r.conflict_count,
            cleanup_count: r.cleanup_count,
            tombstone_count: r.tombstone_count,
            sync_time: r.sync_time,
        }),
        Err(msg) => CommandResult::error(&msg),
    }
}
