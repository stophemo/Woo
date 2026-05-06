/**
 * SQLite 连接单例。数据库文件位于 userData/woo.db。
 */
const path = require('path')
const fs = require('fs')
const { app } = require('electron')
const Database = require('better-sqlite3')
const { initSchema } = require('./schema.cjs')

let db = null

function getDb() {
  if (db) return db
  const dir = app.getPath('userData')
  // 确保目录存在（recursive 会创建完整路径链）
  if (!fs.existsSync(dir)) {
    try {
      fs.mkdirSync(dir, { recursive: true })
    } catch (e) {
      // 如果 userData 目录创建失败，尝试使用 appData + 自定义目录名
      const fallbackDir = path.join(app.getPath('appData'), 'woo-local')
      if (!fs.existsSync(fallbackDir)) fs.mkdirSync(fallbackDir, { recursive: true })
      const file = path.join(fallbackDir, 'woo.db')
      db = new Database(file)
      initSchema(db)
      return db
    }
  }
  const file = path.join(dir, 'woo.db')
  db = new Database(file)
  initSchema(db)
  return db
}

function closeDb() {
  if (db) {
    try { db.close() } catch (_) { /* ignore */ }
    db = null
  }
}

module.exports = { getDb, closeDb }
