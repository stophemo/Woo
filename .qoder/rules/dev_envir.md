---
trigger: always_on
---

# Electron 跨平台开发规则

## 环境声明
- **开发机**: macOS（本地开发、调试、构建）
- **目标平台**: Windows 10/11 + macOS 10.15+（Intel & Apple Silicon）
- **禁止假设**: 不得假设用户文件系统大小写敏感、路径分隔符或 shell 可用性

## 核心编码规则

### 1. 路径与文件系统（强制）
- 统一使用 Node.js `path` 模块：`path.join()`, `path.resolve()`, `path.normalize()`
- 禁止硬编码路径分隔符（`/` 或 `\`）
- 禁止假设文件系统大小写敏感；所有文件名/路径比较必须统一转小写后再判断
- 禁止假设文件系统支持 `:`, `?`, `*` 等特殊字符；用户输入文件名时必须做跨平台 sanitize

### 2. 窗口与 UI 行为（强制）
- 菜单栏：macOS 使用全局 `Menu.setApplicationMenu()`；Windows/Linux 使用窗口级菜单，禁止混用
- 托盘图标：必须分别提供 `.icns` (macOS) 和 `.ico` (Windows) 格式，禁止单格式硬编码
- 窗口控制按钮：macOS 左置，Windows 右置；CSS/布局不得写死标题栏区域高度，使用 `-webkit-app-region: drag` 时预留平台差异空间

### 3. 调试与测试（强制）
- 本地调试以 macOS 为主，但所有文件/路径相关代码必须在代码审查阶段做 Windows 场景脑检
- 日志路径：使用 `app.getPath('logs')` 或 `app.getPath('userData')`，禁止写死 `~/Library/Logs` 或 `C:\Users\xxx`

