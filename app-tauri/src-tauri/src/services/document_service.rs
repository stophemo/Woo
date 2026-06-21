use crate::db;
use crate::services::utils::{new_id, now_str, strip_html_keep_lines};
use rusqlite::{params, Connection};
use serde::Serialize;

use super::lock_service;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentDTO {
    pub id: String,
    pub folder_id: String,
    pub title: String,
    pub content: Option<String>,
    pub branch_name: Option<String>,
    pub sort_order: i32,
    pub create_time: String,
    pub update_time: String,
    pub is_locked: bool,
    pub folder_name: Option<String>,
}

fn doc_from_row(row: &rusqlite::Row) -> rusqlite::Result<DocumentDTO> {
    Ok(DocumentDTO {
        id: row.get(0)?,
        folder_id: row.get(1)?,
        title: row.get(2)?,
        content: row.get(3)?,
        branch_name: row.get(4)?,
        sort_order: row.get(5)?,
        create_time: row.get(6)?,
        update_time: row.get(7)?,
        is_locked: row.get::<_, i32>(9)? != 0,
        folder_name: None,
    })
}

fn doc_from_row_joined(row: &rusqlite::Row) -> rusqlite::Result<DocumentDTO> {
    let mut dto = doc_from_row(row)?;
    dto.folder_name = row.get(10)?;
    Ok(dto)
}

fn extract_first_line_as_title(content: &str) -> String {
    if content.is_empty() {
        return "新文稿".to_string();
    }
    let plain = strip_html_keep_lines(content);
    for line in plain.lines() {
        let t = line.trim();
        if !t.is_empty() {
            if t.chars().count() > 40 {
                return format!("{}…", t.chars().take(40).collect::<String>());
            }
            return t.to_string();
        }
    }
    "新文稿".to_string()
}

fn sanitize(seg: &str) -> String {
    seg.chars()
        .map(|c| {
            if c.is_whitespace() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect()
}

fn restore_folder_chain(conn: &Connection, now: &str, folder_id: Option<&str>) -> Result<(), String> {
    let fid = match folder_id {
        Some(f) => f,
        None => return Ok(()),
    };
    let result = conn.query_row(
        "SELECT deleted, parent_id FROM note_folder WHERE id = ?",
        params![fid],
        |row| Ok((row.get::<_, i32>(0)?, row.get::<_, Option<String>>(1)?)),
    );
    let (deleted, parent_id) = match result {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };
    if deleted == 0 {
        return Ok(());
    }
    restore_folder_chain(conn, now, parent_id.as_deref())?;
    conn.execute(
        "UPDATE note_folder SET deleted = 0, update_time = ? WHERE id = ?",
        params![now, fid],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn promote_empty_folders_inner(conn: &Connection) -> i32 {
    let mut total = 0;
    loop {
        let mut stmt = conn
            .prepare(
                "SELECT f.id FROM note_folder f
             WHERE f.deleted = 1
               AND NOT EXISTS (
                 SELECT 1 FROM note_document d
                 WHERE d.folder_id = f.id AND d.deleted IN (0, 1)
               )
               AND NOT EXISTS (
                 SELECT 1 FROM note_folder c
                 WHERE c.parent_id = f.id AND c.deleted IN (0, 1)
               )",
            )
            .unwrap();
        let empty: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        if empty.is_empty() {
            break;
        }
        let now = now_str();
        for id in &empty {
            conn.execute(
                "UPDATE note_folder SET deleted = 2, update_time = ? WHERE id = ?",
                params![now, id],
            )
            .ok();
        }
        total += empty.len() as i32;
    }
    total
}

pub fn list_by_folder(folder_id: &str) -> Result<Vec<DocumentDTO>, String> {
    db::with_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT d.*, f.name AS folder_name FROM note_document d
             LEFT JOIN note_folder f ON d.folder_id = f.id
             WHERE d.folder_id = ? AND d.deleted = 0
             ORDER BY d.sort_order ASC, d.update_time DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![folder_id], doc_from_row_joined)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .map(|mut dto| {
                dto.is_locked = lock_service::is_document_effectively_locked(conn, &dto.id);
                dto
            })
            .collect();
        Ok(rows)
    })
}

pub fn list_all() -> Result<Vec<DocumentDTO>, String> {
    db::with_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT d.*, f.name AS folder_name FROM note_document d
             LEFT JOIN note_folder f ON d.folder_id = f.id
             WHERE d.deleted = 0
             ORDER BY d.update_time DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], doc_from_row_joined)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .filter(|dto| !lock_service::is_document_hidden(conn, &dto.id))
            .collect();
        Ok(rows)
    })
}

