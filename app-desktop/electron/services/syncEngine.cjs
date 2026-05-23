/**
 * 同步引擎。
 *
 * 设计原则（零预算友好）：
 * - 本地 SQLite 是主数据源，Supabase 是云端副本
 * - 时间戳冲突策略：最后写入者胜出（last-write-wins）
 * - 增量同步：仅推送/拉取 update_time 距上次同步之后变更的记录
 * - 软删除：所有删除标记 deleted=1，不做硬删除
 *
 * 同步流程（每次触发）：
 *   1. PULL 先拉：读取云端 update_time > last_sync_time 的记录 → 合并到本地
 *      （远程新于本地 → 覆盖本地；本地新于远程 → 保持本地）
 *   2. PUSH 后推：读取本地 update_time > last_sync_time 的记录 → upsert 到云端
 *      （先拉后推，确保推送的是合并后的最新结果）
 *   3. 更新 last_sync_time
 *
 * 注意：LWW 策略下，同一篇文档在多设备同时编辑不同字段时，
 *      后保存的设备会整体覆盖先保存的内容。个人笔记场景可接受此限制。
 */
const { getDb } = require('../db/index.cjs')
const { getClient } = require('../config/supabase.cjs')
const fs = require('fs')
const path = require('path')
const { app } = require('electron')

const ENTITIES = [
  { table: 'note_folder', idField: 'id' },
  { table: 'note_document', idField: 'id' },
  { table: 'note_document_version', idField: 'id' }
]

// 同步状态
let isSyncing = false
let currentSyncPromise = null
let lastSyncTime = null
let syncTimer = null
let authUnsubscribe = null

// 缓存当前 session（由 auth state listener 实时更新）
let currentSession = null

// 回调：通知渲染进程同步状态更新
let onStatusChange = null

// 回调：同步完成后通知渲染进程刷新数据
let onDataChange = null

/**
 * 获取或创建 sync_meta 表
 */
function ensureSyncMetaTable() {
  const db = getDb()
  db.exec(`
    CREATE TABLE IF NOT EXISTS sync_meta (
      key TEXT PRIMARY KEY,
      value TEXT NOT NULL
    )
  `)
}

function getSyncMeta(key) {
  const db = getDb()
  const row = db.prepare('SELECT value FROM sync_meta WHERE key = ?').get(key)
  return row ? row.value : null
}

function setSyncMeta(key, value) {
  const db = getDb()
  db.prepare('INSERT OR REPLACE INTO sync_meta (key, value) VALUES (?, ?)').run(key, value)
}

/**
 * 获取 last_sync_time，返回 ISO 字符串或 null
 */
function getLastSyncTime() {
  return getSyncMeta('last_sync_time')
}

function setLastSyncTime(time) {
  setSyncMeta('last_sync_time', time)
  lastSyncTime = time
}

// ============== 推送逻辑 ==============

/**
 * 将本地变更推送到 Supabase
 * 顺序：先处理删除日志 → 再推送 upsert
 */
async function pushChanges(userId, lastSync) {
  const client = getClient()
  if (!client) return { ok: false, message: 'Supabase 未配置' }

  let pushedCount = 0
  let deletedCount = 0
  const errors = []

  // === 第一步：处理本地硬删除（删除日志）===
  // 必须在 upsert 之前处理，否则 PULL 拉回的云端数据又会被 upsert 回去
  const deleteResult = await pushDeleteLog(client, userId)
  deletedCount = deleteResult.deletedCount
  errors.push(...deleteResult.errors)

  // === 第二步：推送常规变更 upsert ===
  for (const entity of ENTITIES) {
    try {
      const localRows = queryLocalChanges(entity.table, entity.idField, lastSync)
      if (localRows.length === 0) continue

      // 为每行记录添加 user_id
      const rowsWithUser = localRows.map(row => ({
        ...row,
        user_id: userId
      }))

      // Supabase upsert（以 id 为主键冲突更新）
      const { error } = await client
        .from(entity.table)
        .upsert(rowsWithUser, { onConflict: 'id', ignoreDuplicates: false })

      if (error) {
        errors.push(`[${entity.table}] upsert 失败: ${error.message}`)
      } else {
        pushedCount += localRows.length
      }
    } catch (err) {
      errors.push(`[${entity.table}] 推送异常: ${err.message}`)
    }
  }

  return { ok: errors.length === 0, pushedCount, deletedCount, errors }
}

