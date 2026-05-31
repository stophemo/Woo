/**
 * 同步引擎。
 *
 * 设计原则（零预算友好）：
 * - 本地 SQLite 是主数据源，Supabase 是云端副本
 * - 时间戳冲突策略：最后写入者胜出（last-write-wins）
 * - 增量同步：仅推送/拉取 update_time 距上次同步之后变更的记录
 * - 软删除三态：deleted=0（正常）delated=1（废纸篓）deleted=2（待清理）
 * - 超期清理：客户端同步时检测本地 deleted=2 且过期的文档/目录 → 云端真删 + 写墓碑
 * - 墓碑传播：云端 sync_tombstone 表实现跨设备真删通知
 *
 * 同步流程（每次触发）：
 *   1. PULL 墓碑：读取云端 sync_tombstone（比 last_tombstone_pull 新）→ 本地真删对应行
 *   2. PULL 云端变更：读取云端 update_time > last_sync_time 的记录 → 合并到本地
 *   3. PUSH 本地变更：读取本地 update_time > last_sync_time 的记录 → upsert 到云端
 *   4. 超期清理：清理过期文档 → 级联空目录升级 deleted=2 → 清理过期目录
 *   5. 更新 last_sync_time + 回收云端 30 天前的旧墓碑
 */
const { getDb } = require('../db/index.cjs')
const { getClient } = require('../config/supabase.cjs')
const { promoteEmptyFoldersToDeleted2 } = require('./documentService.cjs')
const fs = require('fs')
const path = require('path')
const { app } = require('electron')

const ENTITIES = [
  { table: 'note_folder', idField: 'id' },
  { table: 'note_document', idField: 'id' },
  { table: 'note_document_version', idField: 'id' }
]

/**
 * 安全解析算术表达式（只允许数字、空格和 * 号）
 * @param {string|undefined} expr - 环境变量值，如 "60*60*24*7"
 * @param {number} fallback - 解析失败时的默认值
 * @returns {number}
 */
function parseIntSafe(expr, fallback) {
  if (!expr) return fallback
  if (/^[\d\s*]+$/.test(expr)) {
    try {
      // 乘积展开："60*60*24*7" -> final 值
      const parts = String(expr).split('*').map(s => parseInt(s.trim(), 10))
      if (parts.some(isNaN)) return fallback
      return parts.reduce((a, b) => a * b, 1)
    } catch {
      return fallback
    }
  }
  return fallback
}

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
 */
