use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::HashSet;

use super::convert::{self, ContentFormat};
use super::{presence, McpShared, ToolError};
use crate::db;
use crate::services::{document_service, folder_service, utils, version_service};

const INTERNAL_ERROR: &str = "内部错误";

fn db_call<T>(
    shared: &McpShared,
    f: impl FnOnce(&Connection) -> Result<T, ToolError>,
) -> Result<T, ToolError> {
    match db::with_db_for_user(shared.bound_user.as_deref(), f) {
        Ok(result) => result,
        Err(message) => Err(ToolError::UserChanged(message)),
    }
}

fn locked_folder_ids(conn: &Connection) -> Result<HashSet<String>, ToolError> {
    let mut stmt = conn
        .prepare(
            "WITH RECURSIVE locked_folder(id) AS (
                SELECT id FROM note_folder WHERE is_locked = 1
                UNION
                SELECT f.id FROM note_folder f
                JOIN locked_folder l ON f.parent_id = l.id
             )
             SELECT id FROM locked_folder",
        )
        .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
    let mut result = HashSet::new();
    for row in rows {
        result.insert(
            row.map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?,
        );
    }
    Ok(result)
}

fn doc_is_locked(
    is_locked: bool,
    folder_id: Option<&str>,
    locked_folders: &HashSet<String>,
) -> bool {
    is_locked
        || folder_id
            .map(|folder| locked_folders.contains(folder))
            .unwrap_or(false)
}

#[derive(Debug)]
struct DocRow {
    id: String,
    folder_id: Option<String>,
    folder_name: Option<String>,
    title: String,
    content: Option<String>,
    create_time: String,
    update_time: String,
    is_locked: bool,
    deleted: i32,
}

fn read_doc(conn: &Connection, id: &str) -> Result<DocRow, ToolError> {
    conn.query_row(
        "SELECT d.id, d.folder_id, f.name, d.title, d.content, d.create_time, d.update_time,
                d.is_locked, d.deleted
         FROM note_document d
         LEFT JOIN note_folder f ON d.folder_id = f.id
         WHERE d.id = ?1",
        params![id],
        |row| {
            Ok(DocRow {
                id: row.get(0)?,
                folder_id: row.get(1)?,
                folder_name: row.get(2)?,
                title: row.get(3)?,
                content: row.get(4)?,
                create_time: row.get(5)?,
                update_time: row.get(6)?,
                is_locked: row.get::<_, i32>(7)? != 0,
                deleted: row.get(8)?,
            })
        },
    )
    .optional()
    .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?
    .ok_or_else(|| ToolError::NotFound("文稿不存在".to_string()))
}

fn require_active_unlocked(
    conn: &Connection,
    id: &str,
) -> Result<(DocRow, HashSet<String>), ToolError> {
    let locked_folders = locked_folder_ids(conn)?;
    let doc = read_doc(conn, id)?;
    if doc.deleted != 0 {
        return Err(ToolError::NotFound("文稿不存在".to_string()));
    }
    if doc_is_locked(doc.is_locked, doc.folder_id.as_deref(), &locked_folders) {
        return Err(ToolError::Locked(
            "文稿已锁定，请在 Woo 中解锁后再试。".to_string(),
        ));
    }
    Ok((doc, locked_folders))
}