/**
 * 处理同步删除日志：在云端执行对应记录的删除
 * 包含级联删除（如删除 note_document 时同时删 note_document_version）
 */
async function pushDeleteLog(client, userId) {
  const db = getDb()
  const logs = db.prepare('SELECT * FROM sync_delete_log ORDER BY create_time ASC').all()
  if (logs.length === 0) return { deletedCount: 0, errors: [] }

  let deletedCount = 0
  const errors = []
  const processedIds = []

  for (const log of logs) {
    try {
      const { error } = await client
        .from(log.table_name)
        .delete()
        .eq('id', log.record_id)
        .eq('user_id', userId)

      if (error) {
        errors.push(`[${log.table_name}] 云端删除失败(${log.record_id}): ${error.message}`)
        continue
      }

      // 级联删除：删文档时同时删云端版本记录
      if (log.table_name === 'note_document') {
        const { error: verErr } = await client
          .from('note_document_version')
          .delete()
          .eq('document_id', log.record_id)
        if (verErr) {
          errors.push(`[note_document_version] 级联删除失败(${log.record_id}): ${verErr.message}`)
        }
      }

      deletedCount++
      processedIds.push(log.id)
    } catch (err) {
      errors.push(`[${log.table_name}] 云端删除异常(${log.record_id}): ${err.message}`)
    }
  }

  // 清理已处理的日志
  if (processedIds.length > 0) {
    const placeholders = processedIds.map(() => '?').join(',')
    db.prepare(`DELETE FROM sync_delete_log WHERE id IN (${placeholders})`).run(...processedIds)
    console.log('[Sync] 已清理删除日志:', processedIds.length, '条')
  }

  return { deletedCount, errors }
}

/**
 * 查询本地自 lastSync 之后变更的记录
 */
function queryLocalChanges(table, idField, lastSync) {
  const db = getDb()
  try {
    const stmt = db.prepare(
      `SELECT * FROM ${table} WHERE update_time > ? ORDER BY update_time ASC`
    )
    return stmt.all(lastSync || '1970-01-01')
  } catch {
    // 表缺少 update_time 列，返回空数组
    return []
  }
}

// ============== 拉取逻辑 ==============

/**
 * 从 Supabase 拉取远程变更并合并到本地
 */
async function pullChanges(userId, lastSync) {
  const client = getClient()
  if (!client) return { ok: false, message: 'Supabase 未配置' }

  let pulledCount = 0
  let conflictCount = 0
  const errors = []

  for (const entity of ENTITIES) {
    try {
      const remoteRows = await queryRemoteChanges(client, entity.table, userId, lastSync)
      if (remoteRows.length === 0) continue

      const result = mergeIntoLocal(entity.table, entity.idField, remoteRows)
      pulledCount += result.pulled
      conflictCount += result.conflicts
    } catch (err) {
      errors.push(`[${entity.table}] 拉取异常: ${err.message}`)
    }
  }

  return { ok: errors.length === 0, pulledCount, conflictCount, errors }
}

/**
 * 从 Supabase 查询远程变更
 */
async function queryRemoteChanges(client, table, userId, lastSync) {
  const query = client
    .from(table)
    .select('*')
    .eq('user_id', userId)
    .order('update_time', { ascending: true })

  if (lastSync) {
    query.gt('update_time', lastSync)
  }

  const { data, error } = await query
  if (error) throw new Error(error.message)
  return data || []
}

/**
 * 将远程记录合并到本地 SQLite
 * 冲突策略：本地 update_time ≥ 远程 update_time → 保留本地；否则覆盖
 *
 * 注意：远程数据含有 user_id 列，但本地 schema 不含，需剥离。
 * 本地每个数据库文件已对应单一用户，无需冗余存储 user_id。
 */
