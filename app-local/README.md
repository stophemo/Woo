# 无我笔记（本地版） · app-local

> **Woo · Local Edition** — 一个零依赖、纯本地、开箱即用的桌面笔记应用。

基于 Electron + Vue 3 + SQLite 打造，所有数据保存在你自己机器上，无需注册任何账号、无需任何后端服务、无需联网即可运行。

## ✨ 特性

- **完全离线**：业务数据全部存储在本地 SQLite 数据库，默认位置 `%APPDATA%/无我笔记/woo.db`（Windows）
- **零服务器依赖**：Spring Boot / Nacos / MySQL 全部内嵌进 Electron 主进程，用户不需要运行任何服务
- **多账户**：本地可创建多个账户，互相独立
- **层级目录 + Markdown 文稿**：左侧树形目录、Tiptap 富文本编辑器、任意嵌套
- **内容级版本控制**：自动在每次保存时建立快照，24h / 7d 分层合并策略，支持回滚到任意历史版本
- **AI 辅助写作**：内置 Google Gemini 接入（需自备 API Key）
- **开箱即用**：发布包支持一键安装 (.exe) 与绿色版 (.zip)，用户下载解压双击即可

## 🏗️ 技术栈

| 层 | 技术 |
|---|---|
| 壳 | Electron 29（Chromium + Node 20 ABI） |
| UI | Vue 3 + Pinia + Vite 5 |
| 编辑器 | Tiptap v3 |
| 本地存储 | better-sqlite3（SQLite 3，原生模块，同步 API） |
| 密码加密 | bcryptjs |
| 进程通信 | Electron IPC（`contextBridge` + `ipcMain.handle`） |
| 打包 | electron-builder |

## 📂 目录结构

```
app-local/
├── electron/                  # Electron 主进程 + 业务后端
│   ├── main.cjs              # 主进程入口（窗口 + DB 初始化 + IPC 注册）
│   ├── preload.cjs           # 预加载脚本（暴露 window.electronAPI / window.woo）
│   ├── db/
│   │   ├── schema.cjs        # 建表 SQL（sys_user / note_folder / note_document / note_document_version）
│   │   └── index.cjs         # SQLite 连接单例
│   ├── services/             # 业务层（对应原 Spring 的 Service）
│   │   ├── authService.cjs   # 注册 / 登录 / 用户信息
│   │   ├── folderService.cjs # 目录 CRUD + 树构建
│   │   ├── documentService.cjs # 文稿 CRUD + 首行提标题 + Git 分支名
│   │   ├── versionService.cjs  # 版本快照 + Levenshtein 合并 + 回滚
│   │   └── utils.cjs         # newId / sha256 / stripHtml
│   └── ipc/
│       └── register.cjs      # IPC 通道集中注册（auth:* / folder:* / document:* / version:*）
├── src/                      # 渲染进程（Vue 应用，与原 app/ 同构）
│   ├── services/             # API 客户端层（axios → IPC）
│   ├── stores/               # Pinia stores（未改动）
│   ├── components/           # UI 组件（未改动）
│   └── main.ts
├── build/                    # 图标资源（icon.ico / icon.png）
├── scripts/                  # 辅助脚本
├── index.html
├── package.json
└── vite.config.ts
```

## 🔧 开发环境要求

| 工具 | 版本 | 用途 |
|---|---|---|
| Node.js | ≥ 18.19 | 运行 Vite / Electron |
| npm | ≥ 10 | 包管理 |
| Python | 3.10 或 3.11（64-bit）| `node-gyp` 编译 better-sqlite3 |
| Visual Studio Build Tools | 2022，含 "C++ 桌面开发" 工作负载 | MSVC 编译器 |

> ⚠️ Python 和 VS Build Tools **仅打包机需要**，最终用户下载 `.exe` 后 **不需要**这些工具。
> 
> ⚠️ Windows 如果装了 Python 但 `python --version` 无输出，多半是 "应用执行别名" 挡住了，到 `设置 → 应用 → 管理应用执行别名` 里关掉 `python.exe` 和 `python3.exe` 即可。

## 🚀 快速开始

### 1. 安装依赖

```bash
cd app-local
npm install
```

`postinstall` 钩子会自动调用 `electron-builder install-app-deps`，把 better-sqlite3 按 Electron 的 Node ABI 重新编译一次。

### 2. 启动开发模式

两个终端分别跑：

