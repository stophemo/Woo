# CLAUDE.md

请始终使用简体中文与我对话，并在回答时保持专业、简洁。
本文件为 Claude Code (claude.ai/code) 提供使用此仓库时的指导。

## 构建与开发命令

```bash
cd app-tauri
npm install              # 安装前端依赖
npm run dev              # 启动 Vite 开发服务器（http://localhost:5173）
npm run build            # vue-tsc + vite build（仅前端）
npm run tauri:dev        # Tauri 开发模式（桌面端）
npm run tauri:build      # Tauri 生产构建（桌面端）
npm run tauri:android:dev    # Android 开发模式
npm run tauri:android:build  # Android 生产构建
```

## 架构

**Woo（无我笔记）** 是一款本地优先的 Markdown 笔记应用，基于 **Tauri v2** (Rust) + **Vue 3** 构建。使用 SQLite (rusqlite, bundled) 作为主要数据存储，可选 Supabase 云同步。

### IPC 层

```
Vue 组件 → api.ts invoke() → Tauri Command (Rust) → Service → SQLite (本地)
                    ↓                                             ↓
              自动拆包 { ok, data }                          Supabase (同步时)
```

- 所有 IPC 通过 `@tauri-apps/api/core` 的 `invoke()` 调用 Rust 注解的 `#[tauri::command]`
- 所有 Rust 命令返回统一信封 `CommandResult<T> { ok, data?, message? }`
- 前端 `api.ts` 自动拆包：`ok=false` 时抛出异常；原始值直接透传
- `window.electronAPI` 为 Electron 兼容桥接，用于窗口控制等原生操作

### 项目结构

```
Woo/
├── app-tauri/                  # Tauri v2 主项目
│   ├── src/                    # 前端 (Vue 3, ESM)
│   │   ├── stores/             # Pinia 状态管理 (workspace, auth, sync, aiChat, lock, theme)
│   │   ├── services/           # IPC 客户端 + AI 服务 (gemini.ts, deepseek.ts, agent/)
│   │   ├── components/         # Vue 组件 (layout/, ui/, icons/)
│   │   ├── config/             # 菜单、快捷键配置
│   │   └── types/              # TypeScript 类型
│   └── src-tauri/              # Rust 后端 (Tauri v2)
│       ├── src/
│       │   ├── commands/       # 34 条 Tauri 命令 (IPC 入口)
│       │   ├── services/       # 业务逻辑层
│       │   ├── db/             # SQLite 连接管理 (每用户分库)
│       │   ├── supabase/       # Supabase REST API 客户端
│       │   └── models/         # 数据模型
│       └── gen/android/        # Android 项目 (生成)
└── docs/                       # 认证回调页、发布指南
```

### 关键架构决策

- **本地优先，可选云同步**：SQLite (rusqlite) 是主要数据存储。Supabase 同步采用最后写入胜出、增量推拉和墓碑标记删除传播。
- **按用户分库**：未登录 → `woo.db`。登录后 → `woo-{username}.db`。首次登录将本地数据复制到用户数据库。
- **内容编辑**：Tiptap (ProseMirror) 所见即所得编辑器。内容存储为 HTML。标题从第一行自动提取。
- **草稿系统**：在无文件夹状态下创建的文档存储在 localStorage 而非数据库中。草稿 ID 前缀为 `draft_`。
- **冲突处理**：同步检测到远程更改与本地未保存编辑冲突 → 将本地版本保存为 `_conflict` 副本，加载远程版本。
- **窗口样式**：macOS 原生标题栏隐藏 + 交通灯按钮覆盖。
- **三端共用代码**：一套 Rust + Vue 代码，编译到桌面 (macOS/Windows/Linux) + Android + iOS。

### 同步引擎设计 (`src-tauri/src/services/sync_engine.rs`)

1. **拉取墓碑** — 从 Supabase 读取 `sync_tombstone` 表 → 硬删除匹配的本地行 + 级联删除版本
2. **拉取远端更改** — 读取 Supabase 中 `update_time > last_sync_time` 的记录 → 合并到本地 SQLite（最后写入胜出：若 `local.update_time >= remote.update_time` 则本地胜出）
3. **推送本地更改** — 读取本地 `update_time > last_sync_time` 的记录 → upsert 到 Supabase（过滤本轮 echo 行）
4. **清理** — 扫描本地超过清理窗口的 `deleted=2` 记录 → 从两个数据库中硬删除 + 写入墓碑到 Supabase
5. **墓碑垃圾回收** — 回收 Supabase 中超过 30 天的墓碑

