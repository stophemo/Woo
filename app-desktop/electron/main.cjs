/**
 * Electron 主进程（本地版）。
 * 相比原版多做两件事：
 *   1. 初始化 SQLite 数据库
 *   2. 注册所有业务 IPC（folder / document / version）
 */

// [Windows] 强制控制台使用 UTF-8 编码，防止中文日志乱码
if (process.platform === 'win32') {
  try {
    require('child_process').execSync('chcp 65001 > NUL', { stdio: 'ignore' })
  } catch (_) {}
  try { process.stdout.setDefaultEncoding('utf8') } catch (_) {}
  try { process.stderr.setDefaultEncoding('utf8') } catch (_) {}
}

const { app, BrowserWindow, ipcMain, shell, nativeImage, Menu } = require('electron')
const path = require('path')
const fs = require('fs')

const { getDb, closeDb } = require('./db/index.cjs')
const ipcRegister = require('./ipc/register.cjs')
const syncEngine = require('./services/syncEngine.cjs')

/**
 * 判断是否为便携模式（ZIP / portable 绿色版）。
 * 判断依据：exe 所在目录是否可写。
 *   - 可写 → portable 模式（数据放 exe 同级 userData/）
 *   - 不可写 → NSIS 安装模式（数据放标准 app.getPath('userData')，如 %APPDATA%）
 */
function isPortable() {
  if (!app.isPackaged) return true // 开发时始终便携
  try {
    const exeDir = path.dirname(process.execPath)
    const testFile = path.join(exeDir, `.write-test-${Date.now()}`)
    fs.writeFileSync(testFile, '')
    fs.unlinkSync(testFile)
    return true
  } catch {
    return false
  }
}

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
}

/**
 * 通过 GitHub API 检查最新版本，返回匹配当前平台的下载链接。
 * 渲染进程通过 invoke('update:check') 调用，结果直接返回。
 */
async function checkForUpdates() {
  const res = await fetch('https://api.github.com/repos/stophemo/Woo/releases/latest')
  if (!res.ok) throw new Error(`GitHub API 请求失败 (${res.status})`)
  const release = await res.json()
  const latestVer = release.tag_name.replace(/^v/, '')
  const currentVer = app.getVersion()

  // 版本相同或更旧
  if (latestVer === currentVer) {
    return { hasUpdate: false }
  }

  // 根据平台和安装模式匹配下载链接
  const version = latestVer
  let expectedName = ''
  if (process.platform === 'win32') {
    expectedName = isPortable()
      ? `woo-desktop-${version}-win-x64.zip`
      : `woo-desktop-${version}-win-x64-setup.exe`
  } else if (process.platform === 'darwin') {
    expectedName = `woo-desktop-${version}-mac-x64.dmg`
  } else {
    expectedName = `woo-desktop-${version}-linux-x64.AppImage`
  }

  const asset = release.assets.find(a => a.name === expectedName)
  const downloadUrl = asset
    ? asset.browser_download_url
    : release.html_url  // 回退到 Release 页面

  return { hasUpdate: true, version, downloadUrl }
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

/**
 * 迁移旧版数据：将 v0.3.x 时期 exe 同级 userData/ 下的数据库
 * 复制到当前 userData 路径。
 *
 * 此函数用于 NSIS 安装版从旧版升级到新版时的数据迁移：
 *   旧路径: C:\Program Files\Woo\userData\woo.db
 *   新路径: %APPDATA%\Woo\woo.db
 *
 * 便携版（ZIP/portable）两者路径相同，会自动跳过。
 */
function migrateOldUserData() {
  if (!app.isPackaged) return

  const oldDir = path.join(path.dirname(process.execPath), 'userData')
  const newDir = app.getPath('userData')
  if (oldDir === newDir) return

  const oldDb = path.join(oldDir, 'woo.db')
  const newDb = path.join(newDir, 'woo.db')

  // 只有旧路径有数据且新路径还没有时，才执行迁移
  if (!fs.existsSync(oldDb) || fs.existsSync(newDb)) return

  try {
    if (!fs.existsSync(newDir)) fs.mkdirSync(newDir, { recursive: true })
    fs.copyFileSync(oldDb, newDb)
    // 同时迁移 WAL/SHM 文件
    for (const ext of ['-wal', '-shm']) {
      const src = oldDb + ext
      if (fs.existsSync(src)) fs.copyFileSync(src, newDb + ext)
    }
    console.log('[Migrate] 旧版数据已迁移:', oldDb, '→', newDb)

    // 迁移用户专属数据库（woo-{用户名}.db）
    const oldDirList = fs.readdirSync(oldDir)
    for (const file of oldDirList) {
      if (file.startsWith('woo-') && file.endsWith('.db')) {
        const src = path.join(oldDir, file)
        const dst = path.join(newDir, file)
        if (!fs.existsSync(dst)) {
          fs.copyFileSync(src, dst)
          for (const ext of ['-wal', '-shm']) {
            const walSrc = src + ext
            if (fs.existsSync(walSrc)) fs.copyFileSync(walSrc, dst + ext)
          }
          console.log('[Migrate] 用户数据库已迁移:', file)
        }
      }
    }
  } catch (err) {
    console.error('[Migrate] 数据迁移失败:', err)
  }
}

app.whenReady().then(() => {
  // 构建原生菜单（macOS 系统菜单栏集成，窗口聚焦即生效）
  buildMenu()

  // 便携模式（ZIP/portable）：数据放 exe 同级 userData/
  // NSIS 安装模式：使用默认 userData（%APPDATA%/Woo）
  if (isPortable()) {
    const portableData = path.join(path.dirname(process.execPath), 'userData')
    if (!fs.existsSync(portableData)) fs.mkdirSync(portableData, { recursive: true })
    app.setPath('userData', portableData)
  }

  // 迁移旧版数据（v0.3.x 及以前，userData 在 exe 同级目录下）
  migrateOldUserData()

  // 初始化数据库（会在 app.getPath('userData') 下创建 woo.db）
  getDb()
  // 注册业务 IPC
  ipcRegister.register()

  // —— 全局 IPC（只注册一次，不依赖窗口）——
  ipcMain.handle('app:getVersion', () => app.getVersion())
  ipcMain.on('open-external-link', (_e, url) => shell.openExternal(url))

  // 注册检查更新 IPC（invoke 模式，渲染进程直接 await 结果）
  ipcMain.handle('update:check', async () => {
    try {
      return await checkForUpdates()
    } catch (err) {
      return { hasUpdate: false, error: err.message }
    }
  })

  createWindow()

  // —— 窗口控制 IPC（依赖 mainWindow，但只注册一次）——
  ipcMain.on('window:minimize', () => mainWindow.minimize())
  ipcMain.on('window:maximize', () => {
    if (mainWindow.isMaximized()) mainWindow.unmaximize()
    else mainWindow.maximize()
  })
  ipcMain.on('window:close', () => mainWindow.close())
  ipcMain.handle('window:set-fullscreen', (_e, fullscreen) => {
    mainWindow.setFullScreen(fullscreen)
  })

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
