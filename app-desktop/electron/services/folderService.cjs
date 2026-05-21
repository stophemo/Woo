/**
 * 目录服务（本地版）。无用户隔离，直接操作本地数据。
 */
const { getDb } = require('../db/index.cjs')
const { newId, nowStr } = require('./utils.cjs')

function getFolderTree() {
  const db = getDb()
  const rows = db.prepare(`SELECT id, parent_id AS parentId, name, sort_order AS sortOrder
                           FROM note_folder WHERE deleted = 0
                           ORDER BY sort_order ASC, create_time ASC`).all()
  const byParent = new Map()
  for (const r of rows) {
    const key = r.parentId || '__root__'
    if (!byParent.has(key)) byParent.set(key, [])
    byParent.get(key).push(r)
  }
  const build = (parentKey) => {
    const list = byParent.get(parentKey) || []
    return list.map(r => ({
      id: r.id,
      parentId: r.parentId || null,
      name: r.name,
      sortOrder: r.sortOrder,
      children: build(r.id)
    }))
  }
  return build('__root__')
}

function createFolder({ name, parentId }) {
  if (!name) throw new Error('目录名不能为空')
  const db = getDb()
  const id = newId()
  db.prepare(`INSERT INTO note_folder (id, parent_id, name, sort_order)
              VALUES (?, ?, ?, 0)`)
    .run(id, parentId || null, name)
  return id
}

function renameFolder(folderId, newName) {
  const db = getDb()
  const row = db.prepare('SELECT id FROM note_folder WHERE id = ? AND deleted = 0').get(folderId)
  if (!row) throw new Error('目录不存在')
  db.prepare('UPDATE note_folder SET name = ?, update_time = ? WHERE id = ?').run(newName, nowStr(), folderId)
}

function deleteFolder(folderId) {
  const db = getDb()
  const row = db.prepare('SELECT id FROM note_folder WHERE id = ? AND deleted = 0').get(folderId)
  if (!row) throw new Error('目录不存在')
  // 级联删除：收集所有后代
  const all = db.prepare('SELECT id, parent_id FROM note_folder WHERE deleted = 0').all()
  const childrenMap = new Map()
  for (const r of all) {
    const key = r.parent_id || '__root__'
    if (!childrenMap.has(key)) childrenMap.set(key, [])
    childrenMap.get(key).push(r.id)
  }
  const ids = []
  const walk = (fid) => {
    ids.push(fid)
    for (const c of (childrenMap.get(fid) || [])) walk(c)
  }
  walk(folderId)

  const trx = db.transaction(() => {
    const upd = db.prepare('UPDATE note_folder SET deleted = 1 WHERE id = ?')
    const updDoc = db.prepare('UPDATE note_document SET deleted = 1 WHERE folder_id = ?')
    for (const fid of ids) {
      upd.run(fid)
      updDoc.run(fid)
    }
  })
  trx()
}

module.exports = { getFolderTree, createFolder, renameFolder, deleteFolder }