同步每 60 秒运行一次。完成后通过 Tauri 事件系统通知前端刷新。

### 软删除三态方案

- `deleted = 0` — 正常
- `deleted = 1` — 在回收站（可在回收站视图中查看、恢复）
- `deleted = 2` — 待清理（超过清理窗口后将硬删除）

### 状态管理模式（Pinia）

- **workspace store** — 文件夹/文档操作，800ms 防抖自动保存，同步刷新时差异合并，冲突时创建 `_conflict` 文档
- **auth store** — 会话恢复（Supabase → 7 天 localStorage 降级）
- **sync store** — 监听 Tauri 事件桥接的 `sync-status` CustomEvent
- **lock store** — 密码锁状态管理
- **aiChat store** — AI 聊天会话管理

### AI 集成

- `src/services/gemini.ts` — Google Gemini API 客户端
- `src/services/deepseek.ts` — DeepSeek / OpenAI 兼容 API 客户端
- `src/services/agent/` — AI 代理框架（8 个内置工具）
- `src/stores/aiChat.ts` — AI 聊天状态管理
- 用户在设置中配置 API 密钥（不捆绑密钥）

### 关键数据流

1. `App.vue` 挂载 → `authStore.bootstrap()` 恢复会话 → `workspaceStore.bootstrap()` 加载文件夹树
2. 用户选择文件夹 → `workspaceStore.selectFolder()` → 通过 IPC 加载文档列表
3. 用户选择文档 → `workspaceStore.selectDocument()` → 通过 IPC 加载完整内容
4. 编辑器编辑 → 800ms 防抖自动保存，重大更改时自动提交版本
5. 同步引擎（后台任务，60 秒间隔）推送/拉取更改
6. 同步完成 → Tauri 事件 → DOM CustomEvent → workspaceStore.syncRefresh()

### 数据库模式（SQLite）

- `note_folder` — 文件夹树，通过 `parent_id` 层级结构，软删除
- `note_document` — 文档，HTML `content`，软删除
- `note_document_version` — 版本历史（auto/manual/restore）
- `sync_meta` — 键值存储（last_sync_time, last_tombstone_pull）
- `kb_chunks` — 知识库分块（RAG 数据源）

### 构建工具

- Vite 构建前端 → `dist/`
- Tauri v2 编译 Rust 后端，打包为各平台原生应用
- Android 通过 `tauri android init/build`，iOS 通过 `tauri ios init/build`
- `rusqlite` 使用 bundled feature 自带 SQLite C 源码编译

## 代码规范

- TypeScript 严格模式，启用 `noUnusedLocals` 和 `noUnusedParameters`
- 约定式提交：`feat:`、`fix:`、`chore:`、`refactor:`，可选作用域如 `feat(tauri):`
- Supabase URL 和匿名密钥已提交（设计上公开，RLS 保护数据安全）
- 全项目使用 Vue 3 Composition API（`<script setup lang="ts">`）
- Rust 命令返回 `CommandResult<T>`，前端 `api.ts` 自动拆包
- Pinia store 使用 Composition API 风格（`defineStore('name', () => { ... })`）

## 注意事项

- **废弃目录**：`app-desktop/` 和 `app-mobile/` 已迁移至 `electron` 分支。请勿在主分支上修改。
- **`.env`** 中 `SYNC_CLEANUP_SECONDS=60*60*24*7` — 7 天清理周期
- 根目录 `vercel.json` 用于认证成功页面的部署，与应用本身无关

## Git 与发布

- 发布标签格式：`v*` 或 `app-tauri-v*`
- 推送标签触发 GitHub Actions，构建桌面端 + Android APK
- macOS：无代码签名（`CSC_IDENTITY_AUTO_DISCOVERY: "false"`）
- 远端使用 HTTPS；使用 `gh auth git-credential` 辅助工具
- CI 从标签间的约定式提交自动生成发布说明

## 交互规则

- 始终使用简体中文对话，保持专业、简洁
