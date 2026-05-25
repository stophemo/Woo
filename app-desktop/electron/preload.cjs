/**
 * 预加载脚本：通过 contextBridge 暴露受控 API 给渲染进程。
 * 业务 IPC 统一通过 woo.invoke(channel, ...args) 调用。
 */
const { contextBridge, ipcRenderer } = require('electron')

contextBridge.exposeInMainWorld('electronAPI', {
  minimize: () => ipcRenderer.send('window:minimize'),
  maximize: () => ipcRenderer.send('window:maximize'),
  close: () => ipcRenderer.send('window:close'),
  setFullscreen: (fullscreen) => ipcRenderer.invoke('window:set-fullscreen', fullscreen),
  openExternalLink: (url) => ipcRenderer.send('open-external-link', url),
  getAppVersion: () => ipcRenderer.invoke('app:getVersion'),
  // 监听原生菜单动作（macOS 菜单栏/快捷键）
  onMenuAction: (callback) => {
    ipcRenderer.on('menu:action', (_event, action) => callback(action))
  },
  // 移除原生菜单监听
  removeMenuActionListener: () => {
    ipcRenderer.removeAllListeners('menu:action')
  },

  // 检查更新：返回 { hasUpdate, version?, downloadUrl?, error? }
  checkForUpdates: () => ipcRenderer.invoke('update:check'),
})

// 业务 IPC 调用入口：统一 invoke，返回 { ok, data | message }
contextBridge.exposeInMainWorld('woo', {
  invoke: (channel, ...args) => ipcRenderer.invoke(channel, ...args)
})

// 监听主进程推送的同步状态更新
ipcRenderer.on('sync:status-update', (_event, status) => {
  // 通过自定义事件转发给渲染进程
  window.dispatchEvent(new CustomEvent('sync-status', { detail: status }))
})

// 监听主进程通知数据已变更（同步完成后的刷新信号）
ipcRenderer.on('sync:data-changed', () => {
  window.dispatchEvent(new CustomEvent('sync-data-changed'))
})
