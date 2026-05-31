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
    create_time TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now')),
    update_time TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now')),
    deleted INTEGER NOT NULL DEFAULT 0,
    is_locked INTEGER NOT NULL DEFAULT 0
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
    create_time TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now')),
    update_time TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now')),
    deleted INTEGER NOT NULL DEFAULT 0,
    is_locked INTEGER NOT NULL DEFAULT 0
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
    create_time TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now')),
    update_time TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now')),
    deleted INTEGER NOT NULL DEFAULT 0,
    UNIQUE(document_id, version_no)
  )`,
  `CREATE INDEX IF NOT EXISTS idx_ver_doc ON note_document_version(document_id)`,

  // 同步元数据表（记录上次同步时间等状态）
  `CREATE TABLE IF NOT EXISTS sync_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
  )`,

  // 知识库分块表（RAG 数据源）
  `CREATE TABLE IF NOT EXISTS kb_chunks (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL,
    document_title TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    content TEXT NOT NULL,
    create_time TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now'))
  )`,
  `CREATE INDEX IF NOT EXISTS idx_kb_doc ON kb_chunks(document_id)`,

  // 知识库 FTS5 全文索引（非 content-sync，手动管理）
  `CREATE VIRTUAL TABLE IF NOT EXISTS kb_chunks_fts USING fts5(
    content, title,
    tokenize='unicode61'
  )`,
]

function initSchema(db) {
  db.pragma('journal_mode = WAL')
  db.pragma('foreign_keys = ON')
  const trx = db.transaction(() => {
    for (const sql of SCHEMA_SQLS) db.prepare(sql).run()
  })
  trx()

  // 迁移：为旧数据库补充缺失的列
  migrate(db)
}

/**
 * 数据库迁移：兼容旧版本缺失的列
 */
function migrate(db) {
  const migrations = [
    // 加锁功能：为旧版数据库补 is_locked 列（CREATE TABLE 已有，仅旧库需要）
    `ALTER TABLE note_folder ADD COLUMN is_locked INTEGER NOT NULL DEFAULT 0`,
    `ALTER TABLE note_document ADD COLUMN is_locked INTEGER NOT NULL DEFAULT 0`,
    // 修复 kb_chunks_fts 列名不匹配（旧版 content-sync 模式 → 独立 FTS5）
    `DROP TABLE IF EXISTS kb_chunks_fts`
  ]
  for (const sql of migrations) {
    try {
      db.prepare(sql).run()
      console.log('[DB] 迁移:', sql)
    } catch (e) {
      // 如果列已存在等，静默跳过
    }
  }
}

module.exports = { initSchema }
