use crate::db;
use crate::services::utils::now_str;
use chrono::{Duration, Utc};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

const ENTITIES: &[(&str, &str)] = &[
    ("note_folder", "id"),
    ("note_document", "id"),
    ("note_document_version", "id"),
];

// ===================== Sync State =====================

static IS_SYNCING: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));

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

// ===================== Sync Meta Helpers =====================

#[allow(dead_code)]
fn ensure_sync_meta_table() {
    db::with_db(|conn| {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sync_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
        )
        .ok();
    });
}

pub fn get_sync_meta(key: &str) -> Option<String> {
    db::with_db(|conn| {
        let mut stmt = conn
            .prepare("SELECT value FROM sync_meta WHERE key = ?")
            .ok()?;
        stmt.query_row([key], |row| row.get(0)).ok()
    })
}

pub fn set_sync_meta(key: &str, value: &str) {
    db::with_db(|conn| {
        conn.execute(
            "INSERT OR REPLACE INTO sync_meta (key, value) VALUES (?, ?)",
            [key, value],
        )
        .ok();
    });
}

pub fn get_last_sync_time() -> Option<String> {
    get_sync_meta("last_sync_time")
}

pub fn set_last_sync_time(time: &str) {
    set_sync_meta("last_sync_time", time);
}

// ===================== Supabase Stubs =====================

/// Stub: upsert rows to a Supabase table via REST API.
async fn supabase_upsert(table: &str, rows: &[serde_json::Value], _user_id: &str) -> Result<(), String> {
    log::debug!(
        "[Sync] Would upsert {} rows to {} (first id: {})",
        rows.len(),
        table,
        rows.first()
            .and_then(|r| r.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("?"),
    );
    Ok(())
}

/// Stub: select rows from a Supabase table, filtering by user_id and update_time > last_sync.
async fn supabase_select(
    table: &str,
    _user_id: &str,
    last_sync: Option<&str>,
) -> Result<Vec<serde_json::Value>, String> {
    log::debug!(
        "[Sync] Would select from {} where update_time > {:?}",
        table, last_sync
    );
    Ok(Vec::new())
}

/// Stub: delete rows from a Supabase table.
async fn supabase_delete(table: &str, _id_field: &str, ids: &[&str], _user_id: &str) -> Result<(), String> {
    log::debug!(
        "[Sync] Would delete {} rows from {} (ids: {:?})",
        ids.len(),
        table,
        ids
    );
    Ok(())
}

/// Stub: insert a tombstone record.
async fn supabase_insert_tombstone(table_name: &str, record_id: &str, _user_id: &str) -> Result<(), String> {
    log::debug!(
        "[Sync] Would insert tombstone for {}/{}",
        table_name, record_id
    );
    Ok(())
}

/// Stub: query tombstones from Supabase.
async fn supabase_select_tombstones(
    _user_id: &str,
    _last_pull: &str,
) -> Result<Vec<serde_json::Value>, String> {
    log::debug!("[Sync] Would pull tombstones since {}", _last_pull);
    Ok(Vec::new())
}

/// Stub: delete old tombstones from Supabase.
async fn supabase_cleanup_tombstones(_user_id: &str, _cutoff: &str) -> Result<(), String> {
    log::debug!("[Sync] Would cleanup tombstones before {}", _cutoff);
    Ok(())
}

// ===================== Push Logic =====================

fn query_local_changes(table: &str, _id_field: &str, last_sync: Option<&str>) -> Vec<serde_json::Value> {
    let last_sync = last_sync.unwrap_or("1970-01-01T00:00:00");
    db::with_db(|conn| {
        let sql = format!(
            "SELECT * FROM {} WHERE update_time > ? ORDER BY update_time ASC",
            table
        );
        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let col_count = stmt.column_count();
        let col_names: Vec<String> = (0..col_count)
            .filter_map(|i| stmt.column_name(i).ok().map(String::from))
            .collect();

        let rows = match stmt.query_map([last_sync], |row| {
            let mut map = serde_json::Map::new();
            for (i, name) in col_names.iter().enumerate() {
                let val: rusqlite::Result<String> = row.get(i);
                if let Ok(v) = val {
                    map.insert(name.clone(), serde_json::Value::String(v));
                }
            }
            Ok(serde_json::Value::Object(map))
        }) {
            Ok(r) => r.filter_map(|r| r.ok()).collect(),
            Err(_) => return Vec::new(),
        };
        rows
    })
}

/// Push local changes to Supabase.
async fn push_changes(
    user_id: &str,
    last_sync: Option<&str>,
    skip_ids: Option<&HashMap<String, HashSet<String>>>,
) -> (i32, Vec<String>) {
    let mut pushed_count = 0i32;
    let mut errors = Vec::new();

    for &(table, id_field) in ENTITIES {
        let mut local_rows = query_local_changes(table, id_field, last_sync);
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

        match supabase_upsert(table, &rows_with_user, user_id).await {
            Ok(_) => pushed_count += rows_with_user.len() as i32,
            Err(e) => errors.push(format!("[{}] upsert failed: {}", table, e)),
        }
    }

    (pushed_count, errors)
}

// ===================== Pull Logic =====================

/// Get the set of column names that exist in the local table.
fn get_local_columns(table: &str) -> HashSet<String> {
    db::with_db(|conn| {
        let sql = format!("PRAGMA table_info({})", table);
        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return HashSet::new(),
        };
        stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            Ok(name)
        })
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    })
}

