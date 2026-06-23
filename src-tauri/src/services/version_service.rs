use crate::db::with_db;
use crate::services::utils::{new_id, now_str, sha256};
use crate::services::utils::strip_html;
use rusqlite::params;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionDTO {
    pub id: String,
    pub document_id: String,
    pub version_no: i32,
    pub title: Option<String>,
    pub content: Option<String>,
    pub content_hash: Option<String>,
    pub change_type: String,
    pub operator_id: Option<String>,
    pub create_time: String,
    pub update_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

fn build_preview(content: &Option<String>) -> Option<String> {
    let html = content.as_ref()?;
    if html.is_empty() {
        return None;
    }
    let plain = strip_html(html);
    let preview = plain.chars().take(80).collect::<String>();
    if preview.is_empty() { None } else { Some(preview) }
}

fn map_version_row(row: &rusqlite::Row) -> rusqlite::Result<VersionDTO> {
    let content: Option<String> = row.get("content")?;
    let preview = build_preview(&content);
    Ok(VersionDTO {
        id: row.get("id")?,
        document_id: row.get("document_id")?,
        version_no: row.get("version_no")?,
        title: row.get("title")?,
        content,
        content_hash: row.get("content_hash")?,
        change_type: row.get("change_type")?,
        operator_id: row.get("operator_id")?,
        create_time: row.get("create_time")?,
        update_time: row.get("update_time")?,
        preview,
    })
}

fn read_current_doc(document_id: &str) -> Result<(Option<String>, Option<String>), String> {
    with_db(|db| {
        db.query_row(
            "SELECT title, content FROM note_document WHERE id = ?1",
            params![document_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("文档不存在: {}", e))
    })
}

fn find_max_version_no(document_id: &str) -> Result<i32, String> {
    with_db(|db| {
        let max_no: i32 = db
            .query_row(
                "SELECT COALESCE(MAX(version_no), 0) FROM note_document_version WHERE document_id = ?1 AND deleted = 0",
                params![document_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(max_no)
    })
}

fn find_latest_content_hash(document_id: &str) -> Result<Option<String>, String> {
    with_db(|db| {
        let hash: Option<String> = db
            .query_row(
                "SELECT content_hash FROM note_document_version WHERE document_id = ?1 AND deleted = 0 ORDER BY version_no DESC LIMIT 1",
                params![document_id],
                |row| row.get(0),
            )
            .ok();
        Ok(hash)
    })
}

fn insert_version(
    document_id: &str,
    version_no: i32,
    title: Option<String>,
    content: Option<String>,
    content_hash: Option<String>,
    change_type: &str,
) -> Result<(String, String), String> {
    let id = new_id();
    let now = now_str();
    with_db(|db| {
        db.execute(
            "INSERT INTO note_document_version (id, document_id, version_no, title, content, content_hash, change_type, operator_id, create_time, update_time) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![id, document_id, version_no, title, content, content_hash, change_type, "local", now, now],
        )
        .map_err(|e| e.to_string())?;
        Ok((id, now))
    })
}

fn build_dto(
    id: String,
    document_id: String,
    version_no: i32,
    title: Option<String>,
    content: Option<String>,
    content_hash: Option<String>,
    change_type: &str,
    create_time: String,
    update_time: String,
) -> VersionDTO {
    let preview = build_preview(&content);
    VersionDTO {
        id,
        document_id,
        version_no,
        title,
        content,
        content_hash,
        change_type: change_type.to_string(),
        operator_id: Some("local".to_string()),
        create_time,
        update_time,
        preview,
    }
}

pub fn list_versions(document_id: &str) -> Result<Vec<VersionDTO>, String> {
    with_db(|db| {
        let mut stmt = db
            .prepare(
                "SELECT id, document_id, version_no, title, content, content_hash, change_type, operator_id, create_time, update_time \
                 FROM note_document_version WHERE document_id = ?1 AND deleted = 0 \
                 ORDER BY version_no DESC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![document_id], map_version_row)
            .map_err(|e| e.to_string())?;

        let mut versions = Vec::new();
        for row in rows {
            versions.push(row.map_err(|e| e.to_string())?);
        }
        Ok(versions)
    })
}

pub fn get_version(document_id: &str, version_no: i32) -> Result<VersionDTO, String> {
    with_db(|db| {
        let r = db
            .query_row(
                "SELECT id, document_id, version_no, title, content, content_hash, change_type, operator_id, create_time, update_time \
                 FROM note_document_version WHERE document_id = ?1 AND version_no = ?2 AND deleted = 0",
                params![document_id, version_no],
                map_version_row,
            )
            .map_err(|_| "版本不存在".to_string())?;
        Ok(r)
    })
}

pub fn save_manual(document_id: &str) -> Result<VersionDTO, String> {
    let (title, content) = read_current_doc(document_id)?;
    let content_str = content.clone().unwrap_or_default();
    let hash = sha256(&content_str);

    // Dedup: skip if content unchanged from latest version
    if let Ok(Some(latest_hash)) = find_latest_content_hash(document_id) {
        if latest_hash == hash {
            let max_no = find_max_version_no(document_id)?;
            return get_version(document_id, max_no);
        }
    }

    let max_no = find_max_version_no(document_id)?;
    let version_no = max_no + 1;
    let (id, now) = insert_version(document_id, version_no, title.clone(), content.clone(), Some(hash.clone()), "manual")?;

    Ok(build_dto(id, document_id.to_string(), version_no, title, content, Some(hash), "manual", now.clone(), now))
}

pub fn commit(document_id: &str, change_type: &str) -> Result<VersionDTO, String> {
    let normalized = if change_type.eq_ignore_ascii_case("manual") {
        "manual"
    } else {
        "auto"
    };

    let (title, content) = read_current_doc(document_id)?;
    let content_str = content.clone().unwrap_or_default();
    let hash = sha256(&content_str);

    // Dedup: skip if content unchanged from latest version
    if let Ok(Some(latest_hash)) = find_latest_content_hash(document_id) {
        if latest_hash == hash {
            let max_no = find_max_version_no(document_id)?;
            return get_version(document_id, max_no);
        }
    }

    let max_no = find_max_version_no(document_id)?;
    let version_no = max_no + 1;
    let (id, now) = insert_version(document_id, version_no, title.clone(), content.clone(), Some(hash.clone()), normalized)?;

    Ok(build_dto(id, document_id.to_string(), version_no, title, content, Some(hash), normalized, now.clone(), now))
}

pub fn restore(document_id: &str, version_no: i32) -> Result<(), String> {
    let target = get_version(document_id, version_no)?;
    let (current_title, current_content) = read_current_doc(document_id)?;
    let current_content_str = current_content.clone().unwrap_or_default();
    let current_hash = sha256(&current_content_str);
    let max_no = find_max_version_no(document_id)?;
    let backup_version_no = max_no + 1;

    insert_version(
        document_id,
        backup_version_no,
        current_title,
        current_content,
        Some(current_hash),
        "restore",
    )?;

    with_db(|db| {
        db.execute(
            "UPDATE note_document SET title = ?1, content = ?2, update_time = ?3 WHERE id = ?4",
            params![target.title, target.content, now_str(), document_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// Compact version history: mark old versions beyond `max_versions` as deleted.
/// Returns the number of versions compacted.
pub fn compact_history(document_id: &str, max_versions: i32) -> Result<i32, String> {
    let now = now_str();
    with_db(|db| {
        let count = db.execute(
            "UPDATE note_document_version SET deleted = 2, update_time = ?1 \
             WHERE document_id = ?2 AND deleted = 0 \
             AND version_no NOT IN ( \
                 SELECT version_no FROM note_document_version \
                 WHERE document_id = ?2 AND deleted = 0 \
                 ORDER BY version_no DESC LIMIT ?3 \
             )",
            params![now, document_id, max_versions],
        )
        .map_err(|e| e.to_string())?;
        Ok(count as i32)
    })
}

#[allow(dead_code)]
/// Compact all documents. Common use: keep latest 50 versions per document.
pub fn compact_all_documents(max_versions: i32) -> Result<i32, String> {
    let doc_ids: Vec<String> = with_db::<_, Result<Vec<String>, String>>(|db| {
        let mut stmt = db
            .prepare("SELECT DISTINCT document_id FROM note_document_version WHERE deleted = 0")
            .map_err(|e| e.to_string())?;
        let ids = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        Ok(ids.filter_map(|r| r.ok()).collect())
    })?;

    let mut total = 0;
    for doc_id in &doc_ids {
        total += compact_history(doc_id, max_versions)?;
    }
    Ok(total)
}
