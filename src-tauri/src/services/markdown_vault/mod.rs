//! 原生 Markdown 文件库：块切分、索引、读写与（桌面端）迁移。
//!
//! 本地 `.md` 是事实源，SQLite 只保存块与索引；本模块不改变现有 WebDAV 同步。

pub mod blocks;

#[cfg(desktop)]
pub mod migrate;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

pub use blocks::{block_hashes, content_hash, diff_block_hashes, join_blocks, split_blocks, Block};

const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS mv_file (
    path         TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    size_bytes   INTEGER NOT NULL,
    block_count  INTEGER NOT NULL,
    deleted      INTEGER NOT NULL DEFAULT 0,
    updated_at   TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS mv_block (
    hash        TEXT PRIMARY KEY,
    content     TEXT NOT NULL,
    plain_bytes INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS mv_file_block (
    path       TEXT NOT NULL,
    position   INTEGER NOT NULL,
    block_hash TEXT NOT NULL,
    PRIMARY KEY (path, position)
);
CREATE TABLE IF NOT EXISTS mv_op (
    op_id      TEXT PRIMARY KEY,
    kind       TEXT NOT NULL,
    path       TEXT NOT NULL,
    payload    TEXT NOT NULL,
    created_at TEXT NOT NULL,
    uploaded   INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_mv_file_block_hash ON mv_file_block(block_hash);
";

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub root: String,
    pub files: usize,
    pub blocks: usize,
    pub changed: usize,
    pub removed: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    pub path: String,
    pub size_bytes: i64,
    pub content_hash: String,
    pub block_count: usize,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedNote {
    pub note: NoteSummary,
    pub old_hash: Option<String>,
    pub new_hash: String,
    pub block_count: usize,
    pub changes: usize,
}

fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(SCHEMA_SQL)
        .map_err(|error| format!("初始化 Markdown 索引表失败：{error}"))
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "md" | "markdown" | "mdown" | "mkd" | "txt"
            )
        })
        .unwrap_or(false)
}

fn collect_markdown_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|error| format!("读取目录失败 {}：{error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("读取目录项失败：{error}"))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            if name == "node_modules" || name == "attachments" {
                continue;
            }
            collect_markdown_files(root, &path, out)?;
        } else if path.is_file() && is_markdown(&path) {
            out.push(path);
        }
    }
    Ok(())
}

fn relative_path(root: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(root)
        .map(|value| value.to_string_lossy().replace('\\', "/"))
        .map_err(|_| format!("文件不在 vault 目录内：{}", path.display()))
}

/// 把前端传入的相对路径限制在 vault 根目录内，并创建父目录。
pub fn safe_path(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let normalized = rel.replace('\\', "/");
    let normalized = normalized.trim_start_matches("./");
    if normalized.is_empty() || normalized.starts_with('/') || normalized.contains("..") {
        return Err("路径无效".to_string());
    }
    let root_canon = root
        .canonicalize()
        .map_err(|error| format!("vault 目录不存在：{error}"))?;
    let path = root_canon.join(&normalized);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建目录失败：{error}"))?;
        let parent_canon = parent
            .canonicalize()
            .map_err(|error| format!("解析目录失败：{error}"))?;
        if !parent_canon.starts_with(&root_canon) {
            return Err("路径越出 vault 目录".to_string());
        }
    }
    Ok(path)
}

fn old_content_hash(conn: &Connection, rel: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT content_hash FROM mv_file WHERE path = ?1",
        params![rel],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|error| format!("读取文件索引失败：{error}"))
}

fn read_block_hashes(conn: &Connection, rel: &str) -> Result<Vec<String>, String> {
    let mut statement = conn
        .prepare("SELECT block_hash FROM mv_file_block WHERE path = ?1 ORDER BY position")
        .map_err(|error| format!("读取块索引失败：{error}"))?;
    let rows = statement
        .query_map(params![rel], |row| row.get::<_, String>(0))
        .map_err(|error| format!("读取块索引失败：{error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取块索引失败：{error}"))
}

fn write_file_index(
    conn: &Connection,
    rel: &str,
    content: &str,
) -> Result<(Option<String>, usize), String> {
    let old_hash = old_content_hash(conn, rel)?;
    let hash = content_hash(content);
    let blocks = split_blocks(content);
    let timestamp = now();

    conn.execute(
        "INSERT INTO mv_file(path, content_hash, size_bytes, block_count, deleted, updated_at)
         VALUES (?1, ?2, ?3, ?4, 0, ?5)
         ON CONFLICT(path) DO UPDATE SET
           content_hash = excluded.content_hash,
           size_bytes = excluded.size_bytes,
           block_count = excluded.block_count,
           deleted = 0,
           updated_at = excluded.updated_at",
        params![
            rel,
            hash,
            content.len() as i64,
            blocks.len() as i64,
            timestamp
        ],
    )
    .map_err(|error| format!("写入文件索引失败：{error}"))?;

    conn.execute("DELETE FROM mv_file_block WHERE path = ?1", params![rel])
        .map_err(|error| format!("清理旧块索引失败：{error}"))?;

    for (position, block) in blocks.iter().enumerate() {
        conn.execute(
            "INSERT INTO mv_block(hash, content, plain_bytes)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(hash) DO NOTHING",
            params![block.hash, block.text, block.text.len() as i64],
        )
        .map_err(|error| format!("写入块索引失败：{error}"))?;
        conn.execute(
            "INSERT INTO mv_file_block(path, position, block_hash) VALUES (?1, ?2, ?3)",
            params![rel, position as i64, block.hash],
        )
        .map_err(|error| format!("写入文件块索引失败：{error}"))?;
    }

    Ok((old_hash, blocks.len()))
}

