<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="128" height="128" alt="Woo Notes">
</p>

<h1 align="center">无我笔记 Woo</h1>

<p align="center">
  <strong>本地优先 · 跨平台 Markdown 笔记应用</strong>
</p>

<p align="center">
  <a href="https://github.com/stophemo/Woo/releases"><img src="https://img.shields.io/github/v/release/stophemo/Woo?color=%235a9acf" alt="Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="License"></a>
  <a href="https://github.com/stophemo/Woo/releases"><img src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Android-blue" alt="Platform"></a>
  <a href="https://github.com/stophemo/Woo/stargazers"><img src="https://img.shields.io/github/stars/stophemo/Woo?style=social" alt="Stars"></a>
</p>

<p align="center">
  <a href="https://woo-notes.vercel.app">🌐 官网</a> ·
  <a href="https://github.com/stophemo/Woo/releases/latest">📦 下载</a> ·
  <a href="#-开发">🛠️ 开发</a> ·
  <a href="#-路线图">🗺️ 路线图</a>
</p>

---

## ✨ 为什么选择 Woo

Woo（无我笔记）是一款本地优先的 Markdown 笔记应用。无需注册即可在本机离线写作；需要跨设备时，再登录并同步到 macOS、Windows 与 Android。

| | |
|---|---|
| 💾 **本地优先** | 正式文稿写入本机 SQLite，草稿保存在应用本地；断网也能浏览和编辑 |
| ☁️ **可选同步** | 注册并登录后通过 Supabase 增量同步，采用最后写入胜出与冲突副本保护 |
| 📝 **桌面编辑** | Tiptap 所见即所得编辑器，支持 Markdown 输入与粘贴、表格、大纲和思维导图 |
| 📱 **移动编辑** | Android 针对小屏提供 Markdown 编辑、搜索、分享、回收站与版本回溯 |
| ⏱️ **版本历史** | 桌面端自动记录、Android 保存时记录，也可手动建立和恢复正式文稿版本 |
| 📤 **文件与导出** | 桌面端可读写 Markdown / TXT 文件，并将文稿导出为 Markdown、纯文本或 PNG |
| 🔒 **应用内访问锁** | 可锁定文件夹或文稿；用于限制应用内查看，不等同于磁盘加密 |
| 🎨 **极简设计** | 温暖柔和的日间主题 + 护眼暗色主题 |

---

## 📦 下载

