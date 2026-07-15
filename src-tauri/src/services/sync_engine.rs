use crate::db;
use crate::services::utils::now_str;
use crate::supabase;
use chrono::{Duration, Utc};
use once_cell::sync::Lazy;
use rusqlite::OptionalExtension;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

const ENTITIES: &[(&str, &str)] = &[
    ("note_folder", "id"),
    ("note_document", "id"),
    ("note_document_version", "id"),
];

const EMPTY_DB_FULL_SYNC_KEY: &str = "empty_db_full_sync_v1";

// ===================== Sync State =====================

static IS_SYNCING: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));

struct SyncGuard;

impl SyncGuard {
    fn acquire() -> Result<Self, String> {
        let mut is_syncing = IS_SYNCING
            .lock()
            .map_err(|_| "同步状态锁已损坏".to_string())?;
        if *is_syncing {
            return Err("同步正在进行中".to_string());
        }
        *is_syncing = true;
        Ok(Self)
    }
}

impl Drop for SyncGuard {
    fn drop(&mut self) {
        if let Ok(mut is_syncing) = IS_SYNCING.lock() {
            *is_syncing = false;
        }
    }
}

// ===================== Data Structures =====================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub is_syncing: bool,
    pub last_sync_time: Option<String>,
    pub pending_changes: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub pushed_count: i32,
    pub pulled_count: i32,
    pub conflict_count: i32,
    pub cleanup_count: i32,
    pub tombstone_count: i32,
    pub sync_time: String,
}

// ===================== Session Helpers =====================

fn get_current_user_id() -> Option<String> {
    supabase::CURRENT_SESSION
        .lock()
        .ok()
        .and_then(|s| s.clone())
        .map(|s| s.user.id)
}

fn get_current_access_token() -> Option<String> {
    supabase::CURRENT_SESSION
        .lock()
        .ok()
        .and_then(|s| s.clone())
        .map(|s| s.access_token)
}

// ===================== Sync Meta Helpers =====================

fn try_get_sync_meta(key: &str) -> Result<Option<String>, String> {
    db::with_db(|conn| {
        let mut stmt = conn
            .prepare("SELECT value FROM sync_meta WHERE key = ?")
            .map_err(|e| format!("读取同步元数据失败: {}", e))?;
        stmt.query_row([key], |row| row.get(0))
            .optional()
            .map_err(|e| format!("读取同步元数据失败: {}", e))
    })
}

pub fn get_sync_meta(key: &str) -> Option<String> {
    try_get_sync_meta(key).ok().flatten()
}

fn try_set_sync_meta(key: &str, value: &str) -> Result<(), String> {
    db::with_db(|conn| {
        conn.execute(
            "INSERT OR REPLACE INTO sync_meta (key, value) VALUES (?, ?)",
            [key, value],
        )
        .map(|_| ())
        .map_err(|e| format!("写入同步元数据失败: {}", e))
    })
}

pub fn get_last_sync_time() -> Option<String> {
    get_sync_meta("last_sync_time")
}

#[derive(Debug, PartialEq)]
struct SyncCursorPlan {
    cursor: Option<String>,
    mark_empty_full_sync: bool,
    is_empty_db_recovery: bool,
}

fn build_sync_cursor_plan(
    last_sync: Option<String>,
    core_record_count: i64,
    empty_full_sync_completed: bool,
) -> SyncCursorPlan {
    if core_record_count == 0 && !empty_full_sync_completed {
        return SyncCursorPlan {
            is_empty_db_recovery: last_sync.is_some(),
            cursor: None,
            mark_empty_full_sync: true,
        };
    }

    SyncCursorPlan {
        cursor: last_sync,
        mark_empty_full_sync: false,
        is_empty_db_recovery: false,
    }
}

fn count_core_records() -> Result<i64, String> {
    db::with_db(|conn| {
        let mut total = 0i64;
        for &(table, _) in ENTITIES {
            let sql = format!("SELECT COUNT(*) FROM {}", table);
            let count: i64 = conn
                .query_row(&sql, [], |row| row.get(0))
                .map_err(|e| format!("统计本地同步数据失败 [{}]: {}", table, e))?;
            total += count;
        }
        Ok(total)
    })
}