function mergeIntoLocal(table, idField, remoteRows) {
  const db = getDb()
  let pulled = 0
  let conflicts = 0

  const checkStmt = db.prepare(
    `SELECT update_time FROM ${table} WHERE ${idField} = ?`
  )

  // 剥离远端数据中的 user_id（本地 schema 不含此列）
  const localRows = remoteRows.map(row => {
    const { user_id, ...localRow } = row
    return localRow
  })

  if (localRows.length === 0) return { pulled, conflicts }

  const upsertStmt = buildUpsertStmt(db, table, localRows[0])

  for (const row of localRows) {
    const local = checkStmt.get(row[idField])

    if (local) {
      // 本地存在：比较时间戳
      if (local.update_time >= row.update_time) {
        conflicts++
        continue // 本地版本更新，跳过
      }
    }

    // 本地不存在或本地版本更旧 → 覆盖/插入
    upsertStmt.run(Object.values(row))
    pulled++
  }

  return { pulled, conflicts }
}

/**
 * 动态构建 upsert prepared statement
 */
function buildUpsertStmt(db, table, sampleRow) {
  if (!sampleRow) return null
  const keys = Object.keys(sampleRow)
  const placeholders = keys.map(() => '?').join(', ')
  const updateSet = keys.map(k => `${k} = ?`).join(', ')

  // 使用 REPLACE 实现 upsert
  // 注意：REPLACE 会先 DELETE 再 INSERT，这里因为我们有完整数据，效果等同于 upsert
  const sql = `INSERT OR REPLACE INTO ${table} (${keys.join(', ')}) VALUES (${placeholders})`
  return db.prepare(sql)
}

// ============== 对外接口 ==============

/**
 * 执行一次完整的同步（push + pull）
 */
async function syncNow() {
  // 如果已有同步在进行，等待它完成
  if (isSyncing && currentSyncPromise) {
    try { await currentSyncPromise } catch { /* ignore */ }
    if (!isSyncing) return { waitReused: true, syncTime: lastSyncTime }
  }

  // 创建同步 Promise 并保存，供后续并发调用等待
  const promise = doSync()
  currentSyncPromise = promise
  return promise
}

/**
 * 实际执行同步的内部方法
 */
async function doSync() {
  // 优先使用缓存的 session，其次调用 getSession
  const session = currentSession || await getSession()
  if (!session?.user) throw new Error('未登录')

  isSyncing = true
  emitStatus()

  try {
    const userId = session.user.id

    // === 0. 先处理删除日志（在 PULL 之前，避免云端数据被拉回）===
    const client = getClient()
    if (client) {
      await pushDeleteLog(client, userId)
    }

    const lastSync = getLastSyncTime()
    console.log('[Sync] 开始同步, lastSync:', lastSync)

    // === 1. PULL 先拉 ===
    // 先拉取云端变更合并到本地，确保后续推送的是合并后的最新结果
    const pullResult = await pullChanges(userId, lastSync)
    console.log('[Sync] 拉取完成:', pullResult)

    // === 2. PUSH 后推 ===
    // 推送本地变更到云端（此时本地已包含拉取的最新数据）
    const pushResult = await pushChanges(userId, lastSync)
    console.log('[Sync] 推送完成:', pushResult)

    // 3. 更新同步时间戳
    const now = new Date().toISOString()
    setLastSyncTime(now)

    return {
      pushedCount: pushResult.pushedCount || 0,
      pulledCount: pullResult.pulledCount || 0,
      conflictCount: pullResult.conflictCount || 0,
      syncTime: now
    }
  } catch (err) {
    console.error('[Sync] 同步失败:', err)
    throw err
  } finally {
    isSyncing = false
    currentSyncPromise = null
    emitStatus()
    if (onDataChange) onDataChange()
  }
}

/**
 * 执行同步（含并发保护，返回 Promise）
 */
async function runSync() {
  if (isSyncing) {
    if (currentSyncPromise) return currentSyncPromise
  }
  currentSyncPromise = syncNow()
  return currentSyncPromise
}

/**
 * 获取当前同步状态
 */
function getStatus() {
  return {
    isSyncing,
    lastSyncTime: lastSyncTime || getLastSyncTime(),
    pendingChanges: countPendingChanges()
  }
}

/**
 * 计算待同步变更数
 */
function countPendingChanges() {
  const lastSync = getLastSyncTime()
  if (!lastSync) return -1 // 首次同步，尚未确定

  const db = getDb()
  let total = 0
  for (const entity of ENTITIES) {
    try {
      const row = db.prepare(
        `SELECT COUNT(*) as cnt FROM ${entity.table} WHERE update_time > ?`
      ).get(lastSync)
      total += row?.cnt || 0
    } catch {
      // 表缺少 update_time 列时跳过（旧数据库迁移滞后）
      continue
    }
  }
  return total
}

