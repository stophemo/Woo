<p align="center">
  <img src="app-desktop/build/icon.svg" width="120" height="120" alt="Woo Logo">
</p>

<h1 align="center">Woo · 无我笔记</h1>

<p align="center">
  <strong>专注写作的 Markdown 笔记 — 本地优先，掌控你的文字</strong>
</p>

<p align="center">
  <a href="https://github.com/stophemo/Woo/releases/latest">最新版本</a>
  &nbsp;·&nbsp;
  <a href="https://github.com/stophemo/Woo/blob/main/LICENSE">MIT 许可证</a>
  &nbsp;·&nbsp;
  <a href="https://woo-notes.vercel.app">项目主页</a>
</p>

<br>

**Woo**（无我笔记）是一款本地优先的 Markdown 笔记桌面应用，基于 Electron + Vue 3 + SQLite 构建。数据默认存储在本地，登录后可选择开启 Supabase 云端同步，实现多设备数据互联。

> 「无我」源自佛教哲学，意为超越自我执念。Woo 以此命名，希望提供一个简约、专注的写作环境，让你放下对排版、格式、平台的焦虑，回归文字本身。

---

## ✨ 功能特性

### 📦 本地优先架构
所有数据存储在本地 SQLite 数据库，你拥有笔记的**完全所有权**。无需注册账号、无需启动后端服务、无需联网，下载即可使用。数据文件直接复制即可备份或迁移。

### ✍️ 所见即所得编辑
基于 **Tiptap / ProseMirror** 的富文本编辑器，实时 Markdown 渲染。支持标题、列表、代码块、引用等常用格式，书写与预览合一，排版随心。

### 🖥️ 跨平台支持
基于 Electron 构建，适用于 **Windows**（7+）和 **macOS**（Apple Silicon），一套数据随处访问。Linux 支持开发中。

### ☁️ 可选云同步
登录后即可开启 **Supabase** 云端同步，多设备数据互联。最后写入者胜出的合并策略，确保数据一致性。云同步纯属可选，数据始终由你掌控。

### ⏱️ 版本历史记录
每次保存自动建立快照，采用 24 小时 / 7 天分层合并策略，智能差异跟踪。支持回滚到任意历史版本，再也不怕误操作。

### 🌙 暗色主题
精心设计的深色紫主题，降低长时间写作的眼部疲劳。浅色/深色两种主题自由切换，日夜写作同样舒适。

### 🤖 AI 智能助手
基于 **DeepSeek** / **Google Gemini** / **OpenAI 兼容** API 接入（需自备 API 密钥）。通过工具调用，AI 可以：
- 🔍 **语义搜索笔记** — 基于向量嵌入（BGE 本地模型）的 RAG 检索，结合笔记内容回答问题
- ✏️ **自然语言操控笔记** — 通过对话创建、修改、删除笔记和目录，支持流式写入编辑器
- 📋 **实时思考过程** — AI 的搜索、工具调用过程实时展示在聊天界面

### 📤 多格式导出
支持将文稿导出为 **Markdown**、**PDF**、**WebP 图片**、**思维导图**（PNG / SVG）等多种格式。

---

## 📥 下载

<table>
  <tr>
    <td align="center"><b>Windows</b></td>
    <td align="center"><b>macOS</b></td>
  </tr>
  <tr>
    <td>
      <a href="https://github.com/stophemo/Woo/releases/latest">安装版 (.exe)</a><br>
      <a href="https://github.com/stophemo/Woo/releases/latest">便携版 (.exe)</a><br>
      <a href="https://github.com/stophemo/Woo/releases/latest">压缩包 (.zip)</a>
    </td>
    <td>
      <a href="https://github.com/stophemo/Woo/releases/latest">DMG (Apple Silicon)</a><br>
      <a href="https://github.com/stophemo/Woo/releases/latest">压缩包 (.zip)</a>
    </td>
  </tr>
</table>

> 📎 所有安装包均可在 [GitHub Releases](https://github.com/stophemo/Woo/releases) 页面获取。移动端（Flutter）开发中。

---

## 🏗️ 技术栈

| 层次 | 技术 |
|---|---|
| 桌面壳 | Electron 29（Chromium + Node 20 ABI） |
| UI 框架 | Vue 3 (Composition API) + TypeScript |
| 状态管理 | Pinia |
| 编辑器 | Tiptap v3（ProseMirror） |
| 构建工具 | Vite 5 + electron-builder |
| 本地存储 | SQLite（better-sqlite3）|
| 云端同步 | Supabase（可选） |
| AI 集成 | Google Gemini / DeepSeek（用户自备密钥）|
| 密码加密 | bcryptjs |

---

## 📁 项目结构

```
Woo/
├── app-desktop/                # Electron + Vue 3 桌面应用
│   ├── electron/               # 主进程（CommonJS）
│   │   ├── main.cjs            # 入口：窗口、菜单、IPC、数据库
│   │   ├── preload.cjs         # contextBridge API
│   │   ├── ipc/                # IPC 处理注册
│   │   ├── db/                 # SQLite 连接与模式迁移
│   │   ├── services/           # 业务逻辑层
│   │   └── config/             # Supabase 配置
│   ├── src/                    # 渲染进程（Vue 3）
│   │   ├── stores/             # Pinia 状态管理
│   │   ├── services/           # IPC 客户端 + AI 客户端
│   │   ├── components/         # UI 组件
│   │   ├── config/             # 菜单定义
│   │   └── types/              # TypeScript 类型
│   ├── build/                  # 图标资源
│   └── package.json
└── app-mobile/                 # 移动端（Flutter，开发中）
```

---

## 🚀 本地开发

```bash
# 进入桌面端目录
cd app-desktop

# 安装依赖
npm install

# 开发模式（两个终端）
# 终端 1: Vite 热更新
npm run dev

# 终端 2: Electron（加载 Vite 开发服务器）
npm run electron:dev

# 构建发布版
npm run electron:build
```

### 开发环境要求

| 工具 | 版本 | 用途 |
|---|---|---|
| Node.js | ≥ 18.19 | 运行 Vite / Electron |
| npm | ≥ 10 | 包管理 |
| Python | 3.10 / 3.11（64-bit）| node-gyp 编译 SQLite 原生模块 |

> 构建产物位于 `app-desktop/release/`。Windows 构建需要 Visual Studio Build Tools（含 C++ 桌面开发工作负载）。

### Electron 版本升级后

```bash
npm run rebuild:sqlite    # 重新编译 better-sqlite3
```

---

## 📦 发布流程

推送匹配的标签会自动触发 GitHub Actions 构建并发布：

```bash
git tag v0.4.7
git push origin v0.4.7
```

支持三种标签格式：`v*`、`app-desktop-v*`、`Woo-desktop-v*`。CI 会同时在 Windows 和 macOS 上构建，并将产物上传到 GitHub Releases。

---

## 📄 许可证

[MIT](LICENSE) © 2026 Eliza Johnson

---

## 🌐 相关链接

- [项目主页](https://woo-notes.vercel.app)
- [GitHub Releases](https://github.com/stophemo/Woo/releases)
- [邮箱验证页面](https://woo-notes.vercel.app/auth-success)
