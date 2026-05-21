/**
 * 预加载脚本：通过 contextBridge 暴露受控 API 给渲染进程。
 * 业务 IPC 统一通过 woo.invoke(channel, ...args) 调用。
 */
const { contextBridge, ipcRenderer } = require('electron')

contextBridge.exposeInMainWorld('electronAPI', {
  minimize: () => ipcRenderer.send('window:minimize'),
  maximize: () => ipcRenderer.send('window:maximize'),
  close: () => ipcRenderer.send('window:close'),
  openExternalLink: (url) => ipcRenderer.send('open-external-link', url),
  getAppVersion: () => ipcRenderer.invoke('app:getVersion')
})

// 业务 IPC 调用入口：统一 invoke，返回 { ok, data | message }
contextBridge.exposeInMainWorld('woo', {
  invoke: (channel, ...args) => ipcRenderer.invoke(channel, ...args)
})
