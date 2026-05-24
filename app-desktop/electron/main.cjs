/**
 * Electron 主进程（本地版）。
 * 相比原版多做两件事：
 *   1. 初始化 SQLite 数据库
 *   2. 注册所有业务 IPC（folder / document / version）
 */
const { app, BrowserWindow, ipcMain, shell, nativeImage, Menu } = require('electron')
const path = require('path')
const fs = require('fs')

// 获取应用根目录（兼容开发和打包环境）
const getAppRoot = () => {
  // app.isPackaged = true 时为打包环境，false 为开发环境
  if (app.isPackaged) {
    // 打包环境：使用 exe 所在目录
    return path.dirname(process.execPath)
  }
  // 开发环境：返回项目根目录
  return path.join(__dirname, '..')
}

const APP_ROOT = getAppRoot()

const { getDb, closeDb } = require('./db/index.cjs')
const ipcRegister = require('./ipc/register.cjs')
const syncEngine = require('./services/syncEngine.cjs')

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
  const isDev = !!process.env.VITE_DEV_SERVER_URL
  const isMac = process.platform === 'darwin'

  mainWindow = new BrowserWindow({
    width: 1400,
    height: 800,
    minWidth: 1024,
    minHeight: 768,
    // Windows: 无边框（frame: false）；
    // macOS:  保留原生框架（frame: true）并隐藏标题栏文字，
    //         保留交通灯按钮（红黄绿灯）。
    //         titleBarOverlay 使交通灯在 40px 工具栏中垂直居中。
    frame: isMac,
    titleBarStyle: isMac ? 'hidden' : undefined,
    titleBarOverlay: isMac ? {
      height: 40
    } : undefined,
    backgroundColor: '#1a1a1a',
    icon: APP_ICON,
    webPreferences: {
      // 开发模式和生产模式都直接使用 electron/preload.cjs（CJS）
      // 不依赖 vite-plugin-electron 编译后的 ESM 版本，避免兼容问题
      preload: path.join(__dirname, 'preload.cjs'),
      contextIsolation: true,
      nodeIntegration: false
    }
  })

  if (isDev) {
    mainWindow.loadURL(process.env.VITE_DEV_SERVER_URL)
    mainWindow.webContents.openDevTools()
  } else {
    // 打包后使用 app.getAppPath() 获取 asar 包根目录
    const appPath = app.getAppPath()
    mainWindow.loadFile(path.join(appPath, 'dist/index.html'))
  }

  // 监听 preload 加载错误
  mainWindow.webContents.on('preload-error', (_event, preloadPath, err) => {
    console.error('[MAIN] preload 加载失败:', preloadPath, err)
  })

  // macOS 全屏时隐藏应用内菜单栏（依赖系统菜单栏）
  if (isMac) {
    mainWindow.on('enter-full-screen', () => {
      mainWindow.webContents.send('menu:action', 'enter-fullscreen')
    })
    mainWindow.on('leave-full-screen', () => {
      mainWindow.webContents.send('menu:action', 'leave-fullscreen')
    })
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

  // 窗口全屏控制（ESC 退出全屏、程序化控制）
  ipcMain.handle('window:set-fullscreen', (_e, fullscreen) => {
    mainWindow.setFullScreen(fullscreen)
  })
}

// macOS 原生菜单：窗口聚焦时自动显示在系统菜单栏（不仅是全屏模式）
function buildMenu() {
  if (process.platform !== 'darwin') return

  const sendAction = (action) => {
    if (mainWindow && !mainWindow.isDestroyed()) {
      mainWindow.webContents.send('menu:action', action)
    }
  }

  const template = [
    {
      role: 'appMenu',
      submenu: [
        { role: 'about' },
        { type: 'separator' },
        {
          label: '设置',
          accelerator: 'Cmd+,',
          click: () => sendAction('settings')
        },
        { type: 'separator' },
        { role: 'hide' },
        { role: 'hideOthers' },
        { role: 'unhide' },
        { type: 'separator' },
        { role: 'quit' }
      ]
    },
    {
      label: '文件',
      submenu: [
        { label: '新建文稿', accelerator: 'Cmd+N', click: () => sendAction('new-document') },
        { label: '新建文件夹', click: () => sendAction('new-folder') },
        { type: 'separator' },
        { label: '版本历史', click: () => sendAction('version-history') },
        { type: 'separator' },
        { label: '设置', accelerator: 'Cmd+,', click: () => sendAction('settings') },
        { type: 'separator' },
        { role: 'quit' }
      ]
    },
    {
      label: '编辑',
      submenu: [
        { role: 'undo' },
        { role: 'redo' },
        { type: 'separator' },
        { role: 'cut' },
        { role: 'copy' },
        { role: 'paste' },
        { type: 'separator' },
        { label: '查找', accelerator: 'Cmd+F', click: () => sendAction('find') }
      ]
    },
    {
      label: '查看',
      submenu: [
        { label: '切换左侧栏', click: () => sendAction('toggle-left') },
        { label: '切换缩略图栏', click: () => sendAction('toggle-thumbnail') },
        { label: '切换 AI 聊天', click: () => sendAction('toggle-right') },
        { type: 'separator' },
        { label: '切换主题', click: () => sendAction('theme') },
        { type: 'separator' },
        { role: 'togglefullscreen' }
      ]
    },
    {
      label: 'AI',
      submenu: [
        { label: 'Open Chat', click: () => sendAction('open-chat') },
        { label: '模型配置', click: () => sendAction('ai-settings') }
      ]
    },
    {
      label: '帮助',
      submenu: [
        { label: '文档介绍', click: () => sendAction('docs') },
        { type: 'separator' },
        { label: 'GitHub', click: () => sendAction('github') }
      ]
    }
  ]

  const menu = Menu.buildFromTemplate(template)
  Menu.setApplicationMenu(menu)
}

app.whenReady().then(() => {
  // 构建原生菜单（macOS 系统菜单栏集成，窗口聚焦即生效）
  buildMenu()

  // 设置自定义 userData 目录（打包后使用 exe 同级目录，开发环境使用项目根目录）
  const customUserData = path.join(APP_ROOT, 'userData')
  if (!fs.existsSync(customUserData)) fs.mkdirSync(customUserData, { recursive: true })
  app.setPath('userData', customUserData)

  // 初始化数据库（会在 userData 下创建 woo.db）
  getDb()
  // 注册业务 IPC
  ipcRegister.register()
  createWindow()

  // 启动同步引擎：设置状态回调 + 数据变更回调 + 启动定时器
  syncEngine.setOnStatusChange((status) => {
    if (mainWindow && !mainWindow.isDestroyed()) {
      mainWindow.webContents.send('sync:status-update', status)
    }
  })
  syncEngine.setOnDataChange(() => {
    if (mainWindow && !mainWindow.isDestroyed()) {
      mainWindow.webContents.send('sync:data-changed')
    }
  })
  syncEngine.start()

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