fn commit_sync_metadata(sync_time: &str, mark_empty_full_sync: bool) -> Result<(), String> {
    db::with_db(|conn| {
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("开始提交同步游标失败: {}", e))?;
        tx.execute(
            "INSERT OR REPLACE INTO sync_meta (key, value) VALUES ('last_sync_time', ?)",
            [sync_time],
        )
        .map_err(|e| format!("提交同步游标失败: {}", e))?;
        if mark_empty_full_sync {
            tx.execute(
                "INSERT OR REPLACE INTO sync_meta (key, value) VALUES (?, ?)",
                [EMPTY_DB_FULL_SYNC_KEY, sync_time],
            )
            .map_err(|e| format!("记录空库全量同步状态失败: {}", e))?;
        }
        tx.commit()
            .map_err(|e| format!("提交同步元数据失败: {}", e))
    })
}

// ===================== Push Logic =====================

fn query_local_changes(
    table: &str,
    last_sync: Option<&str>,
) -> Result<Vec<serde_json::Value>, String> {
    let last_sync = last_sync.unwrap_or("1970-01-01T00:00:00");
    db::with_db(|conn| {
        let sql = format!(
            "SELECT * FROM {} WHERE update_time > ? ORDER BY update_time ASC",
            table
        );
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| format!("查询本地变更失败 [{}]: {}", table, e))?;
        let col_count = stmt.column_count();
        let col_names: Vec<String> = (0..col_count)
            .map(|i| stmt.column_name(i).map(String::from))
            .collect::<Result<_, _>>()
            .map_err(|e| format!("读取本地字段失败 [{}]: {}", table, e))?;

        let rows = stmt
            .query_map([last_sync], |row| {
                let mut map = serde_json::Map::new();
                for (i, name) in col_names.iter().enumerate() {
                    let value = match row.get_ref(i)? {
                        rusqlite::types::ValueRef::Null => serde_json::Value::Null,
                        rusqlite::types::ValueRef::Integer(v) => serde_json::Value::from(v),
                        rusqlite::types::ValueRef::Real(v) => serde_json::Value::from(v),
                        rusqlite::types::ValueRef::Text(v) => {
                            serde_json::Value::String(String::from_utf8_lossy(v).into_owned())
                        }
                        rusqlite::types::ValueRef::Blob(v) => {
                            serde_json::Value::String(String::from_utf8_lossy(v).into_owned())
                        }
                    };
                    map.insert(name.clone(), value);
                }
                Ok(serde_json::Value::Object(map))
            })
            .map_err(|e| format!("查询本地变更失败 [{}]: {}", table, e))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取本地变更失败 [{}]: {}", table, e))
    })
}

/// Push local changes to Supabase.
async fn push_changes(
    user_id: &str,
    access_token: &str,
    last_sync: Option<&str>,
    skip_ids: Option<&HashMap<String, HashSet<String>>>,
) -> Result<i32, String> {
    let mut pushed_count = 0i32;

    for &(table, id_field) in ENTITIES {
        let mut local_rows = query_local_changes(table, last_sync)?;
        if local_rows.is_empty() {
            continue;
        }

        // Filter out echo rows (pulled this round)
        if let Some(skip_map) = skip_ids {
            if let Some(skip_set) = skip_map.get(table) {
                local_rows.retain(|row| {
                    row.get(id_field)
                        .and_then(|v| v.as_str())
                        .map_or(true, |id| !skip_set.contains(id))
                });
                if local_rows.is_empty() {
                    continue;
                }
            }
        }

        // Add user_id to each row
        let rows_with_user: Vec<serde_json::Value> = local_rows
            .into_iter()
            .map(|mut row| {
                if let serde_json::Value::Object(ref mut map) = row {
                    map.insert(
                        "user_id".to_string(),
                        serde_json::Value::String(user_id.to_string()),
                    );
                }
                row
            })
            .collect();

        supabase::upsert(table, &rows_with_user, access_token)
            .await
            .map_err(|e| format!("推送失败 [{}]: {}", table, e))?;
        pushed_count += rows_with_user.len() as i32;
    }

    Ok(pushed_count)
}

// ===================== Pull Logic =====================

/// Get the set of column names that exist in the local table.
fn get_local_columns(table: &str) -> Result<HashSet<String>, String> {
    db::with_db(|conn| {
        let sql = format!("PRAGMA table_info({})", table);
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| format!("读取本地表结构失败 [{}]: {}", table, e))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| format!("读取本地表结构失败 [{}]: {}", table, e))?;
        rows.collect::<Result<HashSet<_>, _>>()
            .map_err(|e| format!("读取本地表结构失败 [{}]: {}", table, e))
    })
}