fn escape_like(query: &str) -> String {
    let mut escaped = String::with_capacity(query.len());
    for ch in query.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

pub fn clamp_limit(limit: Option<u32>, default: usize, max: usize) -> usize {
    limit
        .map(|value| value as usize)
        .unwrap_or(default)
        .clamp(1, max)
}

pub fn clamp_offset(offset: Option<u32>) -> usize {
    offset.unwrap_or(0) as usize
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    pub id: String,
    pub title: String,
    pub folder_id: Option<String>,
    pub folder_name: Option<String>,
    pub update_time: String,
    pub preview: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteDetail {
    pub id: String,
    pub title: String,
    pub folder_id: Option<String>,
    pub folder_name: Option<String>,
    pub content: String,
    pub format: String,
    pub content_hash: String,
    pub create_time: String,
    pub update_time: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub is_locked: bool,
    pub children: Vec<FolderNode>,
}

#[derive(Debug)]
struct FlatFolder {
    id: String,
    parent_id: Option<String>,
    name: String,
    is_locked: bool,
}

fn build_folder_tree(
    flat: &[FlatFolder],
    parent_id: Option<&str>,
    locked_folders: &HashSet<String>,
) -> Vec<FolderNode> {
    flat.iter()
        .filter(|folder| folder.parent_id.as_deref() == parent_id)
        .map(|folder| FolderNode {
            id: folder.id.clone(),
            parent_id: folder.parent_id.clone(),
            name: folder.name.clone(),
            is_locked: folder.is_locked || locked_folders.contains(&folder.id),
            children: build_folder_tree(flat, Some(&folder.id), locked_folders),
        })
        .collect()
}

pub fn list_folders(shared: &McpShared) -> Result<Vec<FolderNode>, ToolError> {
    db_call(shared, |conn| {
        let locked_folders = locked_folder_ids(conn)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, parent_id, name, is_locked
                 FROM note_folder WHERE deleted = 0
                 ORDER BY sort_order ASC, create_time ASC",
            )
            .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(FlatFolder {
                    id: row.get(0)?,
                    parent_id: row.get(1)?,
                    name: row.get(2)?,
                    is_locked: row.get::<_, i32>(3)? != 0,
                })
            })
            .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
        let mut flat = Vec::new();
        for row in rows {
            flat.push(
                row.map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?,
            );
        }
        Ok(build_folder_tree(&flat, None, &locked_folders))
    })
}

pub fn list_notes(
    shared: &McpShared,
    folder_id: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<(Vec<NoteSummary>, bool), ToolError> {
    let folder_id = folder_id.filter(|value| !value.is_empty());
    let fetch_limit = limit.saturating_add(1) as i64;
    let offset = offset as i64;
    db_call(shared, |conn| {
        let mut stmt = conn
            .prepare(
                "WITH RECURSIVE locked_folder(id) AS (
                    SELECT id FROM note_folder WHERE is_locked = 1
                    UNION
                    SELECT f.id FROM note_folder f
                    JOIN locked_folder l ON f.parent_id = l.id
                 )
                 SELECT d.id, d.title, d.folder_id, f.name, d.update_time, d.content
                 FROM note_document d
                 LEFT JOIN note_folder f ON d.folder_id = f.id
                 WHERE d.deleted = 0
                   AND d.is_locked = 0
                   AND (d.folder_id IS NULL OR d.folder_id NOT IN (SELECT id FROM locked_folder))
                   AND (?1 IS NULL OR d.folder_id = ?1)
                 ORDER BY d.update_time DESC
                 LIMIT ?2 OFFSET ?3",
            )
            .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
        let rows = stmt
            .query_map(params![folder_id, fetch_limit, offset], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            })
            .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
        let mut items = Vec::new();
        for row in rows {
            let (id, title, folder_id, folder_name, update_time, content) =
                row.map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
            items.push(NoteSummary {
                id,
                title,
                folder_id,
                folder_name,
                update_time,
                preview: convert::preview(content.as_deref().unwrap_or(""), 160),
            });
        }
        let has_more = items.len() > limit;
        items.truncate(limit);
        Ok((items, has_more))
    })
}

