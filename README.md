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
  <a href="https://github.com/stophemo/Woo/releases"><img src="https://img.shields.io/badge/platform-macOS%20%7C%20Android-blue" alt="Platform"></a>
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

Woo（无我笔记）是一款本地优先的 Markdown 笔记应用。无需账号即可在本机离线写作；需要跨设备时，每台设备填写同一个 WebDAV 账号即可自动建立或加入同步空间。

> **v0.8.5**：WebDAV 同步改为“填完账号直接同步”，不再需要六位配对码；加入空间不再逐条扫描远端操作；新增全局请求闸门、变更门闩和 6 小时心跳，显著减少无效请求和 429 限流。

| | |
|---|---|
| 💾 **单库本地优先** | 无需登录/注册，启动直接打开本机 `woo.db`；正式文稿写入 SQLite，草稿留在本地，断网也能浏览和编辑 |
| 🔗 **WebDAV 同步空间** | 任意 HTTPS WebDAV（坚果云、Nextcloud、群晖等）；每台设备填写同一账号即自动同步；正文与图片加密后上传 |
| 🧩 **大文档分块** | 长文和内嵌图片按内容定义边界分块、压缩、加密；只上传变化块，单条操作不超过 32 KiB |
| 🔤 **字体系统** | 界面统一使用内置「Noto Sans SC / Woo Sans」（真实 400–700 可变字重）；正文可选文楷（内置霞鹜文楷）、黑体、宋体、等宽；移动端 Aa 面板可调字号 |
| 📝 **桌面编辑** | Tiptap 所见即所得编辑器，支持 Markdown 输入与粘贴、表格、大纲和思维导图 |
| 📱 **移动编辑** | Android 层级目录 + 草稿箱；浮动格式条与 Aa 面板，自动保存，支持搜索、移动到目录、分享、回收站与版本回溯 |
| ⏱️ **版本历史** | 桌面端与 Android 均自动记录版本，也可手动建立和恢复正式文稿版本 |
| 📤 **文件与导出** | 桌面端可读写 Markdown / TXT 文件，并将文稿导出为 Markdown、纯文本或 PNG |
| 🔒 **应用内访问锁** | 可锁定文件夹或文稿；用于限制应用内查看，不等同于磁盘加密 |
| 🔌 **MCP 服务** | 桌面端可选开启；让 Claude Code、Codex、Hermes、Pi 等外部 AI 直接调用 Woo 维护笔记，默认只读，写入需手动开启 |
| 🎨 **极简设计** | 温暖柔和的日间主题 + 护眼暗色主题 |

---

## 🔗 v0.8.4：WebDAV 同步空间

| 关注点 | 说明 |
|---|---|
| **怎么开始** | 每台设备只在「跨设备」填写同一个 WebDAV 账号（服务商预设自动填地址）→ 保存即同步 |
| **安全边界** | 正文和图片 AES-256-GCM 加密，WebDAV 只存密文；新设备用应用密码解开云端密钥槽，因此持有应用密码的人（含供应商）可以解密 |
| **配对** | 默认不需要配对码；需要云端不可恢复密钥时，开启严格模式并用六位码配对 |
| **防误建** | 云端已有空间时不会重复建立，自动加入或提示确认 |
| **大文档** | 正文和内嵌图片按内容定义边界分块、压缩、加密；只上传变化块，单条操作不超过 32 KiB |
| **及时性** | 启动、本地修改、回到前台和每 15 分钟兜底时同步；WebDAV 没有服务端推送，**不是实时同步** |
| **设备列表** | 加密心跳只读展示设备名、平台、最近同步和版本；没有移除按钮，不能远程吊销；退出空间只清本机身份 |
| **加入基准** | 本机没有数据时直接加入；有文稿或已有空间时，必须选择以对方为准（对方批准后清空本机并加入）或以本机为准（取消并让对方输入本机配对码） |
| **旧数据** | 旧 WebDAV 账号自动迁移；旧 Worker / 局域网凭据保留但不启用；旧设备需重新配对 |

完整规则、配对协议和迁移说明见 [docs/domestic-sync.md](docs/domestic-sync.md)。
如果无我笔记与无我待办共用同一个 WebDAV 账号，配对目录是账号级共享的；建议使用不同账号或不同子路径。

## 🤖 MCP：让外部 AI 维护你的笔记

