/**
 * SQLite 建表脚本（本地版）。
 * 纯本地应用，无需用户表，数据直接按本地唯一实例存储。
 * 表：note_folder / note_document / note_document_version
 */

const SCHEMA_SQLS = [
  // 目录表
  `CREATE TABLE IF NOT EXISTS note_folder (
    id TEXT PRIMARY KEY,
    parent_id TEXT,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    create_time TEXT NOT NULL DEFAULT (datetime('now','localtime')),
    update_time TEXT NOT NULL DEFAULT (datetime('now','localtime')),
    deleted INTEGER NOT NULL DEFAULT 0
  )`,
  `CREATE INDEX IF NOT EXISTS idx_folder_parent ON note_folder(parent_id)`,

  // 文稿表
  `CREATE TABLE IF NOT EXISTS note_document (
    id TEXT PRIMARY KEY,
    folder_id TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT,
    branch_name TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    create_time TEXT NOT NULL DEFAULT (datetime('now','localtime')),
    update_time TEXT NOT NULL DEFAULT (datetime('now','localtime')),
    deleted INTEGER NOT NULL DEFAULT 0
  )`,
  `CREATE INDEX IF NOT EXISTS idx_doc_folder ON note_document(folder_id)`,

  // 文稿版本表
  `CREATE TABLE IF NOT EXISTS note_document_version (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL,
    version_no INTEGER NOT NULL,
    title TEXT,
    content TEXT,
    content_hash TEXT,
    change_type TEXT NOT NULL DEFAULT 'auto',
    operator_id TEXT,
    create_time TEXT NOT NULL DEFAULT (datetime('now','localtime')),
    UNIQUE(document_id, version_no)
  )`,
  `CREATE INDEX IF NOT EXISTS idx_ver_doc ON note_document_version(document_id)`
]

function initSchema(db) {
  db.pragma('journal_mode = WAL')
  db.pragma('foreign_keys = ON')
  const trx = db.transaction(() => {
    for (const sql of SCHEMA_SQLS) db.prepare(sql).run()
  })
  trx()
}

module.exports = { initSchema }
