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

/**
 * 截断日志中的 content 字段，只保留标题和摘要。
 * 完整文稿内容在日志中无意义且刷屏。
 */
function sanitizeLog(val) {
  if (Array.isArray(val)) return val.map(sanitizeLog)
  if (val && typeof val === 'object') {
    const copy = { ...val }
    if (copy.content && typeof copy.content === 'string' && copy.content.length > 100) {
      copy.content = `[content: ${copy.content.length} chars]`
    }
    return copy
  }
  return val
}

function wrap(fn) {
  return async (_event, ...args) => {
    try {
      console.log('[IPC] called with args:', sanitizeLog(args))
      const data = await fn(...args)
      console.log('[IPC] success:', sanitizeLog(data))
      return { ok: true, data }
    } catch (err) {
      console.error('[IPC] error:', err)
      return { ok: false, message: err?.message || '操作失败' }
    }
  }
}

function register() {
  // —— folder ——
  ipcMain.handle('folder:tree', wrap(() => folderService.getFolderTree()))
  ipcMain.handle('folder:create', wrap((payload) => folderService.createFolder(payload)))
  ipcMain.handle('folder:rename', wrap((folderId, name) => {
    folderService.renameFolder(folderId, name); return null
  }))
  ipcMain.handle('folder:remove', wrap((folderId) => {
    folderService.deleteFolder(folderId); return null
  }))

  // —— document ——
  ipcMain.handle('document:listByFolder', wrap((folderId) => documentService.listByFolder(folderId)))
  ipcMain.handle('document:listAll', wrap(() => documentService.listAll()))
  ipcMain.handle('document:listTrash', wrap(() => documentService.listTrash()))
  ipcMain.handle('document:listOrphans', wrap(() => documentService.listOrphans()))
  ipcMain.handle('document:search', wrap((keyword) => documentService.search(keyword)))
  ipcMain.handle('document:get', wrap((id) => documentService.getById(id)))
  ipcMain.handle('document:create', wrap((payload) => documentService.create(payload)))
  ipcMain.handle('document:rename', wrap((id, title) => {
    documentService.rename(id, title); return null
  }))
  ipcMain.handle('document:updateContent', wrap((id, content) => {
    documentService.updateContent(id, content); return null
  }))
  ipcMain.handle('document:remove', wrap((id) => {
    documentService.remove(id); return null
  }))
  ipcMain.handle('document:restore', wrap((id) => {
    documentService.restore(id); return null
  }))
  ipcMain.handle('document:hardDelete', wrap((id) => {
    documentService.hardDelete(id); return null
  }))
  ipcMain.handle('document:emptyTrash', wrap(() => {
    documentService.emptyTrash(); return null
  }))

  // —— auth ——
  ipcMain.handle('auth:signUp', wrap(authService.signUp))
  ipcMain.handle('auth:signIn', wrap(authService.signIn))
  ipcMain.handle('auth:signOut', wrap(authService.signOut))
  ipcMain.handle('auth:getUser', wrap(authService.getCurrentUser))
  ipcMain.handle('auth:getSession', wrap(authService.getSession))

  // —— lock ——
  ipcMain.handle('lock:status', wrap(async () => ({
    hasPassword: lockService.hasPassword(),
    mode: lockService.getPasswordMode()
  })))
  ipcMain.handle('lock:setPassword', wrap(async (password) => { lockService.setPassword(password); return null }))
  ipcMain.handle('lock:verifyPassword', wrap(async (password) => lockService.verifyPassword(password)))
  ipcMain.handle('lock:lockFolder', wrap(async (folderId) => { lockService.lockFolder(folderId); return null }))
  ipcMain.handle('lock:unlockFolder', wrap(async (folderId) => { lockService.unlockFolder(folderId); return null }))
  ipcMain.handle('lock:isFolderLocked', wrap(async (folderId) => lockService.isFolderLocked(folderId)))
  ipcMain.handle('lock:isFolderEffectivelyLocked', wrap(async (folderId) => lockService.isFolderEffectivelyLocked(folderId)))
  ipcMain.handle('lock:lockDocument', wrap(async (documentId) => { lockService.lockDocument(documentId); return null }))
  ipcMain.handle('lock:unlockDocument', wrap(async (documentId) => { lockService.unlockDocument(documentId); return null }))
  ipcMain.handle('lock:isDocumentLocked', wrap(async (documentId) => lockService.isDocumentLocked(documentId)))
  ipcMain.handle('lock:cloudPushSettings', wrap(async (password) => { await lockService.cloudPushSettings(password); return null }))
  ipcMain.handle('lock:cloudPullSettings', wrap(async () => { await lockService.cloudPullSettings(); return null }))

  // —— sync ——
  ipcMain.handle('sync:status', wrap(() => {
    const s = syncEngine.getStatus()
    return {
      isSyncing: s.isSyncing,
      lastSyncTime: s.lastSyncTime,
      pendingChanges: s.pendingChanges
    }
  }))
  ipcMain.handle('sync:trigger', wrap(syncEngine.syncNow))

  // —— version ——
  ipcMain.handle('version:list', wrap((documentId) => versionService.listVersions(documentId)))
  ipcMain.handle('version:get', wrap((documentId, versionNo) => versionService.getVersion(documentId, versionNo)))
  ipcMain.handle('version:saveManual', wrap((documentId) => {
    versionService.saveManual(documentId); return null
  }))
  ipcMain.handle('version:commit', wrap((documentId, changeType) => {
    versionService.commit(documentId, changeType); return null
  }))
  ipcMain.handle('version:restore', wrap((documentId, versionNo) => versionService.restore(documentId, versionNo)))
}

module.exports = { register }