```bash
# 终端 A：Vite 开发服务器（热更新）
npm run dev

# 终端 B：Electron 主进程（加载 http://localhost:5173）
npm run electron:dev
```

### 3. 构建发布版

```bash
npm run electron:build
```

产物在 `release/` 目录：
- `无我笔记 Setup 0.1.0.exe` — NSIS 一键安装包
- `win-unpacked/` — 未打包目录，可复制为绿色版

> 想同时产出 portable 单文件绿色版，编辑 `package.json` 的 `build.win.target`：
> ```json
> "target": ["nsis", "portable", "zip"]
> ```

## 🗄️ 数据存储

- **位置**：`app.getPath('userData')/woo.db`
  - Windows: `%APPDATA%/无我笔记/woo.db`
  - macOS: `~/Library/Application Support/无我笔记/woo.db`
  - Linux: `~/.config/无我笔记/woo.db`
- **模式**：SQLite WAL 模式，崩溃安全
- **备份**：直接复制 `woo.db` 文件即可
- **迁移**：把 `woo.db` 复制到另一台机器的同一路径即可原样恢复

## 🔐 账户与认证

- 本地版不签发 JWT。`login` 成功后，`userId` 本身被当作 token 使用。
- 密码使用 bcryptjs（cost=10）加密存储。
- 每次 IPC 调用渲染端会把 `userId` 作为首参数传入，主进程据此查询对应数据。
- 同一机器可创建多个账户，数据严格隔离。

## 🔌 IPC 通道速查

所有通道统一返回 `{ ok: boolean, data?: T, message?: string }` 格式。

| 通道 | 用途 |
|---|---|
| `auth:register` | 注册 |
| `auth:login` | 登录 |
| `auth:me` | 获取当前用户信息 |
| `folder:tree` / `folder:create` / `folder:rename` / `folder:remove` | 目录管理 |
| `document:listByFolder` / `document:get` / `document:create` / `document:rename` / `document:updateContent` / `document:remove` | 文稿管理 |
| `version:list` / `version:get` / `version:saveManual` / `version:commit` / `version:restore` | 版本管理 |

## 📦 发布流程

每次发版仅需：

1. 更新 `package.json` 的 `version`
2. 执行 `npm run electron:build`
3. 把 `release/` 下的 `.exe` / `.zip` 上传到 GitHub Releases

用户侧永远是：下载 → 安装/解压 → 双击运行，三步到位。

## 🆚 与 app/（原版）的关系

`app-local/` 是 `app/` 的本地化分支：

- **复用**：UI 组件、Pinia stores、Tiptap 编辑器、路由
- **替换**：`src/services/*` 的 axios HTTP 调用 → Electron IPC 调用
- **新增**：`electron/` 下的主进程业务层 + SQLite 存储
- **移除**：对 `services/` 下 Spring Boot 微服务、Nacos、MySQL 的依赖

两者可以并存，`app/` 依然保留给有自建服务器需求的场景。

## ❓ 常见问题

<details>
<summary><b>npm install 失败，提示 "Could not find any Python installation"</b></summary>

检查：
1. `python --version` 是否能正常输出 Python 3.10+ 版本号
2. Windows 下是否关闭了 Store 的应用执行别名（`设置 → 应用 → 管理应用执行别名`）
3. Python 是否确实在 PATH 中：`where.exe python`

装了 Python 但命令无输出是 Windows 应用别名挡住了，不是 Python 没装。
</details>

<details>
<summary><b>npm install 失败，提示 MSVC / cl.exe 找不到</b></summary>

缺 Visual Studio Build Tools。安装 https://visualstudio.microsoft.com/visual-cpp-build-tools/，只需勾选 "使用 C++ 的桌面开发" 工作负载。
</details>

<details>
<summary><b>更换 Electron 版本后启动报 "NODE_MODULE_VERSION mismatch"</b></summary>

better-sqlite3 的 `.node` 二进制按旧 Electron ABI 编译，需要重新 rebuild：

```bash
npm run rebuild:sqlite
```
</details>

<details>
<summary><b>想把数据库导出 / 清空 / 重置</b></summary>

- 导出：复制 `%APPDATA%/无我笔记/woo.db`
- 清空：删除 `woo.db` 文件，下次启动会自动重建空库
- 重置：卸载应用 + 手动删除 `%APPDATA%/无我笔记/` 整个目录
</details>

## 📄 License

见仓库根目录 [LICENSE](../LICENSE)。
