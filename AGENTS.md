# Woo · 无我笔记 — Tauri 版开发指南

## 项目结构

```
Woo/
├── app-tauri/                  # Tauri v2 + Vue 3 + TypeScript (active development)
│   ├── src/                    # 前端 (Vue 3, Composition API, ESM)
│   │   ├── stores/             # Pinia stores (workspace, auth, sync, aiChat, lock, theme)
│   │   ├── services/           # IPC 客户端 api.ts + AI 服务 (gemini, deepseek, agent)
│   │   ├── components/         # Vue 组件
│   │   │   ├── layout/         # EditArea, LeftSidebar, RightSidebar, FolderTree, ...
│   │   │   └── ui/             # 通用 UI 组件
│   │   ├── config/             # 菜单、快捷键配置
│   │   └── types/              # TypeScript 类型定义
│   └── src-tauri/              # Rust 后端 (Tauri v2)
│       ├── src/
│       │   ├── commands/       # Tauri 命令 (34条，IPC入口)
│       │   ├── services/       # 业务逻辑层
│       │   ├── db/             # SQLite 连接管理 (每用户分库)
│       │   ├── supabase/       # Supabase REST API 客户端
│       │   └── models/         # 数据模型
│       └── tauri.conf.json     # Tauri 配置
└── docs/                       # 认证回调页、发布指南
```

## 开发命令

所有命令在 `app-tauri/` 目录下运行：

| 命令 | 用途 |
|------|------|
| `npm run dev` | 启动 Vite 开发服务器 |
| `npm run build` | `vue-tsc` + `vite build` |
| `npm run tauri:dev` | Tauri 开发模式 |
| `npm run tauri:build` | Tauri 生产构建 |
| `npm run tauri:android:dev` | Android 开发模式 |
| `npm run tauri:android:build` | Android 生产构建 |

## 架构概览

```
Vue 组件 → api.ts invoke() → Tauri Command (Rust) → Service → SQLite
                    ↓                                            ↓
              自动拆包 { ok, data }                         Supabase (同步时)
```

- 前端通过 `@tauri-apps/api/core` 的 `invoke()` 调用 Rust 命令
- 所有命令返回统一信封 `{ ok, data?, message? }`，前端 `api.ts` 自动拆包
- SQLite 使用 `rusqlite` (bundled)，离线优先，立即写入
- Supabase 同步每 60 秒后台进行，最后写入胜出

## 数据库

- 每用户分库：`woo.db` (本地) / `woo-{username}.db` (登录后)
- 表：`note_folder`, `note_document`, `note_document_version`, `sync_meta`, `kb_chunks`
- 位置：macOS `~/Library/Application Support/Woo/` 或 `woo-dev/` (debug)

## 同步引擎

1. 拉取墓碑 → 本地硬删除
2. 拉取远端变更 → 合并到本地 (最后写入胜出)
3. 推送本地变更 → upsert 到 Supabase
4. 清理过期 deleted=2 记录
5. 完成后发射事件通知前端刷新

## 提交规范

- 约定式提交：`feat:`, `fix:`, `chore:`, `refactor:`
- 可选 scope：`tauri`, `mobile`, `docs`, `ci`
- 标签格式：`v*`, `app-tauri-v*`