function emitStatus() {
  if (onStatusChange) {
    onStatusChange(getStatus())
  }
}

/**
 * 设置状态变更回调（给 main.cjs 注册，用于向渲染进程推送）
 */
function setOnStatusChange(callback) {
  onStatusChange = callback
}

function setOnDataChange(callback) {
  onDataChange = callback
}

async function getSession() {
  const { getClient } = require('../config/supabase.cjs')
  const client = getClient()
  if (!client) return {}
  const { data } = await client.auth.getSession()
  return data?.session || {}
}

// ============== 启动/停止 ==============

/**
 * 启动同步引擎（含定时器）
 * @param {number} intervalMs - 同步间隔，默认 60 秒
 */
function start(intervalMs = 60000) {
  ensureSyncMetaTable()

  // 读取持久化的 last_sync_time
  lastSyncTime = getLastSyncTime()

  console.log('[Sync] 引擎启动, 同步间隔:', intervalMs, 'ms')

  // 注册认证状态监听：登录/登出时切换用户数据库，然后触发同步
  const authService = require('./authService.cjs')
  const { setCurrentUser } = require('../db/index.cjs')
  authUnsubscribe = authService.onAuthStateChange(async (event, session) => {
    // 缓存最新 session（后续 syncNow 直接复用，避免 getSession() 在 Node 环境异常）
    currentSession = session
    if (event === 'SIGNED_IN' || event === 'TOKEN_REFRESHED') {
      const userId = session?.user?.id
      const username = session?.user?.user_metadata?.username
      if (!userId || !username) return

      const { closeDb, setCurrentUser } = require('../db/index.cjs')

      // 首次登录：将 woo.db 复制为 woo-{用户名}.db
      const dbDir = app.getPath('userData')
      const safeName = username.toLowerCase().replace(/[^a-z0-9_-]/g, '_')
      const localDb = path.join(dbDir, 'woo.db')
      const userDb = path.join(dbDir, `woo-${safeName}.db`)
      if (fs.existsSync(localDb) && !fs.existsSync(userDb)) {
        closeDb()
        fs.copyFileSync(localDb, userDb)
        for (const ext of ['-wal', '-shm']) {
          const src = localDb + ext
          if (fs.existsSync(src)) fs.copyFileSync(src, userDb + ext)
        }
        console.log('[Sync] 首次登录，已迁移本地数据到', `woo-${safeName}.db`)
      }

      // 切换到用户数据库
      setCurrentUser(username)
      lastSyncTime = null

      // 清除 sync_meta 中的旧 last_sync_time，确保全量推送
      try {
        const db = getDb()
        db.prepare('DELETE FROM sync_meta WHERE key = ?').run('last_sync_time')
        console.log('[Sync] 已清除旧 last_sync_time，即将全量同步')
      } catch (e) {
        console.warn('[Sync] 清除 last_sync_time 失败:', e.message)
      }

      setTimeout(() => syncNow(), 500)
    } else if (event === 'SIGNED_OUT') {
      console.log('[Sync] 用户登出，切回本地数据库')
      currentSession = null
      setCurrentUser(null)
      lastSyncTime = null
      emitStatus()
    }
  })

  // 周期同步
  syncTimer = setInterval(() => {
    syncNow()
  }, intervalMs)

  // 首次启动：如果已有 session 则设置用户数据库并自动同步
  getSession().then(session => {
    const username = session?.user?.user_metadata?.username
    if (username) {
      const { setCurrentUser } = require('../db/index.cjs')
      setCurrentUser(username)
      lastSyncTime = null
      setTimeout(() => syncNow(), 1000)
    }
  })
}

/**
 * 停止同步引擎
 */
function stop() {
  if (syncTimer) {
    clearInterval(syncTimer)
    syncTimer = null
  }
  if (authUnsubscribe) {
    authUnsubscribe()
    authUnsubscribe = null
  }
  console.log('[Sync] 引擎停止')
}

module.exports = {
  start,
  stop,
  syncNow: syncNow,
  getStatus,
  setOnStatusChange,
  setOnDataChange
}
