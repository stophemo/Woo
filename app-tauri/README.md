# Woo (Tauri 重构版)

> 无我笔记 — `app-desktop` 的 Tauri v2 重构版本

`app-tauri` 是 `app-desktop`（Electron + better-sqlite3 + Supabase）的官方重构目标。
本项目使用 Tauri v2 重写主进程，业务 IPC 一一映射，UI 层沿用 Vue 3 + Pinia + Tiptap，
数据格式与 `app-desktop` 完全兼容，可平滑过渡。

---

## 为什么重构

| 维度 | `app-desktop` (Electron) | `app-tauri` (Tauri v2) |
| --- | --- | --- |
| 运行时 | Chromium + Node | 系统 WebView (WKWebView / WebView2 / WebKitGTK) |
| 安装包体积 | ~120 MB+ | ~10-20 MB |
| 内存占用 | 较高 | 显著降低 |
| 启动速度 | 较慢 | 更快 |
| 主进程语言 | JavaScript (CJS) | Rust |
| 原生能力 | 任意 Node 模块 | 通过 plugin 显式授权 |

---

## 目录结构

```
app-tauri/
├── src/                        # 渲染进程（ESM，Vue 3）
│   ├── App.vue                 # 根组件：组合三大侧栏 + 编辑区
│   ├── main.ts                 # 入口：setupTauriBridge + Pinia + mount
│   ├── setup.ts                # Tauri → window.electronAPI 桥接层
│   ├── components/
│   │   ├── layout/             # 三大侧栏、缩略图列、顶部菜单
│   │   └── settings/           # 设置、AI 配置、登录等弹窗
│   ├── stores/                 # Pinia stores
│   │   ├── workspace.ts        # 文件夹 / 文档状态
│   │   ├── auth.ts             # 登录 / 会话
│   │   ├── aiChat.ts           # AI 对话 + Agent
│   │   ├── lock.ts             # 密码锁
│   │   ├── sync.ts             # 云同步
│   │   └── theme.ts            # 主题
│   ├── services/
│   │   ├── api.ts              # ⭐ 统一 IPC 客户端
│   │   ├── folderApi.ts        # 文件夹 API 封装
│   │   ├── documentApi.ts      # 文档 API 封装
│   │   ├── versionApi.ts       # 版本历史 API
│   │   ├── lockApi.ts          # 密码锁 API
│   │   ├── agent/              # AI Agent（工具调用 / 确认）
│   │   ├── gemini.ts           # Google Gemini 客户端
│   │   ├── deepseek.ts         # OpenAI-兼容客户端
│   │   ├── assetLink.ts        # 资产链接解析
│   │   └── logger.ts           # 命名空间日志
│   ├── types/                  # 共享 DTO 类型
│   └── style.css
└── src-tauri/                  # 主进程（Rust）
    ├── Cargo.toml
    ├── tauri.conf.json         # 应用配置 + 窗口定义
    ├── capabilities/
    │   └── default.json        # 权限白名单
    └── src/
        ├── main.rs             # 入口
        ├── lib.rs              # Builder、setup 钩子
        ├── commands/           # Tauri commands（IPC 处理器）
        │   ├── mod.rs
        │   ├── auth.rs         # auth:*
        │   ├── folder.rs       # folder:*
        │   ├── document.rs     # document:*
        │   ├── version.rs      # version:*
        │   ├── lock.rs         # lock:*
        │   ├── system.rs       # app:*, dialog:*
        │   ├── kb.rs           # kb:*
        │   └── sync.rs         # sync:*
        ├── services/           # 业务服务
        │   ├── folder_service.rs
        │   ├── document_service.rs
        │   ├── version_service.rs
        │   ├── lock_service.rs
        │   ├── auth_service.rs
        │   ├── kb_service.rs
        │   ├── embedding_service.rs
        │   └── sync_engine.rs
        ├── db/                 # SQLite 连接 + Schema 迁移
        ├── models/             # DTO / 表结构
        └── supabase/           # 云同步客户端（占位）
```

---

## IPC 约定

`app-desktop` 使用 Electron 的 `ipcMain.handle('channel:action', …)` 模式：
- 通道名是 `domain:action` 形式
- 主进程 `wrap()` 把所有响应包成 `{ ok, data, message }`

Tauri v2 没有内建 channel / wrap 机制，所以 `app-tauri` 在前端用一层薄适配模拟同样的接口：

```
调用 invoke('document:listByFolder', { folderId })
   ↓ toCamelCase
   documentListByFolder
   ↓ @tauri-apps/api/core → invoke
   Rust #[tauri::command(rename_all = "camelCase", rename = "documentListByFolder")]
   ↓ 业务返回 { ok, data, message }
前端 services/api.ts 自动 unwrap，!ok 时 throw
```

### 通道命名规则

- 命名空间：`document`、`folder`、`auth`、`lock`、`version`、`system`、`app`、`kb`、`sync`
- 格式：`namespace:action`（动作用驼峰），例如 `document:listByFolder`
- 前端传给 `invoke` 的参数必须是单个对象（不是位置参数），键名与 Rust 函数参数同名
  - 例：`invoke('document:search', { keyword: 'foo', limit: 10 })`
- Rust 端必须用 `#[tauri::command(rename_all = "camelCase")]` 让 snake_case 参数自动转换为 camelCase

