const { app, BrowserWindow, ipcMain, shell, nativeImage } = require('electron')
const path = require('path')
const fs = require('fs')

// 禁用应用数据目录的默认行为
app.setPath('userData', path.join(__dirname, 'userData'))

// 解析应用图标：优先使用 Windows 专用 .ico，其次回退到 PNG
function resolveAppIcon() {
  const candidates = [
    // 开发态：app/build/*
    path.join(__dirname, '..', 'build', process.platform === 'win32' ? 'icon.ico' : 'icon.png'),
    path.join(__dirname, '..', 'build', 'icon.png'),
    // 生产态：打包进 dist（public 目录会被 vite 原样复制）
    path.join(__dirname, '..', 'dist', 'icon.png'),
  ]
  for (const p of candidates) {
    if (fs.existsSync(p)) {
      return process.platform === 'win32' && p.endsWith('.ico') ? p : nativeImage.createFromPath(p)
    }
  }
  return undefined
}

const APP_ICON = resolveAppIcon()

// 任务栏 AppUserModelID（Windows 上让任务栏图标与窗口图标保持一致）
if (process.platform === 'win32') {
  app.setAppUserModelId('com.nonegonotes.woo')
}

let mainWindow

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1400,
    height: 800,
    minWidth: 1024,
    minHeight: 768,
    frame: false, // 自定义窗口边框
    titleBarStyle: 'hidden',
    backgroundColor: '#1a1a1a',
    icon: APP_ICON,
    webPreferences: {
      preload: path.join(__dirname, 'preload.cjs'),
      contextIsolation: true,
      nodeIntegration: false
    }
  })

  // 加载应用
  if (process.env.NODE_ENV === 'development') {
    mainWindow.loadURL('http://localhost:5173')
    mainWindow.webContents.openDevTools()
  } else {
    mainWindow.loadFile(path.join(__dirname, '../dist/index.html'))
  }

  // 窗口控制IPC处理
  ipcMain.on('window:minimize', () => {
    mainWindow.minimize()
  })

  ipcMain.on('window:maximize', () => {
    if (mainWindow.isMaximized()) {
      mainWindow.unmaximize()
    } else {
      mainWindow.maximize()
    }
  })

  ipcMain.on('window:close', () => {
    mainWindow.close()
  })

  // 应用版本信息
  ipcMain.handle('app:getVersion', () => {
    return app.getVersion()
  })

  // 在外部浏览器打开链接
  ipcMain.on('open-external-link', (event, url) => {
    shell.openExternal(url)
  })
}

app.whenReady().then(() => {
  createWindow()

  app.on('activate', function () {
    if (BrowserWindow.getAllWindows().length === 0) createWindow()
  })
})

app.on('window-all-closed', function () {
  if (process.platform !== 'darwin') app.quit()
})