/// Merge remote rows into local SQLite (last-write-wins).
/// Returns (pulled_count, conflict_count, pulled_ids).
fn merge_into_local(
    table: &str,
    id_field: &str,
    remote_rows: &[serde_json::Value],
) -> (i32, i32, Vec<String>) {
    if remote_rows.is_empty() {
        return (0, 0, Vec::new());
    }

    let local_cols = get_local_columns(table);

    db::with_db(|conn| {
        let mut pulled = 0i32;
        let mut conflicts = 0i32;
        let mut pulled_ids = Vec::new();

        // Prepare check statement
        let check_sql = format!("SELECT update_time FROM {} WHERE {} = ?", table, id_field);
        let mut check_stmt = match conn.prepare(&check_sql) {
            Ok(s) => s,
            Err(_) => return (0, 0, Vec::new()),
        };

        for row in remote_rows {
            let obj = match row.as_object() {
                Some(o) => o,
                None => continue,
            };

            let id = match obj.get(id_field).and_then(|v| v.as_str()) {
                Some(id) => id.to_string(),
                None => continue,
            };

            let remote_update_time = obj
                .get("update_time")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // Check local record
            let local_wins = check_stmt
                .query_row([&id], |r| {
                    let local_time: String = r.get(0)?;
                    Ok(local_time)
                })
                .ok()
                .map_or(false, |local_time| local_time.as_str() >= remote_update_time);

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
                continue;
            }

            let placeholders: Vec<String> = keys.iter().map(|_| "?".to_string()).collect();
            let insert_sql = format!(
                "INSERT OR REPLACE INTO {} ({}) VALUES ({})",
                table,
                keys.join(", "),
                placeholders.join(", ")
            );

            if let Ok(mut stmt) = conn.prepare(&insert_sql) {
                if stmt.execute(rusqlite::params_from_iter(vals.iter())).is_ok() {
                    pulled += 1;
                    pulled_ids.push(id);
                }
            }
        }

        (pulled, conflicts, pulled_ids)
    })
}

