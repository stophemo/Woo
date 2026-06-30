use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

// ============================================================
// Connection Manager (matches electron/db/index.cjs)
//
// Filename strategy:
// - Not logged in → userData/woo.db (local only)
// - Logged in     → userData/woo-{username}.db (per-user)
// ============================================================

pub struct Database {
    pub conn: Connection,
    pub path: PathBuf,
}

static DB: Mutex<Option<Database>> = Mutex::new(None);
static CURRENT_USER: Mutex<Option<String>> = Mutex::new(None);
static DATA_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Set the app data directory (called once at startup)
pub fn set_data_dir(path: PathBuf) {
    if let Ok(mut dir) = DATA_DIR.lock() {
        *dir = Some(path);
    }
}

/// Switch to a specific user's database.
/// `None` = local mode (woo.db).
pub fn set_current_user(username: Option<&str>) {
    let mut user = CURRENT_USER.lock().unwrap();
    let new_val = username.map(String::from);
    if *user == new_val {
        return;
    }
    // Close existing connection
    if let Ok(mut db) = DB.lock() {
        db.take();
    }
    *user = new_val;
    log::info!("[DB] Switched to user database: {:?}", username);
}

/// On first login, copy woo.db → woo-{username}.db (matches syncEngine.cjs first-login logic)
pub fn copy_local_to_user_db_on_first_login(username: &str) -> Result<(), String> {
    let dir = DATA_DIR.lock().unwrap().clone().unwrap_or_else(|| PathBuf::from("."));
    let local_db = dir.join("woo.db");
    let safe = username.to_lowercase().replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "_");
    let user_db = dir.join(format!("woo-{}.db", safe));

    if local_db.exists() && !user_db.exists() {
        log::info!("[DB] First login — copying woo.db to woo-{}.db", safe);
        std::fs::copy(&local_db, &user_db).map_err(|e| format!("复制数据库失败: {}", e))?;
        // Also copy WAL/SHM files if they exist
        for ext in &["-wal", "-shm"] {
            let src = dir.join(format!("woo.db{}", ext));
            let dst = dir.join(format!("woo-{}.db{}", safe, ext));
            if src.exists() {
                std::fs::copy(&src, &dst).ok();
            }
        }
        // 清除 last_sync_time，让首次同步不传 update_time 过滤条件，拉取全部远端数据
        if let Ok(conn) = rusqlite::Connection::open(&user_db) {
            conn.execute(
                "DELETE FROM sync_meta WHERE key = 'last_sync_time'",
                [],
            )
            .ok();
        }
    }
    Ok(())
}

/// Get the database filename for the current user
fn get_db_path() -> PathBuf {
    let dir = DATA_DIR.lock().unwrap().clone().unwrap_or_else(|| PathBuf::from("."));
    let user = CURRENT_USER.lock().unwrap();
    match user.as_ref() {
        Some(name) => {
            let safe = name.to_lowercase().replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "_");
            dir.join(format!("woo-{}.db", safe))
        }
        None => dir.join("woo.db"),
    }
}

/// Execute a closure with the current database connection.
/// Auto-opens the database on first call.
pub fn with_db<F, T>(f: F) -> T
where
    F: FnOnce(&Connection) -> T,
{
    let mut guard = DB.lock().unwrap();
    let target = get_db_path();

    // Open if not yet open or path changed
    if guard.is_none() || guard.as_ref().map(|d| &d.path) != Some(&target) {
        *guard = None; // drop old
        log::info!("[DB] Opening database: {:?}", target);
        let conn = Connection::open(&target).expect("Failed to open database");
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn.pragma_update(None, "foreign_keys", "ON").ok();
        init_schema(&conn).expect("Failed to initialize schema");
        *guard = Some(Database { conn, path: target });
    }

    let db = guard.as_ref().expect("DB not initialized");
    f(&db.conn)
}

/// Close the current database connection
pub fn close_db() {
    if let Ok(mut db) = DB.lock() {
        db.take();
    }
}

/// Initialize the database schema (matches electron/db/schema.cjs)
fn init_schema(db: &Connection) -> Result<(), rusqlite::Error> {
    db.execute_batch(SCHEMA_SQL)?;
    Ok(())
}

const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS note_folder (
    id TEXT PRIMARY KEY,
    parent_id TEXT,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    create_time TEXT NOT NULL,
    update_time TEXT NOT NULL,
    deleted INTEGER NOT NULL DEFAULT 0,
    is_locked INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_folder_parent ON note_folder(parent_id);

CREATE TABLE IF NOT EXISTS note_document (
    id TEXT PRIMARY KEY,
    folder_id TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT,
    branch_name TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    create_time TEXT NOT NULL,
    update_time TEXT NOT NULL,
    deleted INTEGER NOT NULL DEFAULT 0,
    is_locked INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_doc_folder ON note_document(folder_id);

CREATE TABLE IF NOT EXISTS note_document_version (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL,
    version_no INTEGER NOT NULL,
    title TEXT,
    content TEXT,
    content_hash TEXT,
    change_type TEXT NOT NULL DEFAULT 'auto',
    operator_id TEXT,
    create_time TEXT NOT NULL,
    update_time TEXT NOT NULL,
    deleted INTEGER NOT NULL DEFAULT 0,
    UNIQUE(document_id, version_no)
);
CREATE INDEX IF NOT EXISTS idx_ver_doc ON note_document_version(document_id);

CREATE TABLE IF NOT EXISTS sync_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS kb_chunks (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL,
    document_title TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    content TEXT NOT NULL,
    create_time TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_kb_doc ON kb_chunks(document_id);
";

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_init_schema() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(tables.contains(&"note_folder".to_string()));
        assert!(tables.contains(&"note_document".to_string()));
        assert!(tables.contains(&"note_document_version".to_string()));
        assert!(tables.contains(&"sync_meta".to_string()));
    }
}