pub fn list_orphans() -> Result<Vec<DocumentDTO>, String> {
    db::with_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT d.* FROM note_document d
             WHERE d.deleted = 0 AND (
               d.folder_id IS NULL OR d.folder_id = '' OR
               NOT EXISTS (SELECT 1 FROM note_folder f WHERE f.id = d.folder_id AND f.deleted = 0)
             )
             ORDER BY d.update_time DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], doc_from_row)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .filter(|dto| !lock_service::is_document_hidden(conn, &dto.id))
            .collect();
        Ok(rows)
    })
}

pub fn list_trash() -> Result<Vec<DocumentDTO>, String> {
    db::with_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT * FROM note_document
             WHERE deleted = 1
             ORDER BY update_time DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], doc_from_row)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    })
}

pub fn search(keyword: &str) -> Result<Vec<DocumentDTO>, String> {
    let q = keyword.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let like = format!("%{}%", q);
    db::with_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT * FROM note_document
             WHERE deleted = 0 AND (title LIKE ? OR content LIKE ?)
             ORDER BY update_time DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![like, like], doc_from_row)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .filter(|dto| !lock_service::is_document_hidden(conn, &dto.id))
            .collect();
        Ok(rows)
    })
}

pub fn get_by_id(id: &str) -> Result<DocumentDTO, String> {
    db::with_db(|conn| {
        let mut dto = conn
            .query_row(
                "SELECT * FROM note_document WHERE id = ? AND deleted = 0",
                params![id],
                doc_from_row,
            )
            .map_err(|_| "文稿不存在".to_string())?;
        dto.is_locked = lock_service::is_document_effectively_locked(conn, &dto.id);
        Ok(dto)
    })
}