/// Merge remote rows into local SQLite (last-write-wins).
/// Returns (pulled_count, conflict_count, pulled_ids).
fn merge_into_local(
    table: &str,
    id_field: &str,
    remote_rows: &[serde_json::Value],
) -> Result<(i32, i32, Vec<String>), String> {
    if remote_rows.is_empty() {
        return Ok((0, 0, Vec::new()));
    }

    let local_cols = get_local_columns(table)?;

    db::with_db(|conn| {
        let mut pulled = 0i32;
        let mut conflicts = 0i32;
        let mut pulled_ids = Vec::new();

        // Prepare check statement
        let check_sql = format!("SELECT update_time FROM {} WHERE {} = ?", table, id_field);
        let mut check_stmt = conn
            .prepare(&check_sql)
            .map_err(|e| format!("检查本地记录失败 [{}]: {}", table, e))?;

        for row in remote_rows {
            let obj = row
                .as_object()
                .ok_or_else(|| format!("远端记录格式无效 [{}]", table))?;

            let id = obj
                .get(id_field)
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("远端记录缺少主键 [{}]", table))?
                .to_string();

            let remote_update_time = obj
                .get("update_time")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("远端记录缺少更新时间 [{}:{}]", table, id))?;

            // Check local record
            let local_wins = check_stmt
                .query_row([&id], |r| {
                    let local_time: String = r.get(0)?;
                    Ok(local_time)
                })
                .optional()
                .map_err(|e| format!("检查本地记录失败 [{}:{}]: {}", table, id, e))?
                .map_or(false, |local_time| {
                    local_time.as_str() >= remote_update_time
                });

            if local_wins {
                conflicts += 1;
                continue;
            }

            // Build INSERT OR REPLACE with only locally-existing columns, excluding user_id
            let mut keys = Vec::new();
            let mut vals: Vec<rusqlite::types::Value> = Vec::new();

            for (k, v) in obj {
                if k == "user_id" || !local_cols.contains(k) {
                    continue;
                }
                keys.push(k.clone());
                let val = match v {
                    serde_json::Value::Null => rusqlite::types::Value::Null,
                    serde_json::Value::String(s) => rusqlite::types::Value::Text(s.clone()),
                    serde_json::Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            rusqlite::types::Value::Integer(i)
                        } else if let Some(f) = n.as_f64() {
                            rusqlite::types::Value::Real(f)
                        } else {
                            rusqlite::types::Value::Text(n.to_string())
                        }
                    }
                    serde_json::Value::Bool(b) => {
                        rusqlite::types::Value::Integer(if *b { 1 } else { 0 })
                    }
                    _ => rusqlite::types::Value::Text(v.to_string()),
                };
                vals.push(val);
            }

            if keys.is_empty() {
                return Err(format!("远端记录没有可写入字段 [{}:{}]", table, id));
            }

            let placeholders: Vec<String> = keys.iter().map(|_| "?".to_string()).collect();
            let insert_sql = format!(
                "INSERT OR REPLACE INTO {} ({}) VALUES ({})",
                table,
                keys.join(", "),
                placeholders.join(", ")
            );

            let mut stmt = conn
                .prepare(&insert_sql)
                .map_err(|e| format!("准备合并远端记录失败 [{}:{}]: {}", table, id, e))?;
            stmt.execute(rusqlite::params_from_iter(vals.iter()))
                .map_err(|e| format!("合并远端记录失败 [{}:{}]: {}", table, id, e))?;
            pulled += 1;
            pulled_ids.push(id);
        }

        Ok((pulled, conflicts, pulled_ids))
    })
}

/// Pull remote changes from Supabase and merge into local.
async fn pull_changes(
    user_id: &str,
    access_token: &str,
    last_sync: Option<&str>,
) -> Result<(i32, i32, HashMap<String, HashSet<String>>), String> {
    let mut pulled_count = 0i32;
    let mut conflict_count = 0i32;
    let mut pulled_ids_map: HashMap<String, HashSet<String>> = HashMap::new();

    for &(table, id_field) in ENTITIES {
        let remote_rows = supabase::select(table, user_id, last_sync, access_token)
            .await
            .map_err(|e| format!("拉取失败 [{}]: {}", table, e))?;

        if remote_rows.is_empty() {
            continue;
        }

        let (pulled, conflicts, pulled_ids) = merge_into_local(table, id_field, &remote_rows)?;
        pulled_count += pulled;
        conflict_count += conflicts;

        if !pulled_ids.is_empty() {
            let set: HashSet<String> = pulled_ids.into_iter().collect();
            pulled_ids_map.insert(table.to_string(), set);
        }
    }

    Ok((pulled_count, conflict_count, pulled_ids_map))
}

