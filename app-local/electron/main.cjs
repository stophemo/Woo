/**
 * Electron 主进程（本地版）。
 * 相比原版多做两件事：
 *   1. 初始化 SQLite 数据库
 *   2. 注册所有业务 IPC（folder / document / version）
 */
const { app, BrowserWindow, ipcMain, shell, nativeImage } = require('electron')
const path = require('path')
const fs = require('fs')

const { getDb, closeDb } = require('./db/index.cjs')
const ipcRegister = require('./ipc/register.cjs')

function resolveAppIcon() {
  const candidates = [
    path.join(__dirname, '..', 'build', process.platform === 'win32' ? 'icon.ico' : 'icon.png'),
    path.join(__dirname, '..', 'build', 'icon.png'),
    path.join(__dirname, '..', 'dist', 'icon.png')
  ]
  for (const p of candidates) {
    if (fs.existsSync(p)) {
      return process.platform === 'win32' && p.endsWith('.ico') ? p : nativeImage.createFromPath(p)
    }
  }
  return undefined
}

const APP_ICON = resolveAppIcon()

if (process.platform === 'win32') {
  app.setAppUserModelId('com.nonegonotes.woo.local')
}

let mainWindow

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1400,
    height: 800,
    minWidth: 1024,
    minHeight: 768,
    frame: false,
    titleBarStyle: 'hidden',
    backgroundColor: '#1a1a1a',
    icon: APP_ICON,
    webPreferences: {
      preload: path.join(__dirname, 'preload.cjs'),
      contextIsolation: true,
      nodeIntegration: false
    }
  })

  if (process.env.NODE_ENV === 'development') {
    mainWindow.loadURL('http://localhost:5173')
    mainWindow.webContents.openDevTools()
  } else {
    mainWindow.loadFile(path.join(__dirname, '../dist/index.html'))
  }

  // —— 窗口控制 IPC（与原版兼容）——
  ipcMain.on('window:minimize', () => mainWindow.minimize())
  ipcMain.on('window:maximize', () => {
    if (mainWindow.isMaximized()) mainWindow.unmaximize()
    else mainWindow.maximize()
  })
  ipcMain.on('window:close', () => mainWindow.close())
  ipcMain.handle('app:getVersion', () => app.getVersion())
  ipcMain.on('open-external-link', (_e, url) => shell.openExternal(url))
}

app.whenReady().then(() => {
  // 设置自定义 userData 目录（必须在 whenReady 之后、getDb 之前）
  const customUserData = path.join(__dirname, 'userData')
  if (!fs.existsSync(customUserData)) fs.mkdirSync(customUserData, { recursive: true })
  app.setPath('userData', customUserData)

  // 先初始化数据库（会在 userData 下创建 woo.db）
  getDb()
  // 注册业务 IPC
  ipcRegister.register()
  createWindow()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow()
  })
})

app.on('window-all-closed', () => {
  closeDb()
  if (process.platform !== 'darwin') app.quit()
})

app.on('before-quit', () => {
  closeDb()
})
