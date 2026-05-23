/**
 * 文稿服务（本地版）。无用户隔离。
 * - updateContent 时自动从首行提取 title
 * - 生成 Git 分支名（目录路径-标题，非法字符替换）
 */
const { newId, stripHtmlKeepLines, nowStr } = require('./utils.cjs')
const { getDb } = require('../db/index.cjs')

function toDto(row) {
  if (!row) return null
  return {
    id: row.id,
    folderId: row.folder_id,
    title: row.title,
    content: row.content || '',
    branchName: row.branch_name || undefined,
    sortOrder: row.sort_order,
    createTime: row.create_time,
    updateTime: row.update_time
  }
}

function listByFolder(folderId) {
  const db = getDb()
  const rows = db.prepare(`SELECT d.*, f.name AS folder_name FROM note_document d
                           LEFT JOIN note_folder f ON d.folder_id = f.id
                           WHERE d.folder_id = ? AND d.deleted = 0
                           ORDER BY d.update_time DESC`).all(folderId)
  return rows.map(r => ({ ...toDto(r), folderName: r.folder_name || '' }))
}

function listAll() {
  const db = getDb()
  const rows = db.prepare(`SELECT d.*, f.name AS folder_name FROM note_document d
                           LEFT JOIN note_folder f ON d.folder_id = f.id
                           WHERE d.deleted = 0
                           ORDER BY d.update_time DESC`).all()
  return rows.map(r => ({ ...toDto(r), folderName: r.folder_name || '' }))
}

function listOrphans() {
  const db = getDb()
  const rows = db.prepare(`SELECT d.* FROM note_document d
                           WHERE d.deleted = 0 AND (
                             d.folder_id IS NULL OR d.folder_id = '' OR
                             NOT EXISTS (SELECT 1 FROM note_folder f WHERE f.id = d.folder_id AND f.deleted = 0)
                           )
                           ORDER BY d.update_time DESC`).all()
  return rows.map(toDto)
}

function listTrash() {
  const db = getDb()
  const rows = db.prepare(`SELECT * FROM note_document
                           WHERE deleted = 1
                           ORDER BY update_time DESC`).all()
  return rows.map(toDto)
}

function search(keyword) {
  const db = getDb()
  const q = String(keyword || '').trim()
  if (!q) return []
  const like = `%${q}%`
  const rows = db.prepare(`SELECT * FROM note_document
                           WHERE deleted = 0 AND (title LIKE ? OR content LIKE ?)
                           ORDER BY update_time DESC`).all(like, like)
  return rows.map(toDto)
}

function getById(documentId) {
  const row = verifyExists(documentId)
  return toDto(row)
}

function create({ title, folderId }) {
  if (!title) throw new Error('标题不能为空')
  if (!folderId) throw new Error('目录不能为空')
  const db = getDb()
  const folder = db.prepare('SELECT * FROM note_folder WHERE id = ? AND deleted = 0').get(folderId)
  if (!folder) throw new Error('目录不存在')

  const branchName = generateBranchName(folder, title)
  const id = newId()
  const now = nowStr()
  db.prepare(`INSERT INTO note_document (id, folder_id, title, content, branch_name, sort_order, create_time, update_time)
              VALUES (?, ?, ?, '', ?, 0, ?, ?)`)
    .run(id, folderId, title, branchName, now, now)
  const row = db.prepare('SELECT * FROM note_document WHERE id = ?').get(id)
  return toDto(row)
}

function rename(documentId, title) {
  verifyExists(documentId)
  const db = getDb()
  db.prepare('UPDATE note_document SET title = ?, update_time = ? WHERE id = ?').run(title, nowStr(), documentId)
}

function updateContent(documentId, content) {
  verifyExists(documentId)
  const db = getDb()
  const title = extractFirstLineAsTitle(content)
  db.prepare('UPDATE note_document SET content = ?, title = ?, update_time = ? WHERE id = ?')
    .run(content || '', title, nowStr(), documentId)
}