fn note_summary(conn: &Connection, rel: &str) -> Result<NoteSummary, String> {
    conn.query_row(
        "SELECT path, size_bytes, content_hash, block_count, updated_at
         FROM mv_file WHERE path = ?1",
        params![rel],
        |row| {
            Ok(NoteSummary {
                path: row.get(0)?,
                size_bytes: row.get(1)?,
                content_hash: row.get(2)?,
                block_count: row.get::<_, i64>(3)? as usize,
                updated_at: row.get(4)?,
            })
        },
    )
    .map_err(|error| format!("读取文件索引失败：{error}"))
}

pub fn scan_vault(conn: &Connection, root: &Path) -> Result<ScanSummary, String> {
    ensure_schema(conn)?;
    if !root.is_dir() {
        return Err("请选择一个存在的 Markdown 文件夹".to_string());
    }
    let mut files = Vec::new();
    collect_markdown_files(root, root, &mut files)?;

    let mut summary = ScanSummary {
        root: root.to_string_lossy().to_string(),
        ..Default::default()
    };
    let mut seen = HashSet::new();

    for path in files {
        let rel = relative_path(root, &path)?;
        seen.insert(rel.clone());
        let content = fs::read_to_string(&path)
            .map_err(|error| format!("读取文件失败 {}：{error}", path.display()))?;
        let hash = content_hash(&content);
        let previous = old_content_hash(conn, &rel)?;
        summary.files += 1;
        if previous.as_deref() == Some(hash.as_str()) {
            summary.skipped += 1;
            continue;
        }
        let (_, _block_count) = write_file_index(conn, &rel, &content)?;
        summary.changed += 1;
    }

    summary.blocks = conn
        .query_row("SELECT COUNT(*) FROM mv_file_block", [], |row| {
            row.get::<_, i64>(0)
        })
        .map(|value| value as usize)
        .unwrap_or(0);

    let indexed: Vec<String> = {
        let mut statement = conn
            .prepare("SELECT path FROM mv_file WHERE deleted = 0")
            .map_err(|error| format!("读取索引失败：{error}"))?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| format!("读取索引失败：{error}"))?;
        rows.filter_map(Result::ok).collect()
    };

    for rel in indexed {
        if !seen.contains(&rel) {
            conn.execute(
                "UPDATE mv_file SET deleted = 1, updated_at = ?1 WHERE path = ?2",
                params![now(), rel],
            )
            .map_err(|error| format!("标记文件删除失败：{error}"))?;
            summary.removed += 1;
        }
    }

    Ok(summary)
}

pub fn list_notes(conn: &Connection) -> Result<Vec<NoteSummary>, String> {
    ensure_schema(conn)?;
    let mut statement = conn
        .prepare(
            "SELECT path, size_bytes, content_hash, block_count, updated_at
             FROM mv_file WHERE deleted = 0 ORDER BY path",
        )
        .map_err(|error| format!("读取索引失败：{error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(NoteSummary {
                path: row.get(0)?,
                size_bytes: row.get(1)?,
                content_hash: row.get(2)?,
                block_count: row.get::<_, i64>(3)? as usize,
                updated_at: row.get(4)?,
            })
        })
        .map_err(|error| format!("读取索引失败：{error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取索引失败：{error}"))
}

pub fn read_note(root: &Path, rel: &str) -> Result<String, String> {
    let path = safe_path(root, rel)?;
    fs::read_to_string(&path).map_err(|error| format!("读取文件失败：{error}"))
}

pub fn save_note(
    conn: &Connection,
    root: &Path,
    rel: &str,
    content: &str,
) -> Result<SavedNote, String> {
    ensure_schema(conn)?;
    let path = safe_path(root, rel)?;
    fs::write(&path, content).map_err(|error| format!("保存文件失败：{error}"))?;

    let old_hashes = read_block_hashes(conn, rel)?;
    let (old_hash, block_count) = write_file_index(conn, rel, content)?;
    let new_hash = content_hash(content);
    let new_hashes = block_hashes(&split_blocks(content));
    let changes = diff_block_hashes(&old_hashes, &new_hashes).len();

    let payload = serde_json::json!({
        "path": rel,
        "oldHash": old_hash.clone(),
        "newHash": new_hash.clone(),
        "blockCount": block_count,
    })
    .to_string();
    conn.execute(
        "INSERT INTO mv_op(op_id, kind, path, payload, created_at, uploaded)
         VALUES (?1, 'replace_file', ?2, ?3, ?4, 0)",
        params![format!("mv-{}", uuid::Uuid::new_v4()), rel, payload, now()],
    )
    .map_err(|error| format!("写入本地操作队列失败：{error}"))?;

    Ok(SavedNote {
        note: note_summary(conn, rel)?,
        old_hash,
        new_hash,
        block_count,
        changes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_round_trip() {
        let source = "# 标题\n\n第一段。\n\n- a\n- b\n";
        assert_eq!(join_blocks(&split_blocks(source)), source);
    }

    #[test]
    fn rejects_path_escape() {
        let root = std::env::temp_dir();
        assert!(safe_path(&root, "../etc/passwd").is_err());
        assert!(safe_path(&root, "/etc/passwd").is_err());
    }
}
