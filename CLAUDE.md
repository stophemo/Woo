# CLAUDE.md

请始终使用简体中文与我对话，并在回答时保持专业、简洁。
本文件为 Claude Code (claude.ai/code) 提供使用此仓库时的指导。

## 构建与开发命令

```bash
cd app-desktop
npm install              # 安装依赖；postinstall 自动执行 electron-builder install-app-deps
npm run dev              # 启动 Vite 开发服务器（渲染器热重载于 http://localhost:5173）
npm run preview          # Vite 预览构建后的渲染器
npm run electron:dev     # Electron 开发模式（加载 Vite 开发服务器 URL）。需要先运行 `npm run dev`
npm run build            # vue-tsc + vite build（仅前端，不含 Electron 打包）
npm run electron:build   # vue-tsc + vite build + electron-builder（完整发布）
npm run rebuild:sqlite   # Electron 版本升级后，electron-rebuild better-sqlite3
npm run build:icons      # 从源文件生成应用图标

# 开发工作流 — 两个并发终端：
#   终端 1: npm run dev          （保持 Vite 开发服务器运行）
#   终端 2: npm run electron:dev （启动 Electron，从 Vite 加载）

# 本项目零测试 — 不存在测试运行器或测试文件。
# 构建输出 → app-desktop/release/
#   woo-desktop-{version}-win-x64-setup.exe  （NSIS 安装包）
#   woo-desktop-{version}-win-x64-portable.exe
#   woo-desktop-{version}-win-x64.zip
```

## 架构

**Woo（无我笔记）** 是一款本地优先的 Markdown 笔记桌面应用，基于 Electron + Vue 3 构建。使用 SQLite (better-sqlite3) 作为主要数据存储，可选 Supabase 云同步。

### IPC 层

```
Renderer (Vue)  →  window.woo.invoke(channel, ...args)  →  ipcMain.handle  →  Service  →  SQLite/Supabase
                     │                                      │
                     │  Returns { ok, data }                │  wrap() catches errors
                     │  or { ok: false, message }           │  and returns { ok: false, message }
                     ▼                                      ▼
              api.ts  unwrap()                         register.cjs  wrap()
              throws on !ok                             catches throws
```

- 所有渲染器到主进程的 IPC 调用通过 `preload.cjs` contextBridge 暴露的 `window.woo.invoke()` 进行
- `electron/ipc/register.cjs` 中的 IPC 处理函数使用 `wrap()` 辅助函数，捕获抛出的错误 → 返回 `{ ok: false, message }`
- 渲染器侧的 `api.ts` 有对应的 `unwarp()` 函数，在 `ok: false` 时抛出异常，使 service 函数调用看起来像普通异步调用
- `window.electronAPI` 是独立的 contextBridge 插槽，用于 Electron 原生操作（窗口控制、菜单操作、更新检查）

### 项目结构

```
Woo/
├── app-desktop/                # Electron + Vue 3 桌面应用
│   ├── electron/               # 主进程（CJS 模块）
│   │   ├── main.cjs            # 入口：窗口创建、菜单、IPC 注册、数据库初始化、同步启动
│   │   ├── preload.cjs         # contextBridge API（electronAPI + woo.invoke）
│   │   ├── ipc/register.cjs    # 所有 IPC 处理函数注册（wrap() 模式）
│   │   ├── db/
│   │   │   ├── index.cjs       # 连接管理（按用户分库、自动切换）
│   │   │   └── schema.cjs      # 模式初始化 + ALTER TABLE 迁移
│   │   ├── services/
│   │   │   ├── folderService.cjs, documentService.cjs, versionService.cjs
│   │   │   ├── authService.cjs        # Supabase 认证封装
│   │   │   └── syncEngine.cjs         # Supabase 同步引擎
│   │   └── config/supabase.cjs        # Supabase 客户端单例
│   └── src/                    # 渲染器进程（ESM, Vue 3）
│       ├── stores/              # Pinia 状态管理：workspace, auth, sync, aiChat, theme
│       ├── services/            # IPC 客户端封装（api.ts）+ AI 客户端（gemini.ts, deepseek.ts）
│       ├── components/
│       │   ├── layout/          # 应用外壳：EditArea, FolderTree, LeftSidebar, ThumbnailColumn, ...
│       │   ├── ui/              # 可复用 UI 组件（ContextMenu, Dropdown, UpdateNotification）
│       │   └── icons/           # SVG 图标组件
│       ├── config/menus.ts      # 菜单定义
│       └── types/               # TypeScript 接口（Document, FolderNode, ChatMessage 等）
└── app-mobile/                  # Flutter 移动端应用（占位）
```

### 关键架构决策

- **本地优先，可选云同步**：SQLite (better-sqlite3) 是主要数据存储。Supabase 同步采用最后写入胜出、增量推拉和墓碑标记删除传播。
- **按用户分库**：未登录 → `woo.db`。登录后 → `woo-{username}.db`。首次登录将本地数据复制到用户数据库。切换用户时自动断开/重连。
- **内容编辑**：Tiptap (ProseMirror) 所见即所得编辑器。内容存储为 HTML。标题从第一行自动提取。
- **草稿系统**：在无文件夹状态下创建的文档存储在 localStorage 而非数据库中。草稿 ID 前缀为 `draft_`。
- **冲突处理**：同步检测到远程更改与本地未保存编辑冲突 → 将本地版本保存为 `_conflict` 副本，加载远程版本。
- **版本控制**：重大更改时自动提交版本（100 字符或 10 行差异）。24 小时/7 天分层合并策略。
- **窗口样式**：Windows 无边框（自定义标题栏），macOS 原生标题栏隐藏 + 交通灯按钮覆盖。

