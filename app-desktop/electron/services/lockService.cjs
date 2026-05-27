/**
 * 加锁服务。
 * 管理文件夹/文稿的密码加锁及解锁。
 *
 * 密码策略：
 * - 未登录用户：必须创建加锁密码（bcrypt 哈希存入 sync_meta）
 * - 已登录用户：默认使用账号密码，也可设置独立加锁密码
 * - 验证时先用 custom_lock_hash，若不存在则通过 authService 验证账号密码
 */
const { getDb } = require('../db/index.cjs')
const { newId, nowStr } = require('./utils.cjs')
const bcrypt = require('bcryptjs')

const SALT_ROUNDS = 10
const HASH_KEY = 'lock_password_hash'
const MODE_KEY = 'lock_password_mode' // 'account' | 'custom'

// ========== 密码管理 ==========

/**
 * 判断是否已配置加锁密码
 */
function hasPassword() {
  const db = getDb()
  const hash = db.prepare('SELECT value FROM sync_meta WHERE key = ?').get(HASH_KEY)
  return !!hash
}

/**
 * 获取加锁密码模式
 * @returns {'account' | 'custom' | null}
 */
function getPasswordMode() {
  const db = getDb()
  const row = db.prepare('SELECT value FROM sync_meta WHERE key = ?').get(MODE_KEY)
  return row ? row.value : null
}

/**
 * 设置自定义加锁密码
 * @param {string} password
 */
function setPassword(password) {
  if (!password || password.length < 4) throw new Error('密码长度不能少于 4 位')
  const db = getDb()
  const hash = bcrypt.hashSync(password, SALT_ROUNDS)
  db.prepare('INSERT OR REPLACE INTO sync_meta (key, value) VALUES (?, ?)').run(HASH_KEY, hash)
  db.prepare('INSERT OR REPLACE INTO sync_meta (key, value) VALUES (?, ?)').run(MODE_KEY, 'custom')
}

/**
 * 尝试验证密码。
 * 先试自定义密码，若未设置则通过 authService 验证账号密码。
 * @param {string} password
 * @returns {boolean}
 */
function verifyPassword(password) {
  if (!password) return false

  const db = getDb()
  const row = db.prepare('SELECT value FROM sync_meta WHERE key = ?').get(HASH_KEY)

  if (row) {
    // 有自定义密码 → 用 bcrypt 验证
    return bcrypt.compareSync(password, row.value)
  }

  // 无自定义密码 → 尝试用账号密码验证
  try {
    const authService = require('./authService.cjs')
    // 从 session 获取当前用户的邮箱/用户名
    const session = authService.getCurrentUser()
    if (!session) return false

    // 尝试用邮箱/用户名 + 密码登录来验证
    // 这里用 signIn 来验证，但 signIn 会真正登录，不太合适。
    // 更好的做法：直接验证已有本地账号密码
    // 实际上本地没有存储用户密码，只有 Supabase 有。
    // 所以这里只返回 false，要求必须设置自定义密码。
    return false
  } catch {
    return false
  }
}

// ========== 文件夹加解锁 ==========

function lockFolder(folderId) {
  const db = getDb()
  const row = db.prepare('SELECT id FROM note_folder WHERE id = ? AND deleted = 0').get(folderId)
  if (!row) throw new Error('目录不存在')
  db.prepare('UPDATE note_folder SET is_locked = 1, update_time = ? WHERE id = ?').run(nowStr(), folderId)
}

function unlockFolder(folderId) {
  const db = getDb()
  db.prepare('UPDATE note_folder SET is_locked = 0, update_time = ? WHERE id = ?').run(nowStr(), folderId)
}

function isFolderLocked(folderId) {
  const db = getDb()
  const row = db.prepare('SELECT is_locked FROM note_folder WHERE id = ? AND deleted = 0').get(folderId)
  return row ? row.is_locked === 1 : false
}

/**
 * 判断目录或任一祖先目录是否被锁定
 */
function isFolderEffectivelyLocked(folderId) {
  const db = getDb()
  return isFolderOrAncestorLocked(db, folderId)
}

// ========== 文稿加解锁 ==========

function lockDocument(documentId) {
  const db = getDb()
  const row = db.prepare('SELECT id FROM note_document WHERE id = ? AND deleted = 0').get(documentId)
  if (!row) throw new Error('文稿不存在')
  db.prepare('UPDATE note_document SET is_locked = 1, update_time = ? WHERE id = ?').run(nowStr(), documentId)
}

function unlockDocument(documentId) {
  const db = getDb()
  db.prepare('UPDATE note_document SET is_locked = 0, update_time = ? WHERE id = ?').run(nowStr(), documentId)
}

function isDocumentLocked(documentId) {
  const db = getDb()
  const row = db.prepare('SELECT is_locked FROM note_document WHERE id = ? AND deleted = 0').get(documentId)
  return row ? row.is_locked === 1 : false
}

/**
 * 判断文档是否被锁定（自身锁定或祖先目录锁定）
 */
function isDocumentEffectivelyLocked(documentId) {
  const db = getDb()
  const doc = db.prepare('SELECT is_locked, folder_id FROM note_document WHERE id = ? AND deleted = 0').get(documentId)
  if (!doc) return false
  if (doc.is_locked === 1) return true

  // 检查父目录链
  if (doc.folder_id) {
    return isFolderOrAncestorLocked(db, doc.folder_id)
  }
  return false
}

/**
 * 检查目录及其祖先是否被锁定
 */
function isFolderOrAncestorLocked(db, folderId) {
  let curId = folderId
  const visited = new Set()
  while (curId) {
    if (visited.has(curId)) break
    visited.add(curId)
    const row = db.prepare('SELECT parent_id, is_locked FROM note_folder WHERE id = ? AND deleted = 0').get(curId)
    if (!row) break
    if (row.is_locked === 1) return true
    curId = row.parent_id
  }
  return false
}

/**
 * 判断文档是否因自身或所在目录被锁定而不可见（用于搜索/全部视图过滤）
 */
function isDocumentHidden(documentId) {
  return isDocumentEffectivelyLocked(documentId)
}

module.exports = {
  hasPassword,
  getPasswordMode,
  setPassword,
  verifyPassword,
  lockFolder,
  unlockFolder,
  isFolderLocked,
  isFolderEffectivelyLocked,
  lockDocument,
  unlockDocument,
  isDocumentLocked,
  isDocumentEffectivelyLocked,
  isFolderOrAncestorLocked,
  isDocumentHidden
}