async function pushChanges(userId, lastSync, skipIds) {
  const client = getClient()
  if (!client) return { ok: false, message: 'Supabase 未配置' }

  let pushedCount = 0
  const errors = []

  for (const entity of ENTITIES) {
    try {
      let localRows = queryLocalChanges(entity.table, entity.idField, lastSync)
      if (localRows.length === 0) continue

      // 过滤回显：跳过本轮刚刚从云端拉取的记录
      if (skipIds) {
        const skipSet = skipIds.get(entity.table)
        if (skipSet && skipSet.size > 0) {
          localRows = localRows.filter(row => !skipSet.has(row[entity.idField]))
          if (localRows.length === 0) continue
        }
      }

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

  return { ok: errors.length === 0, pushedCount, errors }
}

/**
 * 超期清理：扫描本地 deleted=2 且超过7天的记录，执行云端真删、写墓碑、本地真删。
 * 由客户端在同步时触发（非云端定时任务）。
 */
async function cleanupExpiredDeletes(userId) {
  const client = getClient()
  if (!client) return { cleanupCount: 0, errors: [] }
  const db = getDb()

  // 支持 .env 中写表达式，如 "60*60*24*7" = 604800 秒
  // 注意：表达式只允许数字、空格和 * 号，不支持函数调用等任意代码
  const cleanupSeconds = parseIntSafe(process.env.SYNC_CLEANUP_SECONDS, 604800)
  const cutoffDate = new Date(Date.now() - cleanupSeconds * 1000)
  const pad = (n) => String(n).padStart(2, '0')
  const cutoff = `${cutoffDate.getUTCFullYear()}-${pad(cutoffDate.getUTCMonth() + 1)}-${pad(cutoffDate.getUTCDate())}T${pad(cutoffDate.getUTCHours())}:${pad(cutoffDate.getUTCMinutes())}:${pad(cutoffDate.getUTCSeconds())}`

  let cleanupCount = 0
  const errors = []

  // === 1. 清理过期文档（deleted=2 且超期） ===
  const expiredDocs = db.prepare(
    'SELECT id FROM note_document WHERE deleted = 2 AND update_time < ?'
  ).all(cutoff)

  for (const doc of expiredDocs) {
    try {
      await client.from('note_document_version')
        .delete()
        .eq('document_id', doc.id)
        .eq('user_id', userId)
      await client.from('note_document')
        .delete()
        .eq('id', doc.id)
        .eq('user_id', userId)
      await client.from('sync_tombstone')
        .insert({ table_name: 'note_document', record_id: doc.id, user_id: userId })
      db.prepare('DELETE FROM note_document_version WHERE document_id = ?').run(doc.id)
      db.prepare('DELETE FROM note_document WHERE id = ?').run(doc.id)
      cleanupCount++
    } catch (err) {
      errors.push(`[cleanup] 文档清理失败(${doc.id}): ${err.message}`)
    }
  }

  // === 2. 级联：文档清理后，空目录链升级为 deleted=2 ===
  if (expiredDocs.length > 0) {
    promoteEmptyFoldersToDeleted2()
  }

  // === 3. 清理过期目录（deleted=2 且超期） ===
  const expiredFolders = db.prepare(
    'SELECT id FROM note_folder WHERE deleted = 2 AND update_time < ?'
  ).all(cutoff)

  for (const folder of expiredFolders) {
    try {
      await client.from('note_folder')
        .delete()
        .eq('id', folder.id)
        .eq('user_id', userId)
      await client.from('sync_tombstone')
        .insert({ table_name: 'note_folder', record_id: folder.id, user_id: userId })
      db.prepare('DELETE FROM note_folder WHERE id = ?').run(folder.id)
      cleanupCount++
    } catch (err) {
      errors.push(`[cleanup] 目录清理失败(${folder.id}): ${err.message}`)
    }
  }

  return { cleanupCount, errors }
}

/**
 * 从云端拉取墓碑并本地真删
 * 同时在云端回收 30 天前的旧墓碑
 */
async function pullTombstones(userId) {
  const client = getClient()
  if (!client) return { pulledDeleteCount: 0, errors: [] }
  const db = getDb()

  const lastPull = getSyncMeta('last_tombstone_pull') || '1970-01-01T00:00:00'

  const { data: tombstones, error } = await client
    .from('sync_tombstone')
    .select('*')
    .eq('user_id', userId)
    .gt('deleted_at', lastPull)
    .order('deleted_at', { ascending: true })

  if (error) return { pulledDeleteCount: 0, errors: [error.message] }
  if (!tombstones || tombstones.length === 0) return { pulledDeleteCount: 0, errors: [] }

  let count = 0
  for (const t of tombstones) {
    try {
      // 级联删版本
      if (t.table_name === 'note_document') {
        db.prepare('DELETE FROM note_document_version WHERE document_id = ?').run(t.record_id)
      }
      // 删业务行
      db.prepare(`DELETE FROM ${t.table_name} WHERE id = ?`).run(t.record_id)
      count++
    } catch (err) {
      console.warn(`[Sync] 本地墓碑删除失败(${t.table_name}/${t.record_id}): ${err.message}`)
    }
  }

  // 更新拉取进度
  const latest = tombstones[tombstones.length - 1].deleted_at
  setSyncMeta('last_tombstone_pull', latest)

  // 回收云端 30 天前的旧墓碑（控制云端存储成本）
  try {
    const cutoff = new Date(Date.now() - 30 * 24 * 60 * 60 * 1000).toISOString()
    const { error: cleanErr } = await client
      .from('sync_tombstone')
      .delete()
      .eq('user_id', userId)
      .lt('deleted_at', cutoff)
    if (cleanErr) {
      console.warn('[Sync] 回收旧墓碑失败:', cleanErr.message)
    }
  } catch (e) {
    console.warn('[Sync] 回收旧墓碑异常:', e.message)
  }

  return { pulledDeleteCount: count, errors: [] }
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
  const pulledIdsMap = new Map()

  for (const entity of ENTITIES) {
    try {
      const remoteRows = await queryRemoteChanges(client, entity.table, userId, lastSync)
      if (remoteRows.length === 0) continue

      const result = mergeIntoLocal(entity.table, entity.idField, remoteRows)
      pulledCount += result.pulled
      conflictCount += result.conflicts

      // 收集本轮实际拉取的 ID，用于防止回显
      if (result.pulledIds && result.pulledIds.length > 0) {
        pulledIdsMap.set(entity.table, new Set(result.pulledIds))
      }
    } catch (err) {
      errors.push(`[${entity.table}] 拉取异常: ${err.message}`)
    }
  }

  return { ok: errors.length === 0, pulledCount, conflictCount, errors, pulledIds: pulledIdsMap }
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
  const pulledIds = []

  const checkStmt = db.prepare(
    `SELECT update_time FROM ${table} WHERE ${idField} = ?`
  )

  // 剥离远端数据中的 user_id（本地 schema 不含此列）
  const localRows = remoteRows.map(row => {
    const { user_id, ...localRow } = row
    return localRow
  })

  if (localRows.length === 0) return { pulled, conflicts, pulledIds }

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
    pulledIds.push(row[idField])
  }

  return { pulled, conflicts, pulledIds }
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
    const lastSync = getLastSyncTime()

    // === 1. PULL 云端墓碑（其他设备的真删 → 本地真删）===
    const tombstoneResult = await pullTombstones(userId)

    // === 2. PULL 云端变更（先拉取再推送，避免过时数据覆盖云端）===
    const pullResult = await pullChanges(userId, lastSync)

    // === 3. PUSH 本地变更（跳过本轮拉取的记录，避免回显）===
    const pushResult = await pushChanges(userId, lastSync, pullResult.pulledIds)

    // === 4. 超期清理（本地 deleted=2 且过期 → 云端真删 + 写墓碑）===
    const cleanupResult = await cleanupExpiredDeletes(userId)

    // 5. 更新同步时间戳
    const now = new Date().toISOString()
    setLastSyncTime(now)

    const summary = {
      push: pushResult.pushedCount || 0,
      pull: pullResult.pulledCount || 0,
      conflict: pullResult.conflictCount || 0,
      cleanup: cleanupResult.cleanupCount || 0,
    }
    const pushErr = pushResult.errors?.length || 0
    console.log(`[Sync] ⇄ 推${summary.push} 拉${summary.pull} 冲突${summary.conflict}${pushErr ? ` 推送错误${pushErr}(${pushResult.errors[0]?.slice(0,60)}...)` : ''}`)

    return {
      pushedCount: summary.push,
      pulledCount: summary.pull,
      conflictCount: summary.conflict,
      cleanupCount: summary.cleanup,
      tombstoneCount: tombstoneResult.pulledDeleteCount || 0,
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

module.exports = {
  start,
  syncNow: syncNow,
  getStatus,
  setOnStatusChange,
  setOnDataChange
}
