const { contextBridge, ipcRenderer } = require('electron')

// 暴露API给渲染进程
contextBridge.exposeInMainWorld('electronAPI', {
  // 窗口控制
  minimize: () => ipcRenderer.send('window:minimize'),
  maximize: () => ipcRenderer.send('window:maximize'),
  close: () => ipcRenderer.send('window:close'),
  // 打开外部链接
  openExternalLink: (url) => ipcRenderer.send('open-external-link', url),
  // 其他API
  getAppVersion: () => ipcRenderer.invoke('app:getVersion')
})

// 监听主进程消息
ipcRenderer.on('message', (event, message) => {
  console.log('Received message from main:', message)
})