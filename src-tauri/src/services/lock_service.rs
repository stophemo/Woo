use crate::db;
use crate::services::utils::now_str;
use crate::supabase;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

const SALT_ROUNDS: u32 = 10;
const HASH_KEY: &str = "lock_password_hash";
const MODE_KEY: &str = "lock_password_mode";
const CLOUD_PENDING_KEY: &str = "lock_password_cloud_pending";

#[allow(dead_code)]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LockStatus {
    pub has_password: bool,
    pub mode: String,
}

pub fn has_password() -> bool {
    db::with_db(|conn| {
        conn.query_row(
            "SELECT value FROM sync_meta WHERE key = ?",
            params![HASH_KEY],
            |row| row.get::<_, String>(0),
        )
        .is_ok()
    })
}

pub fn get_password_mode() -> String {
    db::with_db(|conn| {
        conn.query_row(
            "SELECT value FROM sync_meta WHERE key = ?",
            params![MODE_KEY],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_default()
    })
}

pub fn set_password(password: &str) -> Result<(), String> {
    if password.len() < 4 {
        return Err("密码长度不能少于 4 位".to_string());
    }
    let hash = bcrypt::hash(password, SALT_ROUNDS).map_err(|e| e.to_string())?;
    db::with_db(|conn| {
        conn.execute(
            "INSERT OR REPLACE INTO sync_meta (key, value) VALUES (?, ?)",
            params![HASH_KEY, hash],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO sync_meta (key, value) VALUES (?, ?)",
            params![MODE_KEY, "custom"],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO sync_meta (key, value) VALUES (?, '1')",
            params![CLOUD_PENDING_KEY],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

pub fn verify_password(password: &str) -> Result<bool, String> {
    if password.is_empty() {
        return Ok(false);
    }
    db::with_db(|conn| {
        let hash: Option<String> = conn
            .query_row(
                "SELECT value FROM sync_meta WHERE key = ?",
                params![HASH_KEY],
                |row| row.get(0),
            )
            .ok();
        match hash {
            Some(h) => bcrypt::verify(password, &h).map_err(|e| e.to_string()),
            None => Ok(false),
        }
    })
}

pub fn lock_folder(folder_id: &str) -> Result<(), String> {
    db::with_db(|conn| {
        let exists: bool = conn
            .query_row(
                "SELECT id FROM note_folder WHERE id = ? AND deleted = 0",
                params![folder_id],
                |_| Ok(()),
            )
            .is_ok();
        if !exists {
            return Err("目录不存在".to_string());
        }
        let now = now_str();
        conn.execute(
            "UPDATE note_folder SET is_locked = 1, update_time = ? WHERE id = ?",
            params![now, folder_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

pub fn unlock_folder(folder_id: &str) -> Result<(), String> {
    db::with_db(|conn| {
        let now = now_str();
        conn.execute(
            "UPDATE note_folder SET is_locked = 0, update_time = ? WHERE id = ?",
            params![now, folder_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

pub fn is_folder_locked(folder_id: &str) -> bool {
    db::with_db(|conn| {
        conn.query_row(
            "SELECT is_locked FROM note_folder WHERE id = ? AND deleted = 0",
            params![folder_id],
            |row| row.get::<_, i32>(0),
        )
        .map(|v| v == 1)
        .unwrap_or(false)
    })
}

pub fn is_folder_effectively_locked(folder_id: &str) -> bool {
    db::with_db(|conn| is_folder_or_ancestor_locked(conn, folder_id))
}

pub fn lock_document(document_id: &str) -> Result<(), String> {
    db::with_db(|conn| {
        let exists: bool = conn
            .query_row(
                "SELECT id FROM note_document WHERE id = ? AND deleted = 0",
                params![document_id],
                |_| Ok(()),
            )
            .is_ok();
        if !exists {
            return Err("文稿不存在".to_string());
        }
        let now = now_str();
        conn.execute(
            "UPDATE note_document SET is_locked = 1, update_time = ? WHERE id = ?",
            params![now, document_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

pub fn unlock_document(document_id: &str) -> Result<(), String> {
    db::with_db(|conn| {
        let now = now_str();
        conn.execute(
            "UPDATE note_document SET is_locked = 0, update_time = ? WHERE id = ?",
            params![now, document_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

pub fn is_document_locked(document_id: &str) -> bool {
    db::with_db(|conn| {
        conn.query_row(
            "SELECT is_locked FROM note_document WHERE id = ? AND deleted = 0",
            params![document_id],
            |row| row.get::<_, i32>(0),
        )
        .map(|v| v == 1)
        .unwrap_or(false)
    })
}

/// Connection-aware check for whether a document is locked.
/// Callers that already hold the DB lock should use this to avoid reentrancy deadlocks.
pub fn is_document_effectively_locked(conn: &rusqlite::Connection, document_id: &str) -> bool {
    let (is_locked, folder_id): (i32, Option<String>) = match conn.query_row(
        "SELECT is_locked, folder_id FROM note_document WHERE id = ? AND deleted = 0",
        params![document_id],
        |row| Ok((row.get::<_, i32>(0)?, row.get::<_, Option<String>>(1)?)),
    ) {
        Ok(r) => r,
        Err(_) => return false,
    };
    if is_locked == 1 {
        return true;
    }
    match folder_id {
        Some(fid) => is_folder_or_ancestor_locked(conn, &fid),
        None => false,
    }
}

/// Connection-aware alias for `is_document_effectively_locked`.
pub fn is_document_hidden(conn: &rusqlite::Connection, document_id: &str) -> bool {
    is_document_effectively_locked(conn, document_id)
}

/// Check if a folder or any of its ancestors is locked.
/// Public so other modules (e.g. document_service) can use it.
pub fn is_folder_or_ancestor_locked(conn: &rusqlite::Connection, folder_id: &str) -> bool {
    let mut cur_id = Some(folder_id.to_string());
    let mut visited = std::collections::HashSet::new();
    while let Some(ref fid) = cur_id {
        if !visited.insert((*fid).clone()) {
            break;
        }
        match conn.query_row(
            "SELECT parent_id, is_locked FROM note_folder WHERE id = ? AND deleted = 0",
            params![fid],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i32>(1)?)),
        ) {
            Ok((parent_id, is_locked)) => {
                if is_locked == 1 {
                    return true;
                }
                cur_id = parent_id;
            }
            Err(_) => break,
        }
    }
    false
}

fn get_local_lock_settings() -> Result<Option<(String, String)>, String> {
    db::with_db(|conn| {
        let hash: Option<String> = conn
            .query_row(
                "SELECT value FROM sync_meta WHERE key = ?",
                params![HASH_KEY],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let Some(hash) = hash else {
            return Ok(None);
        };
        let mode = conn
            .query_row(
                "SELECT value FROM sync_meta WHERE key = ?",
                params![MODE_KEY],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "custom".to_string());
        Ok(Some((hash, mode)))
    })
}

fn save_local_lock_settings(hash: &str, mode: &str) -> Result<(), String> {
    // Reject malformed metadata before it can replace a working local password.
    bcrypt::verify("", hash).map_err(|_| "云端锁密码数据无效".to_string())?;
    db::with_db(|conn| {
        conn.execute(
            "INSERT OR REPLACE INTO sync_meta (key, value) VALUES (?, ?)",
            params![HASH_KEY, hash],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO sync_meta (key, value) VALUES (?, ?)",
            params![MODE_KEY, mode],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

fn current_access_token() -> Option<String> {
    supabase::CURRENT_SESSION
        .lock()
        .ok()
        .and_then(|session| session.as_ref().map(|value| value.access_token.clone()))
}

fn has_pending_cloud_settings() -> bool {
    db::with_db(|conn| {
        conn.query_row(
            "SELECT value FROM sync_meta WHERE key = ?",
            params![CLOUD_PENDING_KEY],
            |row| row.get::<_, String>(0),
        )
        .map(|value| value == "1")
        .unwrap_or(false)
    })
}

fn clear_pending_cloud_settings() -> Result<(), String> {
    db::with_db(|conn| {
        conn.execute(
            "DELETE FROM sync_meta WHERE key = ?",
            params![CLOUD_PENDING_KEY],
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    })
}

pub async fn cloud_push_settings() -> Result<(), String> {
    let Some(access_token) = current_access_token() else {
        return Ok(());
    };
    let Some((hash, mode)) = get_local_lock_settings()? else {
        return Ok(());
    };
    let user = supabase::update_lock_settings(&access_token, &hash, &mode).await?;
    supabase::cache_current_user(user);
    clear_pending_cloud_settings()?;
    Ok(())
}

pub async fn cloud_pull_settings() -> Result<(), String> {
    let Some(access_token) = current_access_token() else {
        return Ok(());
    };
    if has_pending_cloud_settings() {
        return cloud_push_settings().await;
    }
    let Some(user) = supabase::get_user(&access_token).await? else {
        return Err("登录会话已失效".to_string());
    };

    let remote_hash = user.user_metadata.woo_lock_password_hash.clone();
    let remote_mode = user
        .user_metadata
        .woo_lock_password_mode
        .clone()
        .unwrap_or_else(|| "custom".to_string());
    supabase::cache_current_user(user);

    if let Some(hash) = remote_hash {
        save_local_lock_settings(&hash, &remote_mode)?;
        return Ok(());
    }

    // 旧版本的各设备可能保存着不同密码。云端为空时保留本地值，只有用户成功
    // 验证或主动设置密码后才上传，避免启动顺序把错误的旧哈希传播到其他设备。
    Ok(())
}