| 平台 | 下载 | 说明 |
|------|------|------|
| 🍎 macOS | [DMG (Apple Silicon)](https://github.com/stophemo/Woo/releases/latest) | 适用于 M 系列 Mac；签名与公证状态以对应 Release 说明为准 |
| 🪟 Windows | [NSIS 安装包 (x64)](https://github.com/stophemo/Woo/releases/latest) | 双击安装 |
| 🤖 Android | [已签名 APK (ARM64)](https://github.com/stophemo/Woo/releases/latest) | 支持 `arm64-v8a`，安装时允许来自此来源的应用 |

> **macOS 用户**：安装前请阅读对应 Release 说明。若版本未完成 Apple 公证，首次启动请在 Finder 中右键 Woo.app 并选择“打开”，或前往“系统设置 → 隐私与安全”选择“仍要打开”。无需、也不要安装任何第三方根证书。

---

## 🚀 快速开始（用户）

1. 前往 [Releases](https://github.com/stophemo/Woo/releases/latest) 下载对应平台安装包
2. 安装并启动
3. 开始写作，文稿会优先保存在当前设备
4. （可选）注册或登录 Woo 账号，启用跨设备云同步

---

## 🛠️ 开发

### 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | [Tauri v2](https://v2.tauri.app/) (Rust) |
| 前端 | Vue 3 + Pinia + Vite + TypeScript |
| 编辑器 | Tiptap (ProseMirror) |
| 数据库 | SQLite (rusqlite, bundled) |
| 云服务 | Supabase (Auth + REST API) |
| 移动端 | Tauri Mobile (Android；iOS 计划中) |

### 项目结构

```
Woo/
├── src/                    # 桌面端前端 (Vue 3, ESM)
│   ├── stores/             # Pinia 状态管理 (workspace, auth, sync, lock…)
│   ├── services/           # IPC 客户端、更新、Markdown 转换与导出
│   ├── components/         # 桌面端 UI 组件 (layout/, settings/)
│   └── types/              # TypeScript 类型定义
├── src-mobile/             # 移动端前端 (Vue 3 + Vant UI)
│   ├── views/              # 移动端页面
│   └── router/             # 移动端路由
├── src-tauri/              # Rust 后端 (桌面端 + 移动端共用)
│   ├── src/
│   │   ├── commands/       # Tauri IPC 命令入口
│   │   ├── services/       # 业务逻辑 (folder, document, sync_engine…)
│   │   ├── db/             # SQLite 连接管理 + Schema 迁移
│   │   ├── supabase/       # Supabase REST API 客户端
│   │   └── models/         # 数据模型
│   └── gen/android/        # Android 项目骨架
├── landing/                # 官网落地页 (部署于 Vercel)
├── index.html              # 桌面端入口
├── index-mobile.html       # 移动端入口
├── vite.config.ts          # 桌面端 Vite 配置
└── vite.mobile.config.ts   # 移动端 Vite 配置
```

### 构建命令

```bash
# 桌面端
npm install                  # 安装前端依赖
npm run dev                  # 启动 Vite 开发服务器 (localhost:5173)
npm run tauri:dev            # Tauri 开发模式（自动启动 Vite + Rust 编译）
npm run build                # vue-tsc + vite build（仅前端）
npm run tauri:build          # 生产构建（含 Rust release 编译）

# 移动端
npm run dev:mobile           # 移动端 Vite 服务器 (localhost:5174)
npm run tauri:android:dev    # Android 开发模式（连接真机）
npm run tauri:android:build  # Android 生产构建
```

### IPC 约定

```
Vue 组件 → api.ts invoke() → Tauri Command (Rust) → Service → SQLite
                    ↓                                             ↓
              自动拆包 { ok, data }                          Supabase (同步时)
```

- 所有 Rust 命令返回 `CommandResult<T> { ok, data?, message? }`
- 前端 `api.ts` 自动解包：`ok=false` 抛异常，原始值直接透传
- IPC 格式：`namespace:action`（如 `document:listByFolder`），参数为对象

> ⚠️ 禁止直接 `import { invoke }` —— 一律走 `services/api.ts`

### 数据库

未登录 → `woo.db`，登录后 → `woo-{username}.db`（首次登录自动迁移）。无文件夹草稿使用 `localStorage`，不进入 SQLite，也不参与云同步和版本历史。

| 表 | 说明 |
|---|---|
| `note_folder` | 文件夹树，`parent_id` 层级结构，软删除三态 |
| `note_document` | 文档，HTML `content`，标题从第一行自动提取 |
| `note_document_version` | 版本历史 (auto / manual / restore) |
| `sync_meta` | 键值存储 (last_sync_time, last_tombstone_pull) |

**软删除三态**：`deleted = 0` 正常 → `1` 回收站 → `2` 待硬删除（7 天清理窗口）

### 同步引擎

登录后，同步引擎以 60 秒间隔在后台运行，也支持手动触发。流程：拉墓碑 → 拉远端 → 推本地 → 清理 → 墓碑 GC。采用最后写入胜出 + 增量同步 + 冲突副本保护策略。

---

## 🗺️ 路线图

- [x] 云同步 (Supabase Auth + Sync Engine)
- [ ] macOS 原生菜单 + 完整快捷键
- [ ] Homebrew Cask 分发
- [ ] iOS 支持 (Tauri Mobile)
- [ ] Windows 代码签名
- [x] 跨平台基础框架 (macOS + Windows + Android)
- [ ] 完成 macOS Developer ID 签名与 Apple 公证
- [x] 移动端响应式 UI 适配
- [ ] AI 辅助写作与 Agent 框架

---

## 📄 许可

[MIT](LICENSE) © 2025–2026 Woo Notes

---

<p align="center">
  <sub>🏗️ 用 Rust + Vue 3 构建 · 跑在 Tauri v2 上</sub>
</p>