### 同步引擎设计（`electron/services/syncEngine.cjs`）

1. **拉取墓碑** — 从 Supabase 读取 `sync_tombstone` 表（比上次拉取更新的记录）→ 硬删除匹配的本地行 + 级联删除版本
2. **拉取远端更改** — 读取 Supabase 中 `update_time > last_sync_time` 的记录 → 合并到本地 SQLite（最后写入胜出：若 `local.update_time >= remote.update_time` 则本地胜出）
3. **推送本地更改** — 读取本地 `update_time > last_sync_time` 的记录 → 写入（upsert）到 Supabase
4. **清理** — 扫描本地超过清理窗口的 `deleted=2` 记录 → 从两个数据库中硬删除 + 写入墓碑到 Supabase
5. **墓碑垃圾回收** — 回收 Supabase 中超过 30 天的墓碑

同步每 60 秒运行一次。认证状态变更触发立即同步 + 数据库切换。

### 软删除三态方案

- `deleted = 0` — 正常
- `deleted = 1` — 在回收站（可在回收站视图中查看、恢复）
- `deleted = 2` — 待清理（超过清理窗口后将硬删除）

### 状态管理模式（Pinia）

- **workspace store** — 文件夹/文档操作的乐观更新（出错时回滚），800ms 防抖自动保存内容，同步刷新时的差异合并（基于 diff，保持 Vue 响应式），同步冲突时创建 `_conflict` 文档
- **auth store** — 会话持久化到 localStorage，支持 Electron 重启恢复
- **sync store** — 监听通过 preload 从主进程转发的 `sync-status` CustomEvent

### AI 集成

- `src/services/gemini.ts` — Google Gemini API 客户端
- `src/services/deepseek.ts` — DeepSeek API 客户端
- `src/stores/aiChat.ts` — AI 聊天状态管理
- `app-desktop/src/components/layout/RightSidebar.vue` — AI 聊天面板
- 用户在设置中配置 API 密钥（不捆绑密钥）

### 关键数据流

1. `App.vue` 挂载 → `authStore.bootstrap()` 恢复会话 → `workspaceStore.bootstrap()` 加载文件夹树
2. 用户选择文件夹 → `workspaceStore.selectFolder()` → 通过 IPC 加载文档列表
3. 用户选择文档 → `workspaceStore.selectDocument()` → 通过 IPC 加载完整内容
4. 编辑器编辑 → 通过 IPC 进行 800ms 防抖自动保存，重大更改时自动提交版本
5. 同步引擎（主进程，60 秒间隔）推送本地更改并从 Supabase 拉取远端更改
6. 同步完成 → 主进程发送 `sync:data-changed` → preload 转发为 CustomEvent → workspaceStore.syncRefresh() 进行差异合并

### 数据库模式（SQLite）

- `note_folder` — 文件夹树，通过 `parent_id` 实现层级结构，通过 `deleted` 实现软删除
- `note_document` — 文档，包含 `folder_id`、HTML `content`，软删除
- `note_document_version` — 版本历史，包含自动/手动/恢复变更类型
- `sync_meta` — 键值存储，用于同步状态（last_sync_time, last_tombstone_pull）

### 构建工具

- `vite-plugin-electron/simple` 编译 Electron 主进程 + preload。但 preload 直接从源码加载（`electron/preload.cjs`），而非编译后的输出，以避免 CJS/ESM 兼容性问题。
- electron-builder 配置支持 Windows（NSIS/便携版/zip）、macOS（dmg/zip）和 Linux（AppImage/deb）。
- better-sqlite3 需要在 Electron 版本变更后执行 `asarUnpack` + `electron-rebuild`。

## 代码规范

- Electron 主进程使用 `.cjs`（CommonJS）；渲染器使用 ESM（`"type": "module"`）
- TypeScript 严格模式，启用 `noUnusedLocals` 和 `noUnusedParameters`
- 约定式提交：`feat:`、`fix:`、`chore:`、`release:`，可选作用域如 `feat(app-desktop):`
- Supabase URL 和匿名密钥已提交（设计上公开，RLS 保护数据安全）
- 全项目使用 Vue 3 Composition API（`<script setup lang="ts">`）
- 所有 IPC service 函数返回原始数据（错误通过异常抛出）。IPC 包装器（`wrap()`）转换为 `{ ok, data/message }` 格式
- Pinia store 使用 Composition API 风格（`defineStore('name', () => { ... })`）

## 注意事项

- **根目录下的 `AGENTS.md`** 是已被取代的旧指导文件 — 应删除
- **`.env`** 中 `SYNC_CLEANUP_SECONDS=50` — 这是异常短的清理间隔（可能用于调试）。生产环境应调整为 `60*60*24*7` = 7 天
- 根目录 `vercel.json` 用于认证成功页面的部署，与桌面应用本身无关

## Git 与发布

- 发布标签格式：`v*`、`Woo-desktop-v*` 或 `app-desktop-v*`
- 推送标签触发 GitHub Actions（`release-app-desktop.yml`），构建 macOS + Windows 版本（5 个构建产物）
- macOS：无代码签名（`CSC_IDENTITY_AUTO_DISCOVERY: "false"`）
- 远端使用 HTTPS；使用 `gh auth git-credential` 辅助工具
- CI 从标签间的约定式提交自动生成发布说明

## 交互规则

- 始终使用简体中文对话，保持专业、简洁