pub fn create(folder_id: &str, title: &str, content: Option<&str>) -> Result<DocumentDTO, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("标题不能为空".to_string());
    }
    if folder_id.is_empty() {
        return Err("目录不能为空".to_string());
    }

    db::with_db(|conn| {
        let folder = conn
            .query_row(
                "SELECT name, parent_id FROM note_folder WHERE id = ? AND deleted = 0",
                params![folder_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .map_err(|_| "目录不存在".to_string())?;
        let (folder_name, folder_parent_id) = folder;

        let mut parts: Vec<String> = Vec::new();
        parts.push(sanitize(&folder_name));
        let mut cur_parent = folder_parent_id;
        while let Some(pid) = cur_parent {
            let (name, parent) = conn
                .query_row(
                    "SELECT name, parent_id FROM note_folder WHERE id = ?",
                    params![pid],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
                )
                .map_err(|_| "目录不存在".to_string())?;
            parts.insert(0, sanitize(&name));
            cur_parent = parent;
        }
        parts.push(sanitize(title));
        let branch_name = parts.join("-");

        let doc_id = new_id();
        let now = now_str();
        let content_val = content.unwrap_or("");
        conn.execute(
            "INSERT INTO note_document (id, folder_id, title, content, branch_name, sort_order, create_time, update_time)
             VALUES (?, ?, ?, ?, ?, 0, ?, ?)",
            params![doc_id, folder_id, title, content_val, branch_name, now, now],
        )
        .map_err(|e| e.to_string())?;

        let doc = conn
            .query_row("SELECT * FROM note_document WHERE id = ?", params![doc_id], doc_from_row)
            .map_err(|e| e.to_string())?;
        Ok(doc)
    })
}

pub fn rename(id: &str, title: &str) -> Result<(), String> {
    db::with_db(|conn| {
        conn.query_row(
            "SELECT id FROM note_document WHERE id = ? AND deleted = 0",
            params![id],
            |_| Ok(()),
        )
        .map_err(|_| "文稿不存在".to_string())?;

        let now = now_str();
        conn.execute(
            "UPDATE note_document SET title = ?, update_time = ? WHERE id = ?",
            params![title, now, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

pub fn update_content(id: &str, content: &str) -> Result<(), String> {
    db::with_db(|conn| {
        conn.query_row(
            "SELECT id FROM note_document WHERE id = ? AND deleted = 0",
            params![id],
            |_| Ok(()),
        )
        .map_err(|_| "文稿不存在".to_string())?;

        let title = extract_first_line_as_title(content);
        let now = now_str();
        conn.execute(
            "UPDATE note_document SET content = ?, title = ?, update_time = ? WHERE id = ?",
            params![content, title, now, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

pub fn remove(id: &str) -> Result<(), String> {
    db::with_db(|conn| {
        conn.query_row(
            "SELECT id FROM note_document WHERE id = ? AND deleted = 0",
            params![id],
            |_| Ok(()),
        )
        .map_err(|_| "文稿不存在".to_string())?;

        let now = now_str();
        conn.execute(
            "UPDATE note_document SET deleted = 1, update_time = ? WHERE id = ?",
            params![now, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

pub fn restore(id: &str) -> Result<(), String> {
    db::with_db(|conn| {
        conn.query_row("SELECT id FROM note_document WHERE id = ?", params![id], |_| Ok(true))
            .map_err(|_| "文稿不存在".to_string())?;

        let now = now_str();
        let folder_id: Option<String> = conn
            .query_row("SELECT folder_id FROM note_document WHERE id = ?", params![id], |row| {
                row.get(0)
            })
            .map_err(|e| e.to_string())?;

        restore_folder_chain(conn, &now, folder_id.as_deref())?;

        conn.execute(
            "UPDATE note_document SET deleted = 0, update_time = ? WHERE id = ?",
            params![now, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

pub fn hard_delete(id: &str) -> Result<(), String> {
    db::with_db(|conn| {
        conn.query_row(
            "SELECT id FROM note_document WHERE id = ? AND deleted = 1",
            params![id],
            |_| Ok(()),
        )
        .map_err(|_| "文稿不存在或未在回收站中".to_string())?;

        let now = now_str();
        conn.execute(
            "UPDATE note_document_version SET deleted = 2, update_time = ? WHERE document_id = ?",
            params![now, id],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE note_document SET deleted = 2, update_time = ? WHERE id = ?",
            params![now, id],
        )
        .map_err(|e| e.to_string())?;

        promote_empty_folders_inner(conn);
        Ok(())
    })
}

pub fn empty_trash() -> Result<(), String> {
    db::with_db(|conn| {
        let now = now_str();
        conn.execute(
            "UPDATE note_document_version SET deleted = 2, update_time = ?
             WHERE document_id IN (SELECT id FROM note_document WHERE deleted = 1)",
            params![now],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE note_document SET deleted = 2, update_time = ? WHERE deleted = 1",
            params![now],
        )
        .map_err(|e| e.to_string())?;

        promote_empty_folders_inner(conn);
        Ok(())
    })
}

#[allow(dead_code)]
pub fn promote_empty_folders_to_deleted2() -> Result<i32, String> {
    Ok(db::with_db(|conn| promote_empty_folders_inner(conn)))
}

pub fn reorder_documents(_folder_id: &str, items: &[(String, i32)]) -> Result<(), String> {
    db::with_db(|conn| {
        let now = now_str();
        let mut stmt = conn
            .prepare("UPDATE note_document SET sort_order = ?, update_time = ? WHERE id = ?")
            .map_err(|e| e.to_string())?;
        for (id, sort_order) in items {
            stmt.execute(params![sort_order, now, id])
                .map_err(|e| format!("排序更新失败: {}", e))?;
        }
        Ok(())
    })
}

#[allow(dead_code)]
pub fn verify_exists(id: &str) -> Result<DocumentDTO, String> {
    db::with_db(|conn| {
        conn.query_row(
            "SELECT * FROM note_document WHERE id = ? AND deleted = 0",
            params![id],
            doc_from_row,
        )
        .map_err(|_| "文稿不存在".to_string())
    })
}
