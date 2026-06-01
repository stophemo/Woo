/**
 * 目录服务（本地版）。无用户隔离，直接操作本地数据。
 */
const { getDb } = require('../db/index.cjs')
const { newId, nowStr } = require('./utils.cjs')

function getFolderTree() {
  const db = getDb()
  const rows = db.prepare(`SELECT id, parent_id AS parentId, name, sort_order AS sortOrder, is_locked AS isLocked
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
      isLocked: r.isLocked === 1,
      children: build(r.id)
    }))
  }
  return build('__root__')
}

function createFolder({ name, parentId }) {
  if (!name) throw new Error('目录名不能为空')
  const db = getDb()
  const pid = parentId || null

  // 同级下是否有同名且未删除的目录
  const existing = db.prepare('SELECT id, deleted FROM note_folder WHERE parent_id IS ? AND name = ?').all(pid, name)
  const active = existing.find(r => r.deleted === 0)
  if (active) throw new Error('同级目录名不能重复')

  // 同级下有同名已软删除的目录 → 恢复它
  const softDeleted = existing.find(r => r.deleted === 1)
  if (softDeleted) {
    const now = nowStr()
    db.prepare('UPDATE note_folder SET deleted = 0, update_time = ? WHERE id = ?').run(now, softDeleted.id)
    return softDeleted.id
  }

  // 新建
  const id = newId()
  const now = nowStr()
  db.prepare(`INSERT INTO note_folder (id, parent_id, name, sort_order, create_time, update_time)
              VALUES (?, ?, ?, ?, ?, ?)`)
    .run(id, pid, name, 0, now, now)
  return id
}

function renameFolder(folderId, newName) {
  const db = getDb()
  const row = db.prepare('SELECT id, parent_id FROM note_folder WHERE id = ? AND deleted = 0').get(folderId)
  if (!row) throw new Error('目录不存在')

  // 同级下是否有其他同名且未删除的目录
  const dup = db.prepare('SELECT id FROM note_folder WHERE parent_id IS ? AND name = ? AND deleted = 0 AND id != ?').get(row.parent_id, newName, folderId)
  if (dup) throw new Error('同级目录名不能重复')

  db.prepare('UPDATE note_folder SET name = ?, update_time = ? WHERE id = ?').run(newName, nowStr(), folderId)
}

function deleteFolder(folderId) {
  const db = getDb()
  const row = db.prepare('SELECT id FROM note_folder WHERE id = ? AND deleted = 0').get(folderId)
  if (!row) throw new Error('目录不存在')

  // 收集所有后代目录（含已废纸篓的，用于完整判断子树文档情况）
  const all = db.prepare('SELECT id, parent_id FROM note_folder WHERE deleted IN (0, 1)').all()
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
    const now = nowStr()
    const placeholders = ids.map(() => '?').join(',')

    // 1. 将目录树下所有活跃文稿移入废纸篓
    db.prepare(
      `UPDATE note_document SET deleted = 1, update_time = ? WHERE folder_id IN (${placeholders}) AND deleted = 0`
    ).run(now, ...ids)

    // 2. 检查子树下是否存在废纸篓文稿
    const { cnt: trashCount } = db.prepare(
      `SELECT COUNT(*) as cnt FROM note_document WHERE folder_id IN (${placeholders}) AND deleted = 1`
    ).get(...ids)

    // 3. 设置目录 deleted 值
    //    子树下有废纸篓文稿 → 目录进入废纸篓（可恢复）
    //    子树为空 → 跳过废纸篓，直接标记永久删除
    const targetDeleted = trashCount > 0 ? 1 : 2
    const upd = db.prepare(
      'UPDATE note_folder SET deleted = ?, update_time = ? WHERE id = ? AND deleted = 0'
    )
    for (const fid of ids) {
      upd.run(targetDeleted, now, fid)
    }
  })
  trx()
}

/**
 * 批量更新同级目录的 sort_order。
 * @param {string|null} parentId - 父目录 id（null 表示根级）
 * @param {Array<{id: string, sortOrder: number}>} items - 排序后的条目数组
 */
function reorderFolders(parentId, items) {
  const db = getDb()
  const stmt = db.prepare('UPDATE note_folder SET sort_order = ?, update_time = ? WHERE id = ?')
  const now = nowStr()
  const trx = db.transaction(() => {
    for (const item of items) {
      stmt.run(item.sortOrder, now, item.id)
    }
  })
  trx()
}

module.exports = { getFolderTree, createFolder, renameFolder, deleteFolder, reorderFolders }