pub fn search_notes(
    shared: &McpShared,
    query: &str,
    folder_id: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<(Vec<NoteSummary>, bool), ToolError> {
    let folder_id = folder_id.filter(|value| !value.is_empty());
    let pattern = format!("%{}%", escape_like(query.trim()));
    let fetch_limit = limit.saturating_add(1) as i64;
    let offset = offset as i64;
    db_call(shared, |conn| {
        let mut stmt = conn
            .prepare(
                "WITH RECURSIVE locked_folder(id) AS (
                    SELECT id FROM note_folder WHERE is_locked = 1
                    UNION
                    SELECT f.id FROM note_folder f
                    JOIN locked_folder l ON f.parent_id = l.id
                 )
                 SELECT d.id, d.title, d.folder_id, f.name, d.update_time, d.content
                 FROM note_document d
                 LEFT JOIN note_folder f ON d.folder_id = f.id
                 WHERE d.deleted = 0
                   AND d.is_locked = 0
                   AND (d.folder_id IS NULL OR d.folder_id NOT IN (SELECT id FROM locked_folder))
                   AND (?1 IS NULL OR d.folder_id = ?1)
                   AND (d.title LIKE ?2 ESCAPE '\\' OR d.content LIKE ?2 ESCAPE '\\')
                 ORDER BY d.update_time DESC
                 LIMIT ?3 OFFSET ?4",
            )
            .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
        let rows = stmt
            .query_map(params![folder_id, pattern, fetch_limit, offset], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            })
            .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
        let mut items = Vec::new();
        for row in rows {
            let (id, title, folder_id, folder_name, update_time, content) =
                row.map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
            items.push(NoteSummary {
                id,
                title,
                folder_id,
                folder_name,
                update_time,
                preview: convert::preview(content.as_deref().unwrap_or(""), 160),
            });
        }
        let has_more = items.len() > limit;
        items.truncate(limit);
        Ok((items, has_more))
    })
}

pub fn list_trash(
    shared: &McpShared,
    limit: usize,
    offset: usize,
) -> Result<(Vec<NoteSummary>, bool), ToolError> {
    let fetch_limit = limit.saturating_add(1) as i64;
    let offset = offset as i64;
    db_call(shared, |conn| {
        let mut stmt = conn
            .prepare(
                "WITH RECURSIVE locked_folder(id) AS (
                    SELECT id FROM note_folder WHERE is_locked = 1
                    UNION
                    SELECT f.id FROM note_folder f
                    JOIN locked_folder l ON f.parent_id = l.id
                 )
                 SELECT d.id, d.title, d.folder_id, f.name, d.update_time, d.content
                 FROM note_document d
                 LEFT JOIN note_folder f ON d.folder_id = f.id
                 WHERE d.deleted = 1
                   AND d.is_locked = 0
                   AND (d.folder_id IS NULL OR d.folder_id NOT IN (SELECT id FROM locked_folder))
                 ORDER BY d.update_time DESC
                 LIMIT ?1 OFFSET ?2",
            )
            .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
        let rows = stmt
            .query_map(params![fetch_limit, offset], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            })
            .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
        let mut items = Vec::new();
        for row in rows {
            let (id, title, folder_id, folder_name, update_time, content) =
                row.map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
            items.push(NoteSummary {
                id,
                title,
                folder_id,
                folder_name,
                update_time,
                preview: convert::preview(content.as_deref().unwrap_or(""), 160),
            });
        }
        let has_more = items.len() > limit;
        items.truncate(limit);
        Ok((items, has_more))
    })
}

fn render_content(html: &str, format: ContentFormat) -> Result<String, ToolError> {
    match format {
        ContentFormat::Markdown => {
            convert::html_to_markdown(html).map_err(ToolError::Internal)
        }
        ContentFormat::Html => Ok(html.to_string()),
    }
}

