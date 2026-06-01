/**
 * SQLite 连接管理。
 *
 * 文件名策略：
 * - 未登录 → userData/woo.db（纯本地数据）
 * - 已登录 → userData/woo-{用户名}.db（当前用户专属数据库）
 * 切换用户时自动断开旧连接、打开新文件，互不干扰。
 */
const path = require('path')
const fs = require('fs')
const { app } = require('electron')
const Database = require('better-sqlite3')
const { initSchema } = require('./schema.cjs')
const { getLoadablePath } = require('sqlite-vec')

/**
 * 加载 sqlite-vec 向量搜索扩展。
 * db.loadExtension() 底层调用 dlopen()，不识别 Electron asar 虚拟路径。
 * 对于 asarUnpacked 的文件，需将 asar 虚拟路径替换为实际文件系统路径。
 */
function loadVecExtension(db) {
  try {
    const originalPath = getLoadablePath()
    const loadPath = originalPath.replace(/\.asar(\/|\\)/, '.asar.unpacked$1')
    db.loadExtension(loadPath)
  } catch (e) {
    console.warn('[DB] sqlite-vec 加载失败:', e.message)
  }
}

let db = null
let currentDbPath = null
let currentUsername = null

function ensureDir(dir) {
  if (!fs.existsSync(dir)) {
    try {
      fs.mkdirSync(dir, { recursive: true })
    } catch (e) {
      const fallbackDir = path.join(app.getPath('appData'), 'woo-local')
      if (!fs.existsSync(fallbackDir)) fs.mkdirSync(fallbackDir, { recursive: true })
      return fallbackDir
    }
  }
  return dir
}

function getSafeFilename(name) {
  // macOS 文件系统大小写不敏感，统一转小写防冲突
  return name.toLowerCase().replace(/[^a-z0-9_-]/g, '_')
}

function getDbFilename() {
  const dir = ensureDir(app.getPath('userData'))
  if (currentUsername) {
    return path.join(dir, `woo-${getSafeFilename(currentUsername)}.db`)
  }
  return path.join(dir, 'woo.db')
}

/**
 * 切换到指定用户的数据库文件。
 * @param {string|null} username - 用户名（null = 本地模式）
 */
function setCurrentUser(username) {
  if (currentUsername === username) return
  closeDb()
  currentUsername = username
  currentDbPath = null
  console.log('[DB] 切换到用户数据库:', username || '本地')
}

/**
 * 获取当前数据库连接。
 * 如果用户已切换（dbPath 变化），自动断开旧连接并打开新文件。
 */
function getDb() {
  const targetPath = getDbFilename()

  // 同一文件，复用已有连接
  if (db && currentDbPath === targetPath) return db

  // 文件变了 → 关闭旧连接
  if (db) {
    try { db.close() } catch (_) { /* ignore */ }
    db = null
  }

  console.log('[DB] 打开数据库:', targetPath)
  db = new Database(targetPath)
  currentDbPath = targetPath
  // 加载 sqlite-vec 向量搜索扩展
  loadVecExtension(db)
  initSchema(db)
  return db
}

function closeDb() {
  if (db) {
    try { db.close() } catch (_) { /* ignore */ }
    db = null
  }
}

module.exports = { getDb, closeDb, setCurrentUser }
