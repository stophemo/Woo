/**
 * 文稿服务（本地版）。无用户隔离。
 * - updateContent 时自动从首行提取 title
 * - 生成 Git 分支名（目录路径-标题，非法字符替换）
 */
const { getDb } = require('../db/index.cjs')
const { newId, stripHtmlKeepLines, nowStr } = require('./utils.cjs')

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
  const rows = db.prepare(`SELECT * FROM note_document
                           WHERE folder_id = ? AND deleted = 0
                           ORDER BY update_time DESC`).all(folderId)
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
  db.prepare(`INSERT INTO note_document (id, folder_id, title, content, branch_name, sort_order)
              VALUES (?, ?, ?, '', ?, 0)`)
    .run(id, folderId, title, branchName)
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
  getById,
  create,
  rename,
  updateContent,
  remove,
  verifyExists
}