Woo 本身**不内置 AI 聊天**，也不绑定某一家模型。它把笔记能力通过 **MCP（Model Context Protocol）** 开放给外部 AI 工具：由你已经在用的 Claude Code、Codex、Hermes、Pi 等 AI 来调用 Woo，搜索、读取、新建、更新、追加、移动和整理笔记。

| 关注点 | 说明 |
|---|---|
| **谁来调用** | Claude Code、Codex、Hermes、Pi，以及任何支持 Streamable HTTP 的 MCP 客户端 |
| **能做什么** | 搜索、读取、新建、更新、追加、移动、整理笔记；删除进回收站，可恢复 |
| **安全边界** | 仅监听 `127.0.0.1`，Bearer Token；默认只读，写入需在「MCP → MCP 服务设置…」中手动开启；锁定文稿/目录不可访问，草稿不暴露 |
| **怎么开始** | Woo → MCP → MCP 服务设置… → 启用，把地址和令牌复制到 AI 工具；设置页提供 Claude Code / Codex / Hermes / Pi 的现成配置 |

完整客户端配置、工具清单和故障排查见 [docs/mcp.md](docs/mcp.md)。

> 换句话说：不是 Woo 内置一个 AI，而是你已经在用的 AI 可以直接调用 Woo 来维护笔记。

---

## 📦 下载

