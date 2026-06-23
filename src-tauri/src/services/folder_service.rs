use crate::db;
use crate::services::utils::{new_id, now_str};
use rusqlite::params;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderTreeNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub sort_order: i32,
    pub is_locked: bool,
    pub children: Vec<FolderTreeNode>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderDTO {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub name: String,
    pub sort_order: i32,
    pub is_locked: bool,
}

/// Get the full folder tree (matches folderService.getFolderTree)
pub fn get_folder_tree() -> Vec<FolderTreeNode> {
    db::with_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT id, parent_id AS parentId, name, sort_order AS sortOrder, is_locked AS isLocked
                 FROM note_folder WHERE deleted = 0
                 ORDER BY sort_order ASC, create_time ASC",
            )
            .unwrap();

        let rows: Vec<FolderDTO> = stmt
            .query_map([], |row| {
                Ok(FolderDTO {
                    id: row.get(0)?,
                    parent_id: row.get(1)?,
                    name: row.get(2)?,
                    sort_order: row.get(3)?,
                    is_locked: row.get::<_, i32>(4)? != 0,
                })
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        build_tree(&rows, None)
    })
}

fn build_tree(flat: &[FolderDTO], parent_id: Option<&str>) -> Vec<FolderTreeNode> {
    flat.iter()
        .filter(|f| f.parent_id.as_deref() == parent_id)
        .map(|f| FolderTreeNode {
            id: f.id.clone(),
            parent_id: f.parent_id.clone(),
            name: f.name.clone(),
            sort_order: f.sort_order,
            is_locked: f.is_locked,
            children: build_tree(flat, Some(&f.id)),
        })
        .collect()
}

/// Create a new folder (matches folderService.createFolder)
pub fn create_folder(name: &str, parent_id: Option<&str>) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("目录名不能为空".to_string());
    }
    let name = name.trim();

    db::with_db(|conn| {
        // Check for existing folder with same name under same parent
        let mut existing = conn
            .prepare("SELECT id, deleted FROM note_folder WHERE parent_id IS ? AND name = ?")
            .unwrap();
        let rows: Vec<(String, i32)> = existing
            .query_map(params![parent_id, name], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        // Active folder with same name → error
        if rows.iter().any(|(_, d)| *d == 0) {
            return Err("同级目录名不能重复".to_string());
        }

        // Soft-deleted folder → restore
        if let Some((id, _)) = rows.iter().find(|(_, d)| *d == 1) {
            let now = now_str();
            conn.execute(
                "UPDATE note_folder SET deleted = 0, update_time = ? WHERE id = ?",
                params![now, id],
            )
            .unwrap();
            return Ok(id.clone());
        }

        // Create new
        let id = new_id();
        let now = now_str();
        conn.execute(
            "INSERT INTO note_folder (id, parent_id, name, sort_order, create_time, update_time)
             VALUES (?, ?, ?, 0, ?, ?)",
            params![id, parent_id, name, now, now],
        )
        .unwrap();
        Ok(id)
    })
}

/// Rename a folder (matches folderService.renameFolder)
pub fn rename_folder(folder_id: &str, new_name: &str) -> Result<(), String> {
    let new_name = new_name.trim().to_string();
    if new_name.is_empty() {
        return Err("目录名不能为空".to_string());
    }

    db::with_db(|conn| {
        let parent_id: Option<String> = conn
            .query_row(
                "SELECT parent_id FROM note_folder WHERE id = ? AND deleted = 0",
                params![folder_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .map_err(|_| "目录不存在".to_string())?;

        // Check for duplicates
        let dup: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM note_folder WHERE parent_id IS ? AND name = ? AND deleted = 0 AND id != ?",
                params![parent_id, new_name, folder_id],
                |row| row.get::<_, i32>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);

        if dup {
            return Err("同级目录名不能重复".to_string());
        }

        let now = now_str();
        conn.execute(
            "UPDATE note_folder SET name = ?, update_time = ? WHERE id = ?",
            params![new_name, now, folder_id],
        )
        .map_err(|_| "更新失败".to_string())?;

        Ok(())
    })
}

/// Delete a folder (soft-delete with cascade, matches folderService.deleteFolder)
pub fn delete_folder(folder_id: &str) -> Result<(), String> {
    db::with_db(|conn| {
        // Verify exists
        conn.query_row(
            "SELECT id FROM note_folder WHERE id = ? AND deleted = 0",
            params![folder_id],
            |_| Ok(()),
        )
        .map_err(|_| "目录不存在".to_string())?;

        // Collect all descendant IDs
        let all: Vec<(String, Option<String>)> = conn
            .prepare("SELECT id, parent_id FROM note_folder WHERE deleted IN (0, 1)")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        let children_map = build_children_map(&all);
        let mut ids = Vec::new();
        collect_descendants(folder_id, &children_map, &mut ids);
        ids.push(folder_id.to_string());

        let now = now_str();
        let placeholders: Vec<String> = ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
        let placeholders_str = placeholders.join(",");

        // 1. Move documents to trash
        let sql1 = format!(
            "UPDATE note_document SET deleted = 1, update_time = ? WHERE folder_id IN ({}) AND deleted = 0",
            placeholders_str
        );
        let mut stmt1 = conn.prepare(&sql1).unwrap();
        let mut params_vec: Vec<&dyn rusqlite::types::ToSql> = vec![&now];
        for id in &ids {
            params_vec.push(id);
        }
        stmt1.execute(rusqlite::params_from_iter(params_vec)).ok();

        // 2. Check for trash documents in subtree
        let sql2 = format!(
            "SELECT COUNT(*) as cnt FROM note_document WHERE folder_id IN ({}) AND deleted = 1",
            placeholders_str
        );
        let trash_count: i32 = {
            let mut stmt2 = conn.prepare(&sql2).unwrap();
            let params_only_ids: Vec<&dyn rusqlite::types::ToSql> =
                ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
            stmt2
                .query_row(rusqlite::params_from_iter(params_only_ids), |row| row.get(0))
                .unwrap_or(0)
        };

        // 3. Set folder deleted status
        let target_deleted = if trash_count > 0 { 1 } else { 2 };
        for fid in &ids {
            conn.execute(
                "UPDATE note_folder SET deleted = ?, update_time = ? WHERE id = ? AND deleted = 0",
                params![target_deleted, now, fid],
            )
            .ok();
        }

        Ok(())
    })
}

/// Reorder folders (batch update sort_order, matches folderService.reorderFolders)
pub fn reorder_folders(_parent_id: Option<&str>, items: &[(String, i32)]) -> Result<(), String> {
    db::with_db(|conn| {
        let now = now_str();
        let mut stmt = conn
            .prepare("UPDATE note_folder SET sort_order = ?, update_time = ? WHERE id = ?")
            .unwrap();
        for (id, sort_order) in items {
            stmt.execute(params![sort_order, now, id])
                .map_err(|e| format!("排序更新失败: {}", e))?;
        }
        Ok(())
    })
}

fn build_children_map(
    rows: &[(String, Option<String>)],
) -> std::collections::HashMap<String, Vec<String>> {
    let mut map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for (id, pid) in rows {
        let key = pid.clone().unwrap_or_default();
        map.entry(key).or_default().push(id.clone());
    }
    map
}

fn collect_descendants(
    parent_id: &str,
    map: &std::collections::HashMap<String, Vec<String>>,
    result: &mut Vec<String>,
) {
    if let Some(children) = map.get(parent_id) {
        for child in children {
            result.push(child.clone());
            collect_descendants(child, map, result);
        }
    }
}