### `services/api.ts` 行为

| 入参 | 发送负载 |
| --- | --- |
| `invoke('folder:tree')` | `undefined` |
| `invoke('document:get', { id })` | `{ id }` |
| 返回 `{ ok, data, message }` | 自动解包 `data`，`!ok` 抛 `Error(message)` |
| 返回原生值（数字、字符串、null） | 原样透传 |

> **禁止** 在业务代码里直接 `import { invoke } from '@tauri-apps/api/core'` —— 一律走 `services/api.ts`，
> 这样才能享受 `wrap()` 自动解包、错误信息中文本地化等收益。

---

## 主窗口配置

主窗口在 `src-tauri/tauri.conf.json` 中声明（不再在 `setup` 中手动创建），由 Tauri 自动创建：

- dev 模式 → `devUrl: http://localhost:5173`
- prod 模式 → `frontendDist: ../dist`（经过 `vite build`）
- macOS 样式：hidden title + overlay + traffic light 自定义位置

> ⚠️ 早期重构版曾在 `setup` 中通过 `WebviewWindowBuilder` 创建窗口，并用 `TAURI_DEV` 环境变量切换 URL。
> 但 Tauri v2 CLI 不会自动设置该变量，导致 dev 模式指向 `tauri://localhost/`，渲染进程无法加载，出现白屏。
> **当前版本已改为在 `tauri.conf.json` 中声明窗口**，由 Tauri 自动选择正确的 URL。

---

## 开发与构建

| 命令 | 作用 |
| --- | --- |
| `npm install` | 安装前端依赖 + postinstall 重建原生模块 |
| `npm run dev` | 启动 Vite dev server（`localhost:5173`） |
| `npm run tauri:dev` | 启动 Tauri 开发模式（自动调用 `npm run dev`） |
| `npm run build` | `vue-tsc --noEmit && vite build`（仅前端） |
| `npm run tauri:build` | 完整打包（含 Rust release 编译） |
| `npm run tauri:icon` | 重新生成应用图标 |

> **Tip**: `tauri:dev` 首次运行需要编译大量 Rust crate，请耐心等待；后续增量编译很快。

---

## 重要环境变量

读取自仓库根目录的 `.env`（vite 通过 `envDir: '..'` 加载）：

| 变量 | 作用 |
| --- | --- |
| `WOO_LOG` | Rust 端日志级别（`info` / `debug` / `trace`），默认 `info` |
| `ELECTRON_DEVTOOLS` | **当前版本不生效**（保留自旧 Electron 版本） |

---

## 与 `app-desktop` 的差异

### 已对齐

- 文件夹 / 文档 CRUD、版本历史、密码锁、回收站的 IPC 通道全部一一对应
- SQLite 表结构、字段命名保持兼容（`create_time` / `update_time` 仍为 snake_case）
- UI 层组件、Pinia store 接口、消息总线事件名沿用同一套
- Tiptap 编辑器、Markmap 脑图、AI Agent 工具调用保持一致

### 待完成

- `auth_service` 仍为占位实现（`signIn` / `signOut` 仅返回 mock session）
- `supabase` 模块为占位，云同步暂未启用
- `embedding_service` / `kb_service` 仍为占位（知识库 RAG 暂未生效）
- 自动更新尚未实现
- 原生菜单（macOS App Menu）暂未配置
- Windows / Linux 平台的 code signing 未配置

### 风格差异

- `app-desktop` 用 CJS（`.cjs`），本项目前端沿用 ESM（`.ts` / `.vue`）+ Vite
- 主进程不再有 `wrap()` 函数，由前端 `services/api.ts` 承担等价职责
- IPC 调用不再是位置参数，必须是对象形式（更类型友好）

---

## 故障排查

### 启动后白屏

确认 `src-tauri/tauri.conf.json` 的 `app.windows` 至少含一个名为 `main` 的窗口，
且 `capabilities/default.json` 的 `windows: ["main"]` 与之匹配。
Tauri 不会为未在配置或代码中创建的窗口渲染任何内容。

### 找不到 channel

- 前端用 `'document:listByFolder'`，Rust 命令必须为 `#[tauri::command(rename = "documentListByFolder")]`
- 确认 `commands::xxx` 模块在 `lib.rs` 的 `tauri::Builder::invoke_handler` 中已注册

### 权限被拒

在 `capabilities/default.json` 的 `permissions` 中追加对应权限集，例如：

```json
"permissions": [
  "core:default",
  "dialog:default",
  "shell:default"
]
```

### 数据库未初始化

主进程 `setup` 钩子会调用 `db::set_data_dir(...)` 和 `db::with_db(...)`。
若路径不可写，应用会 panic。macOS 默认数据目录为
`~/Library/Application Support/com.nonegonotes.woo.tauri/`。

---

## 路线图

- [ ] 完成 `auth_service`（接入 Supabase Auth）
- [ ] 完成 `sync_engine`（基于 `app-desktop` 的 last-write-wins + tombstone 同步）
- [ ] 完成知识库 / 嵌入检索（`embedding_service` + `kb_service`）
- [ ] macOS 原生菜单 + 完整快捷键
- [ ] Windows / Linux 平台打包与签名
- [ ] 移动端（参照 `app-mobile`）

---

## 许可

MIT