| 平台 | 下载 | 说明 |
|------|------|------|
| 🍎 macOS | [DMG (Apple Silicon)](https://github.com/stophemo/Woo/releases/latest) | 适用于 M 系列 Mac；签名与公证状态以对应 Release 说明为准 |
| 🤖 Android | [已签名 APK (ARM64)](https://github.com/stophemo/Woo/releases/latest) | 支持 `arm64-v8a`，安装时允许来自此来源的应用 |

> **macOS 用户**：安装前请阅读对应 Release 说明。若版本未完成 Apple 公证，首次启动请在 Finder 中右键 Woo.app 并选择“打开”，或前往“系统设置 → 隐私与安全”选择“仍要打开”。无需、也不要安装任何第三方根证书。

> **Windows 用户**：Windows 版暂停维护，暂不提供新的安装包与自动更新；已安装的旧版本仍可继续使用，但不再跟进新功能与修复。

---

## 🩺 排查问题

应用会把运行日志写入数据目录下的 `logs/woo.log`（超过 2MB 自动轮转为 `woo.log.1`）。同步、数据库打开等关键步骤均有记录，反馈问题时可只提供这个文件。

| 平台 | 日志路径 |
|------|----------|
| macOS | `~/Library/Application Support/Woo/logs/woo.log`（开发环境为 `woo-dev`） |
| Windows（旧版本） | `%APPDATA%\Woo\logs\woo.log`（开发环境为 `woo-dev`） |
| Android | 应用私有目录下的 `logs/woo.log` |

日志不会记录密码、令牌或 vault 密钥等敏感内容；同步失败时只会记录错误码与描述。

---

## 🚀 快速开始（用户）

1. 前往 [Releases](https://github.com/stophemo/Woo/releases/latest) 下载对应平台安装包
2. 安装并启动
3. 开始写作，文稿保存在本机 `woo.db`，无需登录
4. （可选）桌面端「设置 → 跨设备」或移动端「我的 → 跨设备」配置同一个 WebDAV 账号；保存后自动建立或加入同步空间，直接开始同步

---

## 🛠️ 开发

### 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | [Tauri v2](https://v2.tauri.app/) (Rust) |
| 前端 | Vue 3 + Pinia + Vite + TypeScript |
| 编辑器 | Tiptap (ProseMirror) |
| 数据库 | SQLite (rusqlite, bundled) |
| 同步 | 任意 HTTPS WebDAV + 客户端加密操作日志（填账号即同步，兼容六位码）；旧 Worker / 局域网已停用 |
| 移动端 | Tauri Mobile (Android；iOS 计划中) |

### 项目结构

```
Woo/
├── src/                    # 桌面端前端 (Vue 3, ESM)
│   ├── stores/             # Pinia 状态管理 (workspace, sync, lock, theme…)
│   ├── services/           # IPC 客户端、更新、Markdown 转换与导出
│   ├── components/         # 桌面端 UI 组件 (layout/, settings/)
│   └── types/              # TypeScript 类型定义
├── src-mobile/             # 移动端前端 (Vue 3 + Vant UI)
│   ├── views/              # 移动端页面
│   └── router/             # 移动端路由
├── src-tauri/              # Rust 后端 (桌面端 + 移动端共用)
│   ├── src/
│   │   ├── commands/       # Tauri IPC 命令入口
│   │   ├── services/       # 业务逻辑 (folder, document, sync_engine, vault_sync…)
│   │   ├── db/             # SQLite 连接管理 + Schema 迁移
│   │   ├── supabase/       # 旧 Supabase 模块（运行时未启用）
│   │   └── models/         # 数据模型
│   ├── woo-sync-core/      # 共享加密、协议与内容分块内核
│   └── gen/android/        # Android 项目骨架
├── sync-server/            # 旧 Worker 同步后端（已停用，仅保留迁移参考）
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
              自动拆包 { ok, data }                    WebDAV (密文)
```

- 所有 Rust 命令返回 `CommandResult<T> { ok, data?, message? }`
- 前端 `api.ts` 自动解包：`ok=false` 抛异常，原始值直接透传
- IPC 格式：`namespace:action`（如 `document:listByFolder`），参数为对象

> ⚠️ 禁止直接 `import { invoke }` —— 一律走 `services/api.ts`

### 数据库

应用始终使用单用户本地库 `woo.db`；启动不再恢复云端会话，也不会切换到 `woo-{user}.db`。旧版遗留的用户库文件仍保留在数据目录，但不会被自动打开。无文件夹草稿使用 `localStorage`，不进入 SQLite，也不参与同步和版本历史。

| 表 | 说明 |
|---|---|
| `note_folder` | 文件夹树，`parent_id` 层级结构，软删除三态 |
| `note_document` | 文档，HTML `content`，标题从第一行自动提取 |
| `note_document_version` | 版本历史 (auto / manual / restore)，仅本机 |
| `sync_meta` | 键值存储与同步开关 |
| `vault_sync_state` | 同步身份、Lamport、本地应用序号与上次成功时间 |
| `vault_sync_outbox` | 待发送/确认的加密操作 |
| `vault_sync_pending_*` | 脏文稿/目录与待删除记录（触发器维护） |
| `vault_sync_chunk_refs` / `vault_sync_incoming_chunks` | 正文清单与内容块缓存 |
| `vault_sync_entity_versions` / `vault_sync_applied_ops` | LWW 版本与游标幂等台账 |

**软删除三态**：`deleted = 0` 正常 → `1` 回收站 → `2` 待硬删除（7 天清理窗口）

### 同步引擎

每台设备在「跨设备」填写同一个 WebDAV 账号后即自动同步：第一台设备自动建立空间，其他设备自动加入，不需要配对码。需要云端不可恢复密钥时，可改用六位码配对。同步在启动、本地修改、回到前台和每 15 分钟兜底时进行；WebDAV 没有服务端推送，不是实时同步。

- **端到端加密**：账号与同步身份分别加密保存在本机；正文按内容定义边界分块、压缩后用 AES-256-GCM 加密，WebDAV 只保存密文。
- **增量操作日志**：本地 outbox 推送为 `v1/<vaultId>/ops/<shard>/<opId>.json`；拉取时两层 PROPFIND 列举，按 opId 幂等去重。
- **六位配对码**：兼容旧空间和严格模式；只有持有空间的一方点一次「批准加入」，加入方自动保存身份。
- **冲突副本**：按 `(lamport, deviceId)` LWW；本地有未同步修改时先创建冲突副本，编辑器正在输入时也会先保存远端版本。
- **删除**：回收站状态随正文同步；清空回收站后 7 天内物理删除并生成 tombstone，向所有设备传播。
- **只读边界**：版本历史、密码锁密码哈希、知识库分块、草稿和界面偏好不参与同步。
- **只读设备列表**：每次同步写加密心跳，列表展示设备名、平台、最近同步和版本；没有移除按钮，要阻止旧设备请修改 WebDAV 应用密码。
- 旧 Worker / 局域网凭据保留但不启用，旧 WebDAV 账号会自动迁移。
  详见 [docs/domestic-sync.md](docs/domestic-sync.md)。

---

## 🗺️ 路线图

- [x] WebDAV 同步空间（端到端加密、六位配对码）
- [x] 多设备六位配对码
- [x] 正文/图片分块加密同步
- [x] 移动端阅读目录
- [x] 旧 WebDAV / Worker / 局域网迁移与停用
- [ ] 同步空间快照压缩与恢复码
- [ ] macOS 原生菜单 + 完整快捷键
- [ ] Homebrew Cask 分发
- [ ] iOS 支持 (Tauri Mobile)
- [x] 跨平台基础框架 (macOS + Android)
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
