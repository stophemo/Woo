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
            let next_order: i32 = conn
                .query_row(
                    "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM note_folder
                     WHERE parent_id IS ? AND deleted = 0",
                    params![parent_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            conn.execute(
                "UPDATE note_folder SET deleted = 0, sort_order = ?, update_time = ? WHERE id = ?",
                params![next_order, now, id],
            )
            .unwrap();
            return Ok(id.clone());
        }

        // Create new：排到同级末尾，与前端「新建后追加到列表尾部」保持一致
        let id = new_id();
        let now = now_str();
        let next_order: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM note_folder
                 WHERE parent_id IS ? AND deleted = 0",
                params![parent_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO note_folder (id, parent_id, name, sort_order, create_time, update_time)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![id, parent_id, name, next_order, now, now],
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

/// 把目录移动到新的父目录下，并放到同级的指定位置。
///
/// - `new_parent_id = None` 表示移到根层级
/// - `position = None` 表示追加到同级末尾
/// 会拒绝：移到自己、移到自己的子孙（成环）、目标层级同名冲突。
pub fn move_folder(
    folder_id: &str,
    new_parent_id: Option<&str>,
    position: Option<i32>,
) -> Result<(), String> {
    db::with_db(|conn| {
        let now = now_str();
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("开启事务失败: {}", e))?;
        move_folder_inner(&tx, folder_id, new_parent_id, position, &now)?;
        tx.commit().map_err(|e| format!("提交事务失败: {}", e))?;
        Ok(())
    })
}

fn move_folder_inner(
    conn: &rusqlite::Connection,
    folder_id: &str,
    new_parent_id: Option<&str>,
    position: Option<i32>,
    now: &str,
) -> Result<(), String> {
    // 1. 源目录必须存在
    let (old_parent_id, name) = conn
        .query_row(
            "SELECT parent_id, name FROM note_folder WHERE id = ? AND deleted = 0",
            params![folder_id],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(|_| "目录不存在".to_string())?;

    // 2. 不能移到自己下面
    if new_parent_id == Some(folder_id) {
        return Err("不能把目录移动到自身下级".to_string());
    }

    // 3. 目标父目录必须存在，且不能是自己的子孙（否则子树会脱离根成环）
    if let Some(parent_id) = new_parent_id {
        conn.query_row(
            "SELECT id FROM note_folder WHERE id = ? AND deleted = 0",
            params![parent_id],
            |_| Ok(()),
        )
        .map_err(|_| "目标目录不存在".to_string())?;

        let mut cursor = Some(parent_id.to_string());
        let mut visited = std::collections::HashSet::new();
        while let Some(current) = cursor {
            if current == folder_id {
                return Err("不能把目录移动到自身的子目录下".to_string());
            }
            if !visited.insert(current.clone()) {
                break;
            }
            cursor = conn
                .query_row(
                    "SELECT parent_id FROM note_folder WHERE id = ?",
                    params![current],
                    |row| row.get::<_, Option<String>>(0),
                )
                .unwrap_or(None);
        }
    }

    // 4. 目标层级同名冲突
    let dup: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM note_folder
             WHERE parent_id IS ? AND name = ? AND deleted = 0 AND id != ?",
            params![new_parent_id, name, folder_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if dup > 0 {
        return Err("目标层级已存在同名目录".to_string());
    }

    // 5. 改父级
    conn.execute(
        "UPDATE note_folder SET parent_id = ?, update_time = ? WHERE id = ?",
        params![new_parent_id, now, folder_id],
    )
    .map_err(|e| format!("移动目录失败: {}", e))?;

    // 6. 在新层级里按 position 插入并重排 sort_order
    let mut siblings: Vec<String> = conn
        .prepare(
            "SELECT id FROM note_folder
             WHERE parent_id IS ? AND deleted = 0 AND id != ?
             ORDER BY sort_order ASC, create_time ASC",
        )
        .map_err(|e| e.to_string())?
        .query_map(params![new_parent_id, folder_id], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let insert_at = position
        .map(|p| p.max(0) as usize)
        .unwrap_or(siblings.len())
        .min(siblings.len());
    siblings.insert(insert_at, folder_id.to_string());
    renumber(conn, &siblings, now)?;

    // 7. 原层级留下的空位一并压实，避免序号出现空洞
    if old_parent_id.as_deref() != new_parent_id {
        let remaining: Vec<String> = conn
            .prepare(
                "SELECT id FROM note_folder
                 WHERE parent_id IS ? AND deleted = 0
                 ORDER BY sort_order ASC, create_time ASC",
            )
            .map_err(|e| e.to_string())?
            .query_map(params![old_parent_id], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        renumber(conn, &remaining, now)?;
    }

    Ok(())
}

fn renumber(conn: &rusqlite::Connection, ordered_ids: &[String], now: &str) -> Result<(), String> {
    let mut stmt = conn
        .prepare("UPDATE note_folder SET sort_order = ?, update_time = ? WHERE id = ?")
        .map_err(|e| e.to_string())?;
    for (idx, id) in ordered_ids.iter().enumerate() {
        stmt.execute(params![idx as i32, now, id])
            .map_err(|e| format!("排序更新失败: {}", e))?;
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// 造一张与线上一致的 note_folder 表，避免依赖 init_schema（它是私有函数）。
    fn build_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE note_folder (
                id TEXT PRIMARY KEY,
                parent_id TEXT,
                name TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                create_time TEXT NOT NULL,
                update_time TEXT NOT NULL,
                deleted INTEGER NOT NULL DEFAULT 0,
                is_locked INTEGER NOT NULL DEFAULT 0
            );",
        )
        .unwrap();
        conn
    }

    fn insert_folder(
        conn: &Connection,
        id: &str,
        parent_id: Option<&str>,
        name: &str,
        sort_order: i32,
    ) {
        conn.execute(
            "INSERT INTO note_folder (id, parent_id, name, sort_order, create_time, update_time)
             VALUES (?1, ?2, ?3, ?4, 't', 't')",
            rusqlite::params![id, parent_id, name, sort_order],
        )
        .unwrap();
    }

    /// 读取某父级下的有序子目录 id 列表
    fn child_ids(conn: &Connection, parent_id: Option<&str>) -> Vec<String> {
        conn.prepare(
            "SELECT id FROM note_folder
             WHERE parent_id IS ? AND deleted = 0
             ORDER BY sort_order ASC, create_time ASC",
        )
        .unwrap()
        .query_map(rusqlite::params![parent_id], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    fn parent_of(conn: &Connection, id: &str) -> Option<String> {
        conn.query_row(
            "SELECT parent_id FROM note_folder WHERE id = ?",
            rusqlite::params![id],
            |row| row.get::<_, Option<String>>(0),
        )
        .unwrap()
    }

    #[test]
    fn move_folder_cross_level_reorders_both_layers() {
        let conn = build_conn();
        // 根：A, B, C；A 下：A1, A2；C 下：C1
        insert_folder(&conn, "A", None, "A", 0);
        insert_folder(&conn, "B", None, "B", 1);
        insert_folder(&conn, "C", None, "C", 2);
        insert_folder(&conn, "A1", Some("A"), "A1", 0);
        insert_folder(&conn, "A2", Some("A"), "A2", 1);
        insert_folder(&conn, "C1", Some("C"), "C1", 0);

        // 把 A2 移到 C 下、插到 C1 前面（position=0）
        move_folder_inner(&conn, "A2", Some("C"), Some(0), "t").unwrap();

        assert_eq!(parent_of(&conn, "A2").as_deref(), Some("C"));
        assert_eq!(
            child_ids(&conn, Some("C")),
            vec!["A2".to_string(), "C1".to_string()]
        );
        // 原层级压实：A 下只剩 A1，序号 0
        assert_eq!(child_ids(&conn, Some("A")), vec!["A1".to_string()]);
        assert_eq!(
            child_ids(&conn, None),
            vec!["A".to_string(), "B".to_string(), "C".to_string()]
        );
    }

    #[test]
    fn move_folder_to_root_appends_at_end() {
        let conn = build_conn();
        insert_folder(&conn, "A", None, "A", 0);
        insert_folder(&conn, "B", None, "B", 1);
        insert_folder(&conn, "A1", Some("A"), "A1", 0);

        // 子目录移到根，position=None → 追加到末尾
        move_folder_inner(&conn, "A1", None, None, "t").unwrap();

        assert_eq!(parent_of(&conn, "A1"), None);
        assert_eq!(
            child_ids(&conn, None),
            vec!["A".to_string(), "B".to_string(), "A1".to_string()]
        );
        assert!(child_ids(&conn, Some("A")).is_empty());
    }

    #[test]
    fn move_folder_same_level_honors_position_after_removing_source() {
        let conn = build_conn();
        insert_folder(&conn, "A", None, "A", 0);
        insert_folder(&conn, "B", None, "B", 1);
        insert_folder(&conn, "C", None, "C", 2);

        // A 放到 C 前面：目标列表移除 A 后为 B、C，因此 position=1。
        move_folder_inner(&conn, "A", None, Some(1), "t").unwrap();
        assert_eq!(
            child_ids(&conn, None),
            vec!["B".to_string(), "A".to_string(), "C".to_string()]
        );

        // C 放到 B 后面，position=1 应得到 B、C、A。
        move_folder_inner(&conn, "C", None, Some(1), "t").unwrap();
        assert_eq!(
            child_ids(&conn, None),
            vec!["B".to_string(), "C".to_string(), "A".to_string()]
        );
    }

    #[test]
    fn move_folder_rejects_moving_into_own_subtree() {
        let conn = build_conn();
        insert_folder(&conn, "A", None, "A", 0);
        insert_folder(&conn, "A1", Some("A"), "A1", 0);
        insert_folder(&conn, "A11", Some("A1"), "A11", 0);

        // A → A1（自己的子级）应被拒绝
        assert!(move_folder_inner(&conn, "A", Some("A1"), None, "t").is_err());
        // A → A11（自己的孙级）应被拒绝
        assert!(move_folder_inner(&conn, "A", Some("A11"), None, "t").is_err());
        // 结构未被破坏
        assert_eq!(parent_of(&conn, "A"), None);
        assert_eq!(parent_of(&conn, "A1").as_deref(), Some("A"));
    }

    #[test]
    fn move_folder_rejects_self_and_duplicate_name() {
        let conn = build_conn();
        insert_folder(&conn, "A", None, "A", 0);
        insert_folder(&conn, "B", None, "B", 1);
        insert_folder(&conn, "B1", Some("B"), "B1", 0);

        // 移到自己
        assert!(move_folder_inner(&conn, "A", Some("A"), None, "t").is_err());
        // B 下已有名为 A 的目录？没有，先造一个
        insert_folder(&conn, "Adup", Some("B"), "A", 0);
        // 根下的 A 移到 B 下会撞名
        assert!(move_folder_inner(&conn, "A", Some("B"), None, "t").is_err());
    }
}
