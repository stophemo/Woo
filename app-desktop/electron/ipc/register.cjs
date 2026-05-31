/**
 * IPC 通道注册（本地版）。
 * 无认证，无 userId，渲染端直接调用业务方法。
 */
const { ipcMain } = require('electron')
const folderService = require('../services/folderService.cjs')
const documentService = require('../services/documentService.cjs')
const versionService = require('../services/versionService.cjs')
const authService = require('../services/authService.cjs')
const lockService = require('../services/lockService.cjs')
const syncEngine = require('../services/syncEngine.cjs')
const kbService = require('../services/kbService.cjs')

/**
 * 简要日志包装：只输出做了什么和结果摘要，不打印详参。
 *
 * 格式:  [IPC] folder:tree → 2 folders
 *        [IPC] document:get → ok
 *        [IPC] ERROR folder:remove → xxx (错误信息)
 */
function wrap(fn, label) {
  return async (_event, ...args) => {
    try {
      const data = await fn(...args)
      if (logLevel === 'verbose') {
        console.log(`[IPC] ${label}`, args, '→', data)
      } else {
        console.log(`[IPC] ${label} → ${summarizeResult(data)}`)
      }
      return { ok: true, data }
    } catch (err) {
      console.log(`[IPC] ERROR ${label} → ${err?.message || '操作失败'}`)
      return { ok: false, message: err?.message || '操作失败' }
    }
  }
}

function summarizeResult(data) {
  if (data === null || data === undefined) return 'ok'
  if (Array.isArray(data)) return `${data.length} items`
  if (typeof data === 'object') {
    const keys = Object.keys(data)
    // 列出有意义的数字字段
    const nums = keys.filter(k => typeof data[k] === 'number' && data[k] > 0)
    if (nums.length > 0) return nums.map(k => `${k}:${data[k]}`).join(', ')
    // 布尔字段
    const bools = keys.filter(k => data[k] === true)
    if (bools.length > 0) return bools.join(', ')
    return 'ok'
  }
  return String(data).slice(0, 60)
}

/** 运行时日志级别，可通过 IPC 动态切换 */
let logLevel = process.env.LOG_LEVEL === 'verbose' ? 'verbose' : 'brief'

/** 便捷注册：自动将通道名作为标签传给 wrap */
function handle(channel, fn) {
  ipcMain.handle(channel, wrap(fn, channel))
}

function register() {
  handle('folder:tree', () => folderService.getFolderTree())
  handle('folder:create', (payload) => folderService.createFolder(payload))
  handle('folder:rename', (folderId, name) => { folderService.renameFolder(folderId, name) })
  handle('folder:remove', (folderId) => { folderService.deleteFolder(folderId) })

  handle('document:listByFolder', (folderId) => documentService.listByFolder(folderId))
  handle('document:listAll', () => documentService.listAll())
  handle('document:listTrash', () => documentService.listTrash())
  handle('document:listOrphans', () => documentService.listOrphans())
  handle('document:search', (keyword) => documentService.search(keyword))
  handle('document:get', (id) => documentService.getById(id))
  handle('document:create', (payload) => documentService.create(payload))
  handle('document:rename', (id, title) => { documentService.rename(id, title) })
  handle('document:updateContent', (id, content) => { documentService.updateContent(id, content) })
  handle('document:remove', (id) => { documentService.remove(id) })
  handle('document:restore', (id) => { documentService.restore(id) })
  handle('document:hardDelete', (id) => { documentService.hardDelete(id) })
  handle('document:emptyTrash', () => { documentService.emptyTrash() })

  handle('auth:signUp', authService.signUp)
  handle('auth:signIn', authService.signIn)
  handle('auth:signOut', authService.signOut)
  handle('auth:getUser', authService.getCurrentUser)
  handle('auth:getSession', authService.getSession)

  handle('lock:status', async () => ({ hasPassword: lockService.hasPassword(), mode: lockService.getPasswordMode() }))
  handle('lock:setPassword', async (password) => { lockService.setPassword(password) })
  handle('lock:verifyPassword', async (password) => lockService.verifyPassword(password))
  handle('lock:lockFolder', async (folderId) => { lockService.lockFolder(folderId) })
  handle('lock:unlockFolder', async (folderId) => { lockService.unlockFolder(folderId) })
  handle('lock:isFolderLocked', async (folderId) => lockService.isFolderLocked(folderId))
  handle('lock:isFolderEffectivelyLocked', async (folderId) => lockService.isFolderEffectivelyLocked(folderId))
  handle('lock:lockDocument', async (documentId) => { lockService.lockDocument(documentId) })
  handle('lock:unlockDocument', async (documentId) => { lockService.unlockDocument(documentId) })
  handle('lock:isDocumentLocked', async (documentId) => lockService.isDocumentLocked(documentId))
  handle('lock:cloudPushSettings', async (password) => { await lockService.cloudPushSettings(password) })
  handle('lock:cloudPullSettings', async () => { await lockService.cloudPullSettings() })

  // —— system ——
  ipcMain.handle('system:setLogLevel', wrap(async (level) => {
    if (level === 'verbose' || level === 'brief') {
      logLevel = level
      console.log(`[IPC] 日志级别已切换为: ${level}`)
    }
    return { level: logLevel }
  }, 'system:setLogLevel'))
  ipcMain.handle('system:getLogLevel', wrap(async () => ({ level: logLevel }), 'system:getLogLevel'))

  handle('kb:rebuild', kbService.rebuild)
  handle('kb:search', (query, limit) => kbService.search(query, limit))
  handle('kb:status', kbService.status)

  handle('sync:status', () => { const s = syncEngine.getStatus(); return { isSyncing: s.isSyncing, lastSyncTime: s.lastSyncTime, pendingChanges: s.pendingChanges } })
  handle('sync:trigger', syncEngine.syncNow)

  // Agent 终端日志（渲染进程 → 主进程 → 终端，无需包装）
  ipcMain.handle('log:agent', async (_event, level, message) => {
    if (level === 'warn') console.warn('[Agent]', message)
    else if (level === 'error') console.error('[Agent]', message)
    else console.log('[Agent]', message)
  })

  handle('version:list', (documentId) => versionService.listVersions(documentId))
  handle('version:get', (documentId, versionNo) => versionService.getVersion(documentId, versionNo))
  handle('version:saveManual', (documentId) => { versionService.saveManual(documentId) })
  handle('version:commit', (documentId, changeType) => { versionService.commit(documentId, changeType) })
  handle('version:restore', (documentId, versionNo) => versionService.restore(documentId, versionNo))
}

module.exports = { register }
