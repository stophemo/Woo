# Woo（无我笔记）

Woo（无我笔记）是一款专注写作的 Markdown 笔记软件，**纯本地、零后端**，数据完全存储在你的电脑上。

## 快速开始

无需安装数据库、无需启动后端服务，下载即用：

```powershell
cd app-desktop
npm install
npm run dev            # 开发模式
npm run electron:build # 打包桌面应用
```

打包产物位于 `app-desktop/release/`，包含：
- `woo-local-{version}-win-x64-setup.exe` — NSIS 安装包
- `woo-local-{version}-win-x64-portable.exe` — 单文件绿色版
- `woo-local-{version}-win-x64.zip` — 压缩包绿色版

数据存储位置：`%APPDATA%/无我笔记/woo.db`

> 详细文档见：[app-desktop/README.md](app-desktop/README.md)

## 项目结构

```
Woo/
├── app-desktop/            # 桌面端（Electron + Vue 3 + SQLite）
│   ├── electron/           # Electron 主进程
│   ├── src/                # Vue 3 前端源码
│   ├── build/              # 打包资源
│   └── release/            # 打包产物（gitignore）
└── app-mobile/             # 移动端（Flutter）
```

## 技术栈

- **框架**：Vue 3 + TypeScript
- **桌面壳**：Electron
- **数据库**：SQLite（better-sqlite3）
- **构建**：Vite + electron-builder

## 贡献

欢迎提交 Issue 和 Pull Request 来改进项目。

## 许可证

本项目采用 [LICENSE](LICENSE) 许可证。
