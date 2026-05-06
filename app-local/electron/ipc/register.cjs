/**
 * IPC 通道注册（本地版）。
 * 无认证，无 userId，渲染端直接调用业务方法。
 */
const { ipcMain } = require('electron')
const folderService = require('../services/folderService.cjs')
const documentService = require('../services/documentService.cjs')
const versionService = require('../services/versionService.cjs')

function wrap(fn) {
  return async (_event, ...args) => {
    try {
      console.log('[IPC] called with args:', args)
      const data = await fn(...args)
      console.log('[IPC] success:', data)
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
