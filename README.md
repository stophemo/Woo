<p align="center">
  <img src="app-desktop/build/icon.svg" width="120" height="120" alt="Woo Logo">
</p>

<h1 align="center">Woo · 无我笔记</h1>

<p align="center">
  <strong> - 沉浸写作，心无旁骛 -</strong>
</p>

<p align="center">
  <a href="https://github.com/stophemo/Woo/releases/latest">
    <img src="https://img.shields.io/badge/版本-0.4.7-4f46e5?style=flat" alt="最新版本">
  </a>
  <a href="https://github.com/stophemo/Woo/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/license-MIT-blue?style=flat" alt="MIT 许可证">
  </a>
  <a href="https://woo-notes.vercel.app">
    <img src="https://img.shields.io/badge/website-woo--notes.vercel.app-8b5cf6?style=flat" alt="项目主页">
  </a>
</p>

<br>

**Woo**（无我笔记）是一款 Markdown 笔记桌面应用，基于 Electron + Vue 3 + SQLite 构建。数据默认存储在本地，登录后可选择开启 Supabase 云端同步，实现多设备数据互联。

> 「无我」源自佛教哲学，意为超越自我执念。Woo 以此命名，希望提供一个简洁、干净的写作环境，将写作回归文字本身。

---

## ✨ 功能特性

### 📦 本地架构
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

## 📄 许可证

[MIT](LICENSE) © 2026 Stophemo

---

## 🌐 相关链接

- [项目主页](https://woo-notes.vercel.app)
