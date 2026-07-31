use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

// ============================================================
// Connection Manager (matches electron/db/index.cjs)
//
// Filename strategy:
// - Not logged in → userData/woo.db (local only)
// - Logged in     → userData/woo-{stable user id}.db (per-user)
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

/// Switch to a specific user's database using an immutable account key.
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

fn safe_user_key(value: &str) -> String {
    value
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "_")
}

fn user_db_path(dir: &std::path::Path, user_key: &str) -> PathBuf {
    dir.join(format!("woo-{}.db", safe_user_key(user_key)))
}

fn find_legacy_user_db(
    dir: &std::path::Path,
    user_id: &str,
    legacy_username: Option<&str>,
) -> Option<PathBuf> {
    let direct = legacy_username
        .filter(|username| !username.is_empty() && *username != user_id)
        .map(|username| user_db_path(dir, username))
        .filter(|path| path.exists());
    if direct.is_some() {
        return direct;
    }

    // If stale session metadata contains an opaque id instead of the old username,
    // a single non-UUID user database is still an unambiguous migration source.
    let candidates: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("db"))
        .filter(|path| {
            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                return false;
            };
            let Some(key) = stem.strip_prefix("woo-") else {
                return false;
            };
            uuid::Uuid::parse_str(key).is_err()
        })
        .collect();
    (candidates.len() == 1).then(|| candidates[0].clone())
}

/// Prepare the stable user-id database. Existing username-keyed databases are copied once
/// during upgrade, preserving local changes while preventing mutable profile metadata from
/// switching the active database later.
pub fn prepare_user_db_on_first_login(
    user_id: &str,
    legacy_username: Option<&str>,
) -> Result<(), String> {
    let dir = DATA_DIR
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| PathBuf::from("."));
    let user_db = user_db_path(&dir, user_id);
    if user_db.exists() {
        return Ok(());
    }

    // Closing the old connection checkpoints its WAL before the database file is copied.
    close_db();
    let local_db = dir.join("woo.db");
    let legacy_db = find_legacy_user_db(&dir, user_id, legacy_username);
    let source_db = legacy_db.as_ref().unwrap_or(&local_db);

    if source_db.exists() {
        log::info!(
            "[DB] Preparing stable user database {:?} from {:?}",
            user_db,
            source_db
        );
        std::fs::copy(source_db, &user_db).map_err(|e| format!("复制数据库失败: {}", e))?;
        // 清除 last_sync_time，让首次同步不传 update_time 过滤条件，拉取全部远端数据
        if let Ok(conn) = rusqlite::Connection::open(&user_db) {
            conn.execute("DELETE FROM sync_meta WHERE key = 'last_sync_time'", [])
                .ok();
        }
    }
    Ok(())
}

/// Get the database filename for the current user
fn get_db_path() -> PathBuf {
    let dir = DATA_DIR
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| PathBuf::from("."));
    let user = CURRENT_USER.lock().unwrap();
    match user.as_ref() {
        Some(name) => user_db_path(&dir, name),
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
mod user_db_tests {
    use super::*;

    #[test]
    fn finds_direct_legacy_username_database() {
        let dir = std::env::temp_dir().join(format!("woo-db-migration-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let legacy = user_db_path(&dir, "huojie");
        std::fs::write(&legacy, b"").unwrap();

        assert_eq!(
            find_legacy_user_db(&dir, "c6288975-7818-4c70-b649-d237a779643a", Some("huojie")),
            Some(legacy)
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn falls_back_only_when_one_named_legacy_database_exists() {
        let dir = std::env::temp_dir().join(format!("woo-db-migration-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let legacy = user_db_path(&dir, "huojie");
        std::fs::write(&legacy, b"").unwrap();

        assert_eq!(
            find_legacy_user_db(
                &dir,
                "c6288975-7818-4c70-b649-d237a779643a",
                Some("f38a0bbc-247f-4f50-bbd3-c0d60d2d6a1b")
            ),
            Some(legacy)
        );

        std::fs::write(user_db_path(&dir, "another-user"), b"").unwrap();
        assert_eq!(
            find_legacy_user_db(
                &dir,
                "c6288975-7818-4c70-b649-d237a779643a",
                Some("f38a0bbc-247f-4f50-bbd3-c0d60d2d6a1b")
            ),
            None
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

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