/// Pull remote changes from Supabase and merge into local.
async fn pull_changes(
    user_id: &str,
    last_sync: Option<&str>,
) -> (i32, i32, Vec<String>, HashMap<String, HashSet<String>>) {
    let mut pulled_count = 0i32;
    let mut conflict_count = 0i32;
    let mut errors = Vec::new();
    let mut pulled_ids_map: HashMap<String, HashSet<String>> = HashMap::new();

    for &(table, id_field) in ENTITIES {
        let remote_rows = match supabase_select(table, user_id, last_sync).await {
            Ok(rows) => rows,
            Err(e) => {
                errors.push(format!("[{}] pull failed: {}", table, e));
                continue;
            }
        };

        if remote_rows.is_empty() {
            continue;
        }

        let (pulled, conflicts, pulled_ids) = merge_into_local(table, id_field, &remote_rows);
        pulled_count += pulled;
        conflict_count += conflicts;

        if !pulled_ids.is_empty() {
            let set: HashSet<String> = pulled_ids.into_iter().collect();
            pulled_ids_map.insert(table.to_string(), set);
        }
    }

    (pulled_count, conflict_count, errors, pulled_ids_map)
}

// ===================== Tombstone Logic =====================

async fn pull_tombstones(user_id: &str) -> (i32, Vec<String>) {
    let last_pull = get_sync_meta("last_tombstone_pull").unwrap_or_else(|| "1970-01-01T00:00:00".to_string());

    let tombstones = match supabase_select_tombstones(user_id, &last_pull).await {
        Ok(t) => t,
        Err(e) => return (0, vec![e]),
    };

    if tombstones.is_empty() {
        return (0, Vec::new());
    }

    let mut count = 0i32;

    db::with_db(|conn| {
        for t in &tombstones {
            let table_name = t.get("table_name").and_then(|v| v.as_str());
            let record_id = t.get("record_id").and_then(|v| v.as_str());
            let _deleted_at = t.get("deleted_at").and_then(|v| v.as_str());

            let (tn, rid) = match (table_name, record_id) {
                (Some(tn), Some(rid)) => (tn, rid),
                _ => continue,
            };

            // Cascade delete versions for documents
            if tn == "note_document" {
                conn.execute("DELETE FROM note_document_version WHERE document_id = ?", [rid])
                    .ok();
            }

            let sql = format!("DELETE FROM {} WHERE id = ?", tn);
            if conn.execute(&sql, [rid]).is_ok() {
                count += 1;
            }
        }
    });

    // Update last tombstone pull time
    if let Some(latest) = tombstones
        .iter()
        .filter_map(|t| t.get("deleted_at").and_then(|v| v.as_str()))
        .max()
    {
        set_sync_meta("last_tombstone_pull", latest);
    }

    // Cleanup old tombstones (30 day cutoff)
    let cutoff = (Utc::now() - Duration::days(30))
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();

    supabase_cleanup_tombstones(user_id, &cutoff).await.ok();

    (count, Vec::new())
}

// ===================== Cleanup Logic =====================

