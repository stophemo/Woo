//! 旧 SQLite HTML 数据 → Markdown 文件迁移（桌面端）。
//!
//! 只读 `note_folder` / `note_document`，不修改原数据库；先预览再执行。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use super::safe_path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrateItem {
    pub title: String,
    pub path: String,
    pub status: String,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigratePreview {
    pub total: usize,
    pub will_write: usize,
    pub conflicts: usize,
    pub warnings: usize,
    pub items: Vec<MigrateItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrateReport {
    pub written: usize,
    pub skipped: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

struct PlannedDoc {
    title: String,
    rel: String,
    content: String,
    warning: Option<String>,
}

fn sanitize_name(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    let trimmed = out.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed.to_string()
    }
}

fn folder_path(
    id: &str,
    folders: &HashMap<String, (Option<String>, String)>,
    depth: usize,
) -> String {
    if depth > 64 {
        return String::new();
    }
    match folders.get(id) {
        Some((parent, name)) => {
            let prefix = parent
                .as_deref()
                .map(|value| folder_path(value, folders, depth + 1))
                .unwrap_or_default();
            if prefix.is_empty() {
                sanitize_name(name)
            } else {
                format!("{}/{}", prefix, sanitize_name(name))
            }
        }
        None => String::new(),
    }
}

fn load_folders(conn: &Connection) -> Result<HashMap<String, (Option<String>, String)>, String> {
    let mut statement = conn
        .prepare("SELECT id, parent_id, name FROM note_folder WHERE deleted = 0")
        .map_err(|error| format!("读取文件夹失败：{error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("读取文件夹失败：{error}"))?;
    let mut folders = HashMap::new();
    for row in rows {
        let (id, parent, name) = row.map_err(|error| format!("读取文件夹失败：{error}"))?;
        folders.insert(id, (parent, name));
    }
    Ok(folders)
}

fn plan_documents(conn: &Connection) -> Result<Vec<PlannedDoc>, String> {
    let folders = load_folders(conn)?;
    let mut statement = conn
        .prepare(
            "SELECT folder_id, title, content FROM note_document
             WHERE deleted = 0 ORDER BY folder_id, sort_order, create_time",
        )
        .map_err(|error| format!("读取文稿失败：{error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(|error| format!("读取文稿失败：{error}"))?;

    let mut used = HashSet::new();
    let mut planned = Vec::new();
    for row in rows {
        let (folder_id, title, content) = row.map_err(|error| format!("读取文稿失败：{error}"))?;
        let html = content.unwrap_or_default();
        let dir = folder_path(&folder_id, &folders, 0);
        let base = sanitize_name(&title);
        let mut rel = if dir.is_empty() {
            format!("{base}.md")
        } else {
            format!("{dir}/{base}.md")
        };
        let mut suffix = 2;
        while used.contains(&rel) {
            rel = if dir.is_empty() {
                format!("{base} ({suffix}).md")
            } else {
                format!("{dir}/{base} ({suffix}).md")
            };
            suffix += 1;
        }
        used.insert(rel.clone());

        let (markdown, warning) = match htmd::convert(&html) {
            Ok(value) => (value, None),
            Err(error) => (
                html.clone(),
                Some(format!("HTML 转 Markdown 失败，已保留原 HTML：{error}")),
            ),
        };
        let warning = if html.contains("data:image/") {
            Some(match warning {
                Some(existing) => format!("{existing}；内嵌图片未抽出为附件"),
                None => "内嵌图片未抽出为附件".to_string(),
            })
        } else {
            warning
        };

        planned.push(PlannedDoc {
            title,
            rel,
            content: markdown,
            warning,
        });
    }
    Ok(planned)
}

pub fn preview(conn: &Connection, target: &Path) -> Result<MigratePreview, String> {
    let planned = plan_documents(conn)?;
    let mut items = Vec::with_capacity(planned.len());
    let mut conflicts = 0;
    let mut warnings = 0;
    for doc in &planned {
        let exists = target.join(&doc.rel).exists();
        if exists {
            conflicts += 1;
        }
        if doc.warning.is_some() {
            warnings += 1;
        }
        items.push(MigrateItem {
            title: doc.title.clone(),
            path: doc.rel.clone(),
            status: if exists { "conflict" } else { "new" }.to_string(),
            warning: doc.warning.clone(),
        });
    }
    Ok(MigratePreview {
        total: planned.len(),
        will_write: planned.len().saturating_sub(conflicts),
        conflicts,
        warnings,
        items,
    })
}

pub fn execute(conn: &Connection, target: &Path, overwrite: bool) -> Result<MigrateReport, String> {
    let planned = plan_documents(conn)?;
    fs::create_dir_all(target).map_err(|error| format!("创建迁移目标目录失败：{error}"))?;
    let mut report = MigrateReport {
        written: 0,
        skipped: 0,
        failed: 0,
        errors: Vec::new(),
    };
    for doc in planned {
        let path = safe_path(target, &doc.rel)?;
        if path.exists() && !overwrite {
            report.skipped += 1;
            continue;
        }
        match fs::write(&path, &doc.content) {
            Ok(()) => report.written += 1,
            Err(error) => {
                report.failed += 1;
                report.errors.push(format!("{}：{}", doc.rel, error));
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_unsafe_names() {
        assert_eq!(sanitize_name("a/b:c"), "a_b_c");
        assert_eq!(sanitize_name("  ..  "), "untitled");
    }
}