pub fn get_note(
    shared: &McpShared,
    id: &str,
    format: ContentFormat,
) -> Result<NoteDetail, ToolError> {
    db_call(shared, |conn| {
        let (doc, _) = require_active_unlocked(conn, id)?;
        let html = doc.content.unwrap_or_default();
        let content = render_content(&html, format)?;
        Ok(NoteDetail {
            id: doc.id,
            title: doc.title,
            folder_id: doc.folder_id,
            folder_name: doc.folder_name,
            content,
            format: format.as_str().to_string(),
            content_hash: utils::sha256(&html),
            create_time: doc.create_time,
            update_time: doc.update_time,
        })
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionSummary {
    pub version_no: i32,
    pub title: Option<String>,
    pub change_type: String,
    pub create_time: String,
    pub preview: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionDetail {
    pub document_id: String,
    pub version_no: i32,
    pub title: Option<String>,
    pub content: String,
    pub format: String,
    pub change_type: String,
    pub create_time: String,
}

pub fn list_note_versions(
    shared: &McpShared,
    document_id: &str,
    limit: usize,
    offset: usize,
) -> Result<(Vec<VersionSummary>, bool), ToolError> {
    let versions = db_call(shared, |conn| {
        require_active_unlocked(conn, document_id)?;
        version_service::list_versions_inner(conn, document_id).map_err(ToolError::Internal)
    })?;
    let mut items: Vec<VersionSummary> = versions
        .into_iter()
        .map(|version| VersionSummary {
            version_no: version.version_no,
            title: version.title,
            change_type: version.change_type,
            create_time: version.create_time,
            preview: version.preview,
        })
        .collect();
    let has_more = items.len() > offset + limit;
    items = items.into_iter().skip(offset).take(limit).collect();
    Ok((items, has_more))
}

pub fn get_note_version(
    shared: &McpShared,
    document_id: &str,
    version_no: i32,
    format: ContentFormat,
) -> Result<VersionDetail, ToolError> {
    let version = db_call(shared, |conn| {
        require_active_unlocked(conn, document_id)?;
        version_service::get_version_inner(conn, document_id, version_no)
            .map_err(ToolError::Internal)
    })?;
    let html = version.content.unwrap_or_default();
    let content = render_content(&html, format)?;
    Ok(VersionDetail {
        document_id: version.document_id,
        version_no: version.version_no,
        title: version.title,
        content,
        format: format.as_str().to_string(),
        change_type: version.change_type,
        create_time: version.create_time,
    })
}

fn prepare_content(content: &str, format: ContentFormat) -> Result<String, ToolError> {
    if content.len() > 2 * 1024 * 1024 {
        return Err(ToolError::Invalid("正文超过 2 MiB 限制".to_string()));
    }
    Ok(match format {
        ContentFormat::Markdown => convert::markdown_to_html(content),
        ContentFormat::Html => convert::sanitize_html(content),
    })
}

fn validate_title(title: &str) -> Result<String, ToolError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(ToolError::Invalid("标题不能为空".to_string()));
    }
    if title.chars().count() > 500 {
        return Err(ToolError::Invalid("标题超过 500 字符".to_string()));
    }
    Ok(title.to_string())
}

fn now() -> String {
    utils::now_str()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateResult {
    pub id: String,
    pub title: String,
    pub folder_id: Option<String>,
    pub update_time: String,
    pub content_hash: String,
}

pub fn create_note(
    shared: &McpShared,
    folder_id: &str,
    title: Option<&str>,
    content: &str,
    format: ContentFormat,
) -> Result<CreateResult, ToolError> {
    if folder_id.trim().is_empty() {
        return Err(ToolError::Invalid("folderId 不能为空".to_string()));
    }
    let html = prepare_content(content, format)?;
    let title = match title {
        Some(value) => validate_title(value)?,
        None => {
            let derived = convert::derive_title(&html);
            if derived.trim().is_empty() {
                "新文稿".to_string()
            } else {
                derived
            }
        }
    };

    let dto = db_call(shared, |conn| {
        let locked_folders = locked_folder_ids(conn)?;
        let deleted: Option<i32> = conn
            .query_row(
                "SELECT deleted FROM note_folder WHERE id = ?1",
                params![folder_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
        match deleted {
            None => return Err(ToolError::NotFound("目录不存在".to_string())),
            Some(value) if value != 0 => {
                return Err(ToolError::NotFound("目录不存在".to_string()))
            }
            Some(_) if locked_folders.contains(folder_id) => {
                return Err(ToolError::Locked("目录已锁定，无法新建文稿".to_string()))
            }
            Some(_) => {}
        }
        document_service::create_inner(conn, folder_id, &title, Some(&html))
            .map_err(ToolError::Internal)
    })?;

    let _ = version_service::commit(&dto.id, "auto");
    Ok(CreateResult {
        id: dto.id,
        title: dto.title,
        folder_id: dto.folder_id.into(),
        update_time: dto.update_time,
        content_hash: utils::sha256(&html),
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    pub id: String,
    pub title: String,
    pub update_time: String,
    pub content_hash: String,
}

fn check_expected(
    doc: &DocRow,
    expected_hash: Option<&str>,
    expected_time: Option<&str>,
) -> Result<(), ToolError> {
    let old_content = doc.content.as_deref().unwrap_or("");
    let old_hash = utils::sha256(old_content);
    let hash_mismatch = expected_hash
        .map(|expected| expected != old_hash)
        .unwrap_or(false);
    let time_mismatch = expected_time
        .map(|expected| expected != doc.update_time)
        .unwrap_or(false);
    if hash_mismatch || time_mismatch {
        return Err(ToolError::Conflict {
            current_content_hash: old_hash,
            current_update_time: doc.update_time.clone(),
            current_title: doc.title.clone(),
        });
    }
    Ok(())
}

pub fn update_note(
    shared: &McpShared,
    id: &str,
    content: &str,
    format: ContentFormat,
    title: Option<&str>,
    derive_title: bool,
    expected_hash: &str,
    expected_time: Option<&str>,
) -> Result<UpdateResult, ToolError> {
    if presence::is_editing(id) {
        return Err(ToolError::EditorBusy(
            "当前文稿正在 Woo 中编辑且有未保存修改，请先保存后再让 AI 修改。".to_string(),
        ));
    }
    if expected_hash.trim().is_empty() {
        return Err(ToolError::Invalid(
            "expectedContentHash 不能为空；请先 get_note 获取 contentHash".to_string(),
        ));
    }
    let html = prepare_content(content, format)?;
    let result = db_call(shared, |conn| {
        let (doc, _) = require_active_unlocked(conn, id)?;
        check_expected(&doc, Some(expected_hash), expected_time)?;
        let new_title = match title {
            Some(value) => validate_title(value)?,
            None if derive_title => {
                let derived = convert::derive_title(&html);
                if derived.trim().is_empty() {
                    doc.title.clone()
                } else {
                    derived
                }
            }
            None => doc.title.clone(),
        };
        version_service::insert_snapshot(
            conn,
            id,
            &doc.title,
            doc.content.as_deref().unwrap_or(""),
            "auto",
        )
        .map_err(ToolError::Internal)?;
        let now = now();
        conn.execute(
            "UPDATE note_document SET content = ?1, title = ?2, update_time = ?3
             WHERE id = ?4 AND deleted = 0",
            params![html, new_title, now, id],
        )
        .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
        Ok(UpdateResult {
            id: id.to_string(),
            title: new_title,
            update_time: now,
            content_hash: utils::sha256(&html),
        })
    })?;
    let _ = version_service::commit(id, "auto");
    Ok(result)
}

pub fn append_note(
    shared: &McpShared,
    id: &str,
    content: &str,
    format: ContentFormat,
    separator: Option<&str>,
    expected_hash: &str,
    expected_time: Option<&str>,
) -> Result<UpdateResult, ToolError> {
    if presence::is_editing(id) {
        return Err(ToolError::EditorBusy(
            "当前文稿正在 Woo 中编辑且有未保存修改，请先保存后再让 AI 追加。".to_string(),
        ));
    }
    if expected_hash.trim().is_empty() {
        return Err(ToolError::Invalid(
            "expectedContentHash 不能为空；请先 get_note 获取 contentHash".to_string(),
        ));
    }
    let appended_html = prepare_content(content, format)?;
    let separator = separator.unwrap_or("\n");
    if separator.len() > 1000 {
        return Err(ToolError::Invalid("separator 过长".to_string()));
    }
    let separator = convert::sanitize_text(separator);
    let result = db_call(shared, |conn| {
        let (doc, _) = require_active_unlocked(conn, id)?;
        check_expected(&doc, Some(expected_hash), expected_time)?;
        let old_content = doc.content.as_deref().unwrap_or("");
        version_service::insert_snapshot(conn, id, &doc.title, old_content, "auto")
            .map_err(ToolError::Internal)?;
        let new_html = if old_content.is_empty() {
            appended_html.clone()
        } else {
            format!("{old_content}{separator}{appended_html}")
        };
        let now = now();
        conn.execute(
            "UPDATE note_document SET content = ?1, update_time = ?2
             WHERE id = ?3 AND deleted = 0",
            params![new_html, now, id],
        )
        .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
        Ok(UpdateResult {
            id: id.to_string(),
            title: doc.title,
            update_time: now,
            content_hash: utils::sha256(&new_html),
        })
    })?;
    let _ = version_service::commit(id, "auto");
    Ok(result)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameResult {
    pub id: String,
    pub title: String,
    pub update_time: String,
}

pub fn rename_note(
    shared: &McpShared,
    id: &str,
    title: &str,
    expected_hash: Option<&str>,
    expected_time: Option<&str>,
) -> Result<RenameResult, ToolError> {
    if presence::is_editing(id) {
        return Err(ToolError::EditorBusy(
            "当前文稿正在 Woo 中编辑且有未保存修改，请先保存后再让 AI 重命名。".to_string(),
        ));
    }
    let title = validate_title(title)?;
    db_call(shared, |conn| {
        let (doc, _) = require_active_unlocked(conn, id)?;
        check_expected(&doc, expected_hash, expected_time)?;
        let now = now();
        conn.execute(
            "UPDATE note_document SET title = ?1, update_time = ?2
             WHERE id = ?3 AND deleted = 0",
            params![title, now, id],
        )
        .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
        Ok(RenameResult {
            id: id.to_string(),
            title,
            update_time: now,
        })
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveResult {
    pub id: String,
    pub folder_id: String,
    pub update_time: String,
}

pub fn move_note(
    shared: &McpShared,
    id: &str,
    folder_id: &str,
    expected_time: Option<&str>,
) -> Result<MoveResult, ToolError> {
    if presence::is_editing(id) {
        return Err(ToolError::EditorBusy(
            "当前文稿正在 Woo 中编辑且有未保存修改，请先保存后再移动。".to_string(),
        ));
    }
    if folder_id.trim().is_empty() {
        return Err(ToolError::Invalid("folderId 不能为空".to_string()));
    }
    db_call(shared, |conn| {
        let (doc, locked_folders) = require_active_unlocked(conn, id)?;
        check_expected(&doc, None, expected_time)?;
        let target_deleted: Option<i32> = conn
            .query_row(
                "SELECT deleted FROM note_folder WHERE id = ?1",
                params![folder_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
        match target_deleted {
            None => return Err(ToolError::NotFound("目标目录不存在".to_string())),
            Some(value) if value != 0 => {
                return Err(ToolError::NotFound("目标目录不存在".to_string()))
            }
            Some(_) if locked_folders.contains(folder_id) => {
                return Err(ToolError::Locked("目标目录已锁定".to_string()))
            }
            Some(_) => {}
        }
        let next_sort: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1
                 FROM note_document WHERE folder_id = ?1 AND deleted = 0",
                params![folder_id],
                |row| row.get(0),
            )
            .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
        let now = now();
        conn.execute(
            "UPDATE note_document SET folder_id = ?1, sort_order = ?2, update_time = ?3
             WHERE id = ?4 AND deleted = 0",
            params![folder_id, next_sort, now, id],
        )
        .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
        Ok(MoveResult {
            id: id.to_string(),
            folder_id: folder_id.to_string(),
            update_time: now,
        })
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
    pub id: String,
    pub trashed: bool,
    pub update_time: String,
}

pub fn delete_note(
    shared: &McpShared,
    id: &str,
    expected_time: Option<&str>,
) -> Result<DeleteResult, ToolError> {
    if presence::is_editing(id) {
        return Err(ToolError::EditorBusy(
            "当前文稿正在 Woo 中编辑且有未保存修改，请先保存后再移入回收站。".to_string(),
        ));
    }
    db_call(shared, |conn| {
        let (doc, _) = require_active_unlocked(conn, id)?;
        check_expected(&doc, None, expected_time)?;
        let now = now();
        conn.execute(
            "UPDATE note_document SET deleted = 1, update_time = ?1 WHERE id = ?2",
            params![now, id],
        )
        .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
        Ok(DeleteResult {
            id: id.to_string(),
            trashed: true,
            update_time: now,
        })
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    pub id: String,
    pub folder_id: Option<String>,
    pub update_time: String,
}

pub fn restore_note(shared: &McpShared, id: &str) -> Result<RestoreResult, ToolError> {
    let folder_id = db_call(shared, |conn| {
        let doc = read_doc(conn, id)?;
        if doc.deleted != 1 {
            return Err(ToolError::NotFound("文稿不在回收站".to_string()));
        }
        let locked_folders = locked_folder_ids(conn)?;
        if let Some(folder) = doc.folder_id.as_deref() {
            if locked_folders.contains(folder) {
                return Err(ToolError::Locked("目标目录已锁定，无法恢复".to_string()));
            }
        }
        document_service::restore_inner(conn, id).map_err(ToolError::Internal)?;
        Ok(doc.folder_id)
    })?;
    Ok(RestoreResult {
        id: id.to_string(),
        folder_id,
        update_time: now(),
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderResult {
    pub id: String,
    pub name: String,
}

pub fn create_folder(
    shared: &McpShared,
    name: &str,
    parent_id: Option<&str>,
) -> Result<FolderResult, ToolError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ToolError::Invalid("目录名不能为空".to_string()));
    }
    if name.chars().count() > 200 {
        return Err(ToolError::Invalid("目录名超过 200 字符".to_string()));
    }
    let parent = parent_id.filter(|value| !value.is_empty());
    let id = db_call(shared, |conn| {
        if let Some(parent) = parent {
            let locked_folders = locked_folder_ids(conn)?;
            let deleted: Option<i32> = conn
                .query_row(
                    "SELECT deleted FROM note_folder WHERE id = ?1",
                    params![parent],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
            match deleted {
                None => return Err(ToolError::NotFound("父目录不存在".to_string())),
                Some(value) if value != 0 => {
                    return Err(ToolError::NotFound("父目录不存在".to_string()))
                }
                Some(_) if locked_folders.contains(parent) => {
                    return Err(ToolError::Locked("父目录已锁定".to_string()))
                }
                Some(_) => {}
            }
        }
        folder_service::create_folder_inner(conn, name, parent).map_err(ToolError::Internal)
    })?;
    Ok(FolderResult {
        id,
        name: name.to_string(),
    })
}

pub fn rename_folder(
    shared: &McpShared,
    folder_id: &str,
    name: &str,
) -> Result<FolderResult, ToolError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ToolError::Invalid("目录名不能为空".to_string()));
    }
    if name.chars().count() > 200 {
        return Err(ToolError::Invalid("目录名超过 200 字符".to_string()));
    }
    db_call(shared, |conn| {
        let locked_folders = locked_folder_ids(conn)?;
        let deleted: Option<i32> = conn
            .query_row(
                "SELECT deleted FROM note_folder WHERE id = ?1",
                params![folder_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| ToolError::Internal(format!("{INTERNAL_ERROR}: {error}")))?;
        match deleted {
            None => return Err(ToolError::NotFound("目录不存在".to_string())),
            Some(value) if value != 0 => {
                return Err(ToolError::NotFound("目录不存在".to_string()))
            }
            Some(_) if locked_folders.contains(folder_id) => {
                return Err(ToolError::Locked("目录已锁定".to_string()))
            }
            Some(_) => {}
        }
        folder_service::rename_folder_inner(conn, folder_id, name).map_err(ToolError::Internal)
    })?;
    Ok(FolderResult {
        id: folder_id.to_string(),
        name: name.to_string(),
    })
}