function remove(documentId) {
  verifyExists(documentId)
  const db = getDb()
  db.prepare('UPDATE note_document SET deleted = 1, update_time = ? WHERE id = ?').run(nowStr(), documentId)
}

function restore(documentId) {
  const db = getDb()
  const doc = db.prepare('SELECT * FROM note_document WHERE id = ?').get(documentId)
  if (!doc) throw new Error('文稿不存在')

  const now = nowStr()

  // 递归恢复目录链（父目录软删除时一并恢复）
  function restoreFolderChain(folderId) {
    if (!folderId) return
    const folder = db.prepare('SELECT * FROM note_folder WHERE id = ?').get(folderId)
    if (!folder) return // 目录已被真删，无法恢复
    if (folder.deleted === 0) return // 目录已存在

    // 先恢复父目录
    restoreFolderChain(folder.parent_id)
    // 再恢复当前目录
    db.prepare('UPDATE note_folder SET deleted = 0, update_time = ? WHERE id = ?').run(now, folderId)
  }

  restoreFolderChain(doc.folder_id)

  // 恢复文档
  db.prepare('UPDATE note_document SET deleted = 0, update_time = ? WHERE id = ?').run(now, documentId)
}

function logDelete(tableName, recordIds) {
  if (!recordIds || recordIds.length === 0) return
  const db = getDb()
  const now = nowStr()
  const stmt = db.prepare('INSERT OR IGNORE INTO sync_delete_log (id, table_name, record_id, create_time) VALUES (?, ?, ?, ?)')
  const trx = db.transaction(() => {
    for (const rid of recordIds) {
      stmt.run(newId(), tableName, rid, now)
    }
  })
  trx()
}

function hardDelete(documentId) {
  const db = getDb()
  const doc = db.prepare('SELECT * FROM note_document WHERE id = ? AND deleted = 1').get(documentId)
  if (!doc) throw new Error('文稿不存在或未在回收站中')
  logDelete('note_document', [documentId])
  db.prepare('DELETE FROM note_document_version WHERE document_id = ?').run(documentId)
  db.prepare('DELETE FROM note_document WHERE id = ?').run(documentId)
}

function emptyTrash() {
  const db = getDb()
  // 先收集所有待真删的文档 ID（用于同步删除日志）
  const toDelete = db.prepare('SELECT id FROM note_document WHERE deleted = 1').all()
  if (toDelete.length > 0) {
    logDelete('note_document', toDelete.map(r => r.id))
  }
  db.prepare(`DELETE FROM note_document_version
               WHERE document_id IN (SELECT id FROM note_document WHERE deleted = 1)`).run()
  db.prepare('DELETE FROM note_document WHERE deleted = 1').run()
}

function verifyExists(documentId) {
  const db = getDb()
  const row = db.prepare('SELECT * FROM note_document WHERE id = ? AND deleted = 0').get(documentId)
  if (!row) throw new Error('文稿不存在')
  return row
}

function extractFirstLineAsTitle(content) {
  if (!content) return '新文稿'
  const plain = stripHtmlKeepLines(content)
  for (const line of plain.split(/\r?\n/)) {
    const t = line.trim()
    if (t) return t.length > 40 ? t.substring(0, 40) + '…' : t
  }
  return '新文稿'
}

function generateBranchName(folder, title) {
  const db = getDb()
  const parts = []
  let cur = folder
  while (cur) {
    parts.unshift(sanitize(cur.name))
    if (!cur.parent_id) break
    cur = db.prepare('SELECT * FROM note_folder WHERE id = ?').get(cur.parent_id)
  }
  parts.push(sanitize(title))
  return parts.join('-')
}

function sanitize(seg) {
  return String(seg || '').replace(/[\s/\\:*?"<>|]/g, '_')
}

module.exports = {
  listByFolder,
  listAll,
  listTrash,
  listOrphans,
  search,
  getById,
  create,
  rename,
  updateContent,
  remove,
  restore,
  hardDelete,
  emptyTrash,
  verifyExists
}