// ===================== Tombstone Logic =====================

async fn pull_tombstones(user_id: &str, access_token: &str) -> Result<i32, String> {
    let last_pull = try_get_sync_meta("last_tombstone_pull")?
        .unwrap_or_else(|| "1970-01-01T00:00:00".to_string());

    let tombstones = supabase::select_tombstones(user_id, &last_pull, access_token)
        .await
        .map_err(|e| format!("拉取墓碑失败: {}", e))?;

    if tombstones.is_empty() {
        return Ok(0);
    }

    let count = db::with_db(|conn| -> Result<i32, String> {
        let mut count = 0i32;
        for t in &tombstones {
            let tn = t
                .get("table_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "墓碑缺少 table_name".to_string())?;
            let rid = t
                .get("record_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "墓碑缺少 record_id".to_string())?;
            if !ENTITIES.iter().any(|(table, _)| *table == tn) {
                return Err(format!("墓碑包含不支持的数据表: {}", tn));
            }

            // Cascade delete versions for documents
            if tn == "note_document" {
                conn.execute(
                    "DELETE FROM note_document_version WHERE document_id = ?",
                    [rid],
                )
                .map_err(|e| format!("应用文稿版本墓碑失败 [{}]: {}", rid, e))?;
            }

            let sql = format!("DELETE FROM {} WHERE id = ?", tn);
            conn.execute(&sql, [rid])
                .map_err(|e| format!("应用墓碑失败 [{}:{}]: {}", tn, rid, e))?;
            count += 1;
        }
        Ok(count)
    })?;

    // Update last tombstone pull time
    if let Some(latest) = tombstones
        .iter()
        .filter_map(|t| t.get("deleted_at").and_then(|v| v.as_str()))
        .max()
    {
        try_set_sync_meta("last_tombstone_pull", latest)?;
    } else {
        return Err("墓碑缺少 deleted_at".to_string());
    }

    // Cleanup old tombstones (30 day cutoff)
    let cutoff = (Utc::now() - Duration::days(30))
        .format("%Y-%m-%dT%H:%M:%S%.f+00:00")
        .to_string();

    if let Err(e) = supabase::cleanup_tombstones(user_id, &cutoff, access_token).await {
        log::warn!("[Sync] 墓碑垃圾回收失败（不影响本轮同步）: {}", e);
    }

    Ok(count)
}

// ===================== Cleanup Logic =====================

fn parse_cleanup_seconds() -> i64 {
    match std::env::var("SYNC_CLEANUP_SECONDS") {
        Ok(expr) => {
            if expr
                .chars()
                .all(|c| c.is_ascii_digit() || c == '*' || c == ' ')
            {
                let parts: Vec<i64> = expr
                    .split('*')
                    .filter_map(|s| s.trim().parse::<i64>().ok())
                    .collect();
                if parts.is_empty() || parts.iter().any(|&p| p <= 0) {
                    return 7 * 24 * 3600;
                }
                return parts.iter().product();
            }
            7 * 24 * 3600
        }
        Err(_) => 7 * 24 * 3600,
    }
}

/// Cleanup expired deleted=2 records.
async fn cleanup_expired_deletes(user_id: &str, access_token: &str) -> Result<i32, String> {
    let cleanup_seconds = parse_cleanup_seconds();
    let cutoff = (Utc::now() - Duration::seconds(cleanup_seconds))
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();

    let mut cleanup_count = 0i32;

    // 1. Cleanup expired documents (deleted=2 and update_time < cutoff)
    let expired_docs = db::with_db(|conn| -> Result<Vec<String>, String> {
        let mut stmt = conn
            .prepare("SELECT id FROM note_document WHERE deleted = 2 AND update_time < ?")
            .map_err(|e| format!("查询待清理文稿失败: {}", e))?;
        let rows = stmt
            .query_map([&cutoff], |row| row.get(0))
            .map_err(|e| format!("查询待清理文稿失败: {}", e))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取待清理文稿失败: {}", e))
    })?;

    for doc_id in &expired_docs {
        supabase::delete(
            "note_document_version",
            "document_id",
            &[doc_id],
            access_token,
        )
        .await
        .map_err(|e| format!("清理远端文稿版本失败 [{}]: {}", doc_id, e))?;
        supabase::delete("note_document", "id", &[doc_id], access_token)
            .await
            .map_err(|e| format!("清理远端文稿失败 [{}]: {}", doc_id, e))?;
        supabase::insert_tombstone("note_document", doc_id, user_id, access_token)
            .await
            .map_err(|e| format!("写入文稿墓碑失败 [{}]: {}", doc_id, e))?;

        db::with_db(|conn| -> Result<(), String> {
            conn.execute(
                "DELETE FROM note_document_version WHERE document_id = ?",
                [doc_id],
            )
            .map_err(|e| format!("清理本地文稿版本失败 [{}]: {}", doc_id, e))?;
            conn.execute("DELETE FROM note_document WHERE id = ?", [doc_id])
                .map_err(|e| format!("清理本地文稿失败 [{}]: {}", doc_id, e))?;
            Ok(())
        })?;
        cleanup_count += 1;
    }

    // 2. Promote empty folders (placeholder - calls stub)
    if !expired_docs.is_empty() {
        promote_empty_folders_to_deleted2();
    }

    // 3. Cleanup expired folders
    let expired_folders = db::with_db(|conn| -> Result<Vec<String>, String> {
        let mut stmt = conn
            .prepare("SELECT id FROM note_folder WHERE deleted = 2 AND update_time < ?")
            .map_err(|e| format!("查询待清理文件夹失败: {}", e))?;
        let rows = stmt
            .query_map([&cutoff], |row| row.get(0))
            .map_err(|e| format!("查询待清理文件夹失败: {}", e))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取待清理文件夹失败: {}", e))
    })?;

    for folder_id in &expired_folders {
        supabase::delete("note_folder", "id", &[folder_id], access_token)
            .await
            .map_err(|e| format!("清理远端文件夹失败 [{}]: {}", folder_id, e))?;
        supabase::insert_tombstone("note_folder", folder_id, user_id, access_token)
            .await
            .map_err(|e| format!("写入文件夹墓碑失败 [{}]: {}", folder_id, e))?;
        db::with_db(|conn| -> Result<(), String> {
            conn.execute("DELETE FROM note_folder WHERE id = ?", [folder_id])
                .map_err(|e| format!("清理本地文件夹失败 [{}]: {}", folder_id, e))?;
            Ok(())
        })?;
        cleanup_count += 1;
    }

    // 4. Cleanup expired versions
    let expired_versions = db::with_db(|conn| -> Result<Vec<String>, String> {
        let mut stmt = conn
            .prepare("SELECT id FROM note_document_version WHERE deleted = 2 AND update_time < ?")
            .map_err(|e| format!("查询待清理文稿版本失败: {}", e))?;
        let rows = stmt
            .query_map([&cutoff], |row| row.get(0))
            .map_err(|e| format!("查询待清理文稿版本失败: {}", e))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取待清理文稿版本失败: {}", e))
    })?;

    for ver_id in &expired_versions {
        supabase::delete("note_document_version", "id", &[ver_id], access_token)
            .await
            .map_err(|e| format!("清理远端文稿版本失败 [{}]: {}", ver_id, e))?;
        db::with_db(|conn| -> Result<(), String> {
            conn.execute("DELETE FROM note_document_version WHERE id = ?", [ver_id])
                .map_err(|e| format!("清理本地文稿版本失败 [{}]: {}", ver_id, e))?;
            Ok(())
        })?;
        cleanup_count += 1;
    }

    Ok(cleanup_count)
}

/// Placeholder: promote empty folder chains to deleted=2.
fn promote_empty_folders_to_deleted2() {
    log::debug!("[Sync] Would promote empty folders to deleted=2");
}

// ===================== Orchestration =====================

/// Execute a full sync cycle: tombstone pull → remote pull → local push → cleanup.
async fn do_sync() -> Result<SyncResult, String> {
    let user_id = get_current_user_id().ok_or("未登录，无法同步".to_string())?;
    let access_token = get_current_access_token().ok_or("未登录，无法同步".to_string())?;

    // 在任何网络请求前固定水位。同步过程中产生的变更会在下一轮再次覆盖，避免结束时间跳过数据。
    let sync_time = now_str();
    let last_sync = try_get_sync_meta("last_sync_time")?;
    let core_record_count = count_core_records()?;
    let empty_full_sync_completed = try_get_sync_meta(EMPTY_DB_FULL_SYNC_KEY)?.is_some();
    let cursor_plan =
        build_sync_cursor_plan(last_sync, core_record_count, empty_full_sync_completed);

    if cursor_plan.is_empty_db_recovery {
        log::warn!("[Sync] 检测到空用户库和历史游标，本轮执行一次安全全量拉取");
    }

    // 1. Pull tombstones
    let tombstone_count = pull_tombstones(&user_id, &access_token).await?;

    // 2. Pull remote changes
    let (pulled_count, conflict_count, pulled_ids) =
        pull_changes(&user_id, &access_token, cursor_plan.cursor.as_deref()).await?;

    // 3. Push local changes
    let pushed_count = push_changes(
        &user_id,
        &access_token,
        cursor_plan.cursor.as_deref(),
        Some(&pulled_ids),
    )
    .await?;

    // 4. Cleanup expired deletes
    let cleanup_count = cleanup_expired_deletes(&user_id, &access_token).await?;

    // 所有核心阶段成功后，才以事务方式推进本轮开始水位和空库恢复标记。
    commit_sync_metadata(&sync_time, cursor_plan.mark_empty_full_sync)?;

    log::info!(
        "[Sync] ⇄ push={} pull={} conflict={} cleanup={} tombstone={}",
        pushed_count,
        pulled_count,
        conflict_count,
        cleanup_count,
        tombstone_count
    );

    Ok(SyncResult {
        pushed_count,
        pulled_count,
        conflict_count,
        cleanup_count,
        tombstone_count,
        sync_time,
    })
}

/// Run a full sync cycle (async, with concurrency guard).
pub async fn sync_now_async() -> Result<SyncResult, String> {
    let _guard = SyncGuard::acquire()?;
    do_sync().await
}

/// Get current sync status.
pub fn get_status() -> SyncStatus {
    let is_syncing = *IS_SYNCING.lock().unwrap();
    SyncStatus {
        is_syncing,
        last_sync_time: get_last_sync_time(),
        pending_changes: count_pending_changes(),
    }
}

/// Count how many records have changed since last sync.
pub fn count_pending_changes() -> i32 {
    let last_sync = match get_last_sync_time() {
        Some(t) => t,
        None => return -1, // first sync, unknown
    };

    let mut total = 0i32;
    for &(table, _id_field) in ENTITIES {
        let sql = format!(
            "SELECT COUNT(*) as cnt FROM {} WHERE update_time > ?",
            table
        );
        let count: Option<i32> =
            db::with_db(|conn| conn.query_row(&sql, [&last_sync], |row| row.get(0)).ok());
        total += count.unwrap_or(0);
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_database_with_old_cursor_forces_one_full_sync() {
        let plan = build_sync_cursor_plan(Some("2026-07-01T10:00:00".to_string()), 0, false);

        assert_eq!(plan.cursor, None);
        assert!(plan.mark_empty_full_sync);
        assert!(plan.is_empty_db_recovery);
    }

    #[test]
    fn empty_database_recovery_marker_prevents_repeated_full_sync() {
        let cursor = "2026-07-01T10:00:00".to_string();
        let plan = build_sync_cursor_plan(Some(cursor.clone()), 0, true);

        assert_eq!(plan.cursor, Some(cursor));
        assert!(!plan.mark_empty_full_sync);
        assert!(!plan.is_empty_db_recovery);
    }

    #[test]
    fn database_with_local_data_keeps_incremental_cursor() {
        let cursor = "2026-07-01T10:00:00".to_string();
        let plan = build_sync_cursor_plan(Some(cursor.clone()), 3, false);

        assert_eq!(plan.cursor, Some(cursor));
        assert!(!plan.mark_empty_full_sync);
        assert!(!plan.is_empty_db_recovery);
    }

    #[test]
    fn first_sync_of_empty_database_records_full_sync_marker() {
        let plan = build_sync_cursor_plan(None, 0, false);

        assert_eq!(plan.cursor, None);
        assert!(plan.mark_empty_full_sync);
        assert!(!plan.is_empty_db_recovery);
    }
}
