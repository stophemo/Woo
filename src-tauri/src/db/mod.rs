use rusqlite::Connection;
use std::cell::Cell;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

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

/// 连接与「当前用户」必须由同一把锁保护。
///
/// 早期实现分别使用 `DB` 与 `CURRENT_USER` 两把锁，`with_db` 的加锁顺序是
/// `DB → CURRENT_USER`，而 `set_current_user` 是 `CURRENT_USER → DB`，
/// 两者在同步定时器与登录/登出并发时会形成 ABBA 死锁，表现为登录卡住、
/// 界面永久无响应（无法自愈，只能杀进程）。合并为单一状态后不再可能死锁。
struct DbState {
    db: Option<Database>,
    user: Option<String>,
}

static STATE: Mutex<DbState> = Mutex::new(DbState { db: None, user: None });
static DATA_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

thread_local! {
    /// 把 `with_db` 的意外嵌套从「永久阻塞」暴露成可定位的错误。
    static WITH_DB_DEPTH: Cell<u32> = Cell::new(0);
}

/// 任何持锁线程 panic 都不应让登录/同步永久失败：锁中毒时直接取回内部数据。
fn lock_state() -> MutexGuard<'static, DbState> {
    STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn lock_data_dir() -> MutexGuard<'static, Option<PathBuf>> {
    DATA_DIR.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 数据目录是叶子锁：只在不需要其它锁时短暂持有，返回后立即释放。
fn data_dir() -> PathBuf {
    lock_data_dir()
        .clone()
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Set the app data directory (called once at startup)
pub fn set_data_dir(path: PathBuf) {
    *lock_data_dir() = Some(path);
}

/// Switch to a specific user's database using an immutable account key.
/// `None` = local mode (woo.db).
pub fn set_current_user(username: Option<&str>) {
    let mut state = lock_state();
    let new_val = username.map(String::from);
    if state.user == new_val {
        return;
    }
    // Close existing connection before switching files.
    state.db = None;
    state.user = new_val;
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
    let dir = data_dir();
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

/// Execute a closure with the current database connection. Auto-opens the database
/// on first call and after a user switch.
///
/// 注意：闭包内**不得**再调用任何会走到 `with_db` 的函数（本函数持锁且不可重入）。
/// 需要连接时请把 `&Connection` 传给 `*_inner(conn, ...)` 形式的内部函数。
pub fn with_db<F, T>(f: F) -> T
where
    F: FnOnce(&Connection) -> T,
{
    struct DepthGuard;
    impl Drop for DepthGuard {
        fn drop(&mut self) {
            WITH_DB_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
        }
    }

    WITH_DB_DEPTH.with(|depth| {
        let current = depth.get();
        assert!(
            current == 0,
            "db::with_db 被重入调用，请改用接收 &Connection 的内部函数"
        );
        depth.set(current + 1);
    });
    let _depth_guard = DepthGuard;

    let mut state = lock_state();
    let dir = data_dir();
    let target = match state.user.as_ref() {
        Some(name) => user_db_path(&dir, name),
        None => dir.join("woo.db"),
    };

    // Open if not yet open or path changed
    if state.db.as_ref().map(|d| &d.path) != Some(&target) {
        state.db = None; // drop old connection first
        log::info!("[DB] Opening database: {:?}", target);
        let conn = Connection::open(&target)
            .unwrap_or_else(|e| panic!("无法打开数据库 {:?}: {}", target, e));
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn.pragma_update(None, "foreign_keys", "ON").ok();
        init_schema(&conn).unwrap_or_else(|e| panic!("初始化数据库结构失败: {}", e));
        state.db = Some(Database { conn, path: target });
    }

    let db = state.db.as_ref().expect("DB not initialized");
    f(&db.conn)
}

/// Close the current database connection
pub fn close_db() {
    lock_state().db = None;
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

#[cfg(test)]
mod lock_order_tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// 回归测试：`with_db` 与 `set_current_user` 并发时曾经会因为两把锁顺序相反
    /// （DB → CURRENT_USER 与 CURRENT_USER → DB）形成 ABBA 死锁，表现为登录卡死、
    /// 界面永久无响应。合并为单一状态后必须能在同一时间段内完成。
    #[test]
    fn switching_user_database_does_not_deadlock_with_queries() {
        let dir = std::env::temp_dir().join(format!("woo-lock-order-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        set_data_dir(dir.clone());
        set_current_user(None);

        let (sender, receiver) = mpsc::channel();

        let reader_sender = sender.clone();
        let reader = std::thread::spawn(move || {
            for _ in 0..200 {
                with_db(|conn| {
                    let _: i64 = conn
                        .query_row("SELECT COUNT(*) FROM note_document", [], |row| row.get(0))
                        .unwrap_or(0);
                });
            }
            reader_sender.send("reader").ok();
        });

        let switcher = std::thread::spawn(move || {
            for round in 0..200 {
                if round % 2 == 0 {
                    set_current_user(Some("lock-order-user"));
                } else {
                    set_current_user(None);
                }
            }
            sender.send("switcher").ok();
        });

        let first = receiver.recv_timeout(Duration::from_secs(15));
        let second = receiver.recv_timeout(Duration::from_secs(15));

        assert!(
            first.is_ok() && second.is_ok(),
            "with_db 与 set_current_user 并发时发生死锁（旧实现的双锁顺序问题）"
        );

        reader.join().ok();
        switcher.join().ok();

        set_current_user(None);
        std::fs::remove_dir_all(&dir).ok();
    }
}
