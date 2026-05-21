# 无我笔记（移动端） · app-mobile

> **Woo · Mobile Edition** — 基于 Flutter 的跨平台移动端笔记应用，覆盖 Android / iOS。

---

## 技术选型

### 框架：Flutter

| 维度 | 选择理由 |
|------|---------|
| **性能** | 自绘 Skia 引擎，60fps 原生级流畅度，不依赖 WebView |
| **跨平台** | 一套 Dart 代码同时输出 Android + iOS |
| **生态** | 笔记类所需插件成熟（Markdown、富文本、SQLite、文件操作） |
| **成熟度** | Google 主推，v3+ 稳定版，国内大厂广泛使用 |

### 语言：Dart

桌面端（`app-desktop`）使用 TypeScript，移动端改用 Dart，UI 层面需重新实现，但数据模型、业务逻辑可以翻译移植。

---

## 所需核心插件

| 功能 | Flutter 插件 |
|------|-------------|
| Markdown 渲染 | `flutter_markdown` |
| 富文本编辑器 | `flutter_quill` |
| SQLite 数据库 | `drift`（推荐）或 `sqflite` |
| 文件选择 | `file_picker` |
| 文件路径 | `path_provider` |
| 系统分享 | `share_plus` |
| 生物识别锁 | `local_auth` |
| 手写/涂鸦（规划） | 自定义 CustomPainter |

---

## 与桌面端的关系

```
Vue 3 前端（app-desktop: 仅桌面端使用）
└── 不共享 UI 代码

SQLite 表结构（共享设计）
├── app-desktop: better-sqlite3（Node.js 原生模块）
└── app-mobile: drift / sqflite（Dart 实现）

业务逻辑（翻译移植）
├── 目录/文档 CRUD
├── 版本快照管理
├── 账户认证（本地 bcrypt）
└── AI 辅助写作
```

### 不共享的部分
- **UI 组件**：Vue 组件无法在 Flutter 中使用，需全部重写
- **数据层实现**：Electron IPC → Flutter 插件调用
- **原生功能**：文件系统、分享、生物识别等平台 API

### 可复用的设计
- SQLite 表结构（可对照 `app-desktop/electron/db/schema.cjs` 建表）
- 业务逻辑流程（注册/登录、目录树构建、版本合并策略）
- 数据模型的字段设计

---

## 初始化流程

```bash
# 安装 Flutter SDK（参考 https://docs.flutter.dev/get-started/install）
flutter doctor            # 检查环境
flutter create .          # 在当前目录初始化 Flutter 项目
flutter run               # 运行调试
flutter build apk         # 构建 Android
flutter build ios         # 构建 iOS
```

> 初始化后 `.gitignore` 中的 Flutter 忽略规则会自动生效，构建产物不会被提交。

---

## 版本规划建议

| 阶段 | 目标 |
|------|------|
| **MVP** | 基础 Markdown 编辑器 + SQLite 本地存储 + 目录树 |
| **V1.0** | 富文本编辑（flutter_quill）+ 版本快照 + 账户系统 |
| **V1.1** | 主题适配 + 文件导入导出 + iCloud/Google Drive 同步 |
| **V1.2** | AI 辅助写作 + 手写涂鸦 |