fn parse_cleanup_seconds() -> i64 {
    match std::env::var("SYNC_CLEANUP_SECONDS") {
        Ok(expr) => {
            if expr.chars().all(|c| c.is_ascii_digit() || c == '*' || c == ' ') {
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
async fn cleanup_expired_deletes(user_id: &str) -> (i32, Vec<String>) {
    let cleanup_seconds = parse_cleanup_seconds();
    let cutoff = (Utc::now() - Duration::seconds(cleanup_seconds))
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();

    let mut cleanup_count = 0i32;
    let errors = Vec::<String>::new();

    // 1. Cleanup expired documents (deleted=2 and update_time < cutoff)
    let expired_docs: Vec<String> = db::with_db(|conn| {
        let mut stmt = conn
            .prepare("SELECT id FROM note_document WHERE deleted = 2 AND update_time < ?")
            .unwrap();
        stmt.query_map([&cutoff], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    });

    for doc_id in &expired_docs {
        // Delete versions from Supabase
        supabase_delete("note_document_version", "document_id", &[doc_id], user_id)
            .await
            .ok();
        // Delete document from Supabase
        supabase_delete("note_document", "id", &[doc_id], user_id)
            .await
            .ok();
        // Insert tombstone
        supabase_insert_tombstone("note_document", doc_id, user_id)
            .await
            .ok();

        // Delete locally
        db::with_db(|conn| {
            conn.execute("DELETE FROM note_document_version WHERE document_id = ?", [doc_id])
                .ok();
            conn.execute("DELETE FROM note_document WHERE id = ?", [doc_id])
                .ok();
        });
        cleanup_count += 1;
    }

    // 2. Promote empty folders (placeholder - calls stub)
    if !expired_docs.is_empty() {
        promote_empty_folders_to_deleted2();
    }

    // 3. Cleanup expired folders
    let expired_folders: Vec<String> = db::with_db(|conn| {
        let mut stmt = conn
            .prepare("SELECT id FROM note_folder WHERE deleted = 2 AND update_time < ?")
            .unwrap();
        stmt.query_map([&cutoff], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    });

    for folder_id in &expired_folders {
        supabase_delete("note_folder", "id", &[folder_id], user_id)
            .await
            .ok();
        supabase_insert_tombstone("note_folder", folder_id, user_id)
            .await
            .ok();
        db::with_db(|conn| {
            conn.execute("DELETE FROM note_folder WHERE id = ?", [folder_id])
                .ok();
        });
        cleanup_count += 1;
    }

    // 4. Cleanup expired versions
    let expired_versions: Vec<String> = db::with_db(|conn| {
        let mut stmt = conn
            .prepare("SELECT id FROM note_document_version WHERE deleted = 2 AND update_time < ?")
            .unwrap();
        stmt.query_map([&cutoff], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    });

    for ver_id in &expired_versions {
        supabase_delete("note_document_version", "id", &[ver_id], user_id)
            .await
            .ok();
        db::with_db(|conn| {
            conn.execute("DELETE FROM note_document_version WHERE id = ?", [ver_id])
                .ok();
        });
        cleanup_count += 1;
    }

    (cleanup_count, errors)
}

/// Placeholder: promote empty folder chains to deleted=2.
fn promote_empty_folders_to_deleted2() {
    // TODO: implement when document_service is ported
    log::debug!("[Sync] Would promote empty folders to deleted=2");
}

// ===================== Orchestration =====================

/// Execute a full sync cycle: tombstone pull → remote pull → local push → cleanup.
async fn do_sync() -> Result<SyncResult, String> {
    let user_id = "stub-user-id"; // TODO: wire up actual auth session

    let last_sync = get_last_sync_time();

    // 1. Pull tombstones
    let (tombstone_count, _tombstone_errors) = pull_tombstones(user_id).await;

    // 2. Pull remote changes
    let (pulled_count, conflict_count, _pull_errors, pulled_ids) =
        pull_changes(user_id, last_sync.as_deref()).await;

    // 3. Push local changes
    let (pushed_count, _push_errors) =
        push_changes(user_id, last_sync.as_deref(), Some(&pulled_ids)).await;

    // 4. Cleanup expired deletes
    let (cleanup_count, _cleanup_errors) = cleanup_expired_deletes(user_id).await;

    // 5. Update sync time
    let sync_time = now_str();
    set_last_sync_time(&sync_time);

    log::info!(
        "[Sync] ⇄ push={} pull={} conflict={} cleanup={} tombstone={}",
        pushed_count, pulled_count, conflict_count, cleanup_count, tombstone_count
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
    {
        let mut is_syncing = IS_SYNCING.lock().unwrap();
        if *is_syncing {
            return Err("同步正在进行中".to_string());
        }
        *is_syncing = true;
    }

    let result = do_sync().await;

    {
        let mut is_syncing = IS_SYNCING.lock().unwrap();
        *is_syncing = false;
    }

    result
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
        let count: Option<i32> = db::with_db(|conn| {
            conn.query_row(&sql, [&last_sync], |row| row.get(0)).ok()
        });
        total += count.unwrap_or(0);
    }
    total
}
