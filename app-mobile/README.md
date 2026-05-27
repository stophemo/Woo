# 无我笔记（移动端）· app-mobile

> **Woo · Mobile Edition** — 基于 Flutter 的 Android 本地优先笔记应用，与桌面端完全兼容。

---

## 架构总览

```
lib/
├── main.dart                    # 入口
├── app.dart                     # MaterialApp + 认证路由 + 主壳（底部导航）
├── config/
│   ├── constants.dart           # 全局常量
│   └── supabase_config.dart     # Supabase 配置（与桌面端共用同一项目）
├── database/
│   └── app_database.dart        # SQLite 封装（sqflite）— 表结构与桌面端完全一致
├── models/
│   └── models.dart              # Folder, Document, DocumentVersion, FolderNode
├── services/
│   ├── folder_service.dart      # 目录 CRUD（镜像 folderService.cjs）
│   ├── document_service.dart    # 文稿 CRUD（镜像 documentService.cjs）
│   ├── supabase_service.dart    # Supabase 认证封装
│   └── sync_engine.dart         # 同步引擎（与 syncEngine.cjs 兼容的协议）
├── providers/
│   └── providers.dart           # Riverpod 状态管理
├── screens/
│   ├── auth_screen.dart         # 登录/注册
│   ├── home_screen.dart         # 主页（文件夹抽屉 + 文档列表 + 搜索）
│   ├── editor_screen.dart       # 编辑器（Markdown 编辑 / HTML 存储）
│   ├── trash_screen.dart        # 回收站（恢复/永久删除/清空）
│   └── settings_screen.dart     # 设置（账户/同步/存储/关于）
└── widgets/
    ├── folder_tree_tile.dart    # 可展开的目录树节点
    └── sync_status_banner.dart  # 同步状态横幅
```

## 零成本策略

| 成本项 | 策略 |
|--------|------|
| **Apple Developer ($99/年)** | 跳过 iOS，仅 Android |
| **Google Play ($25 一次性)** | 跳过，通过 GitHub Releases 分发 APK |
| **服务器** | 无服务器 — Supabase 免费层（500MB DB，与桌面端共用） |
| **推送通知** | 不实现（避免 FCM 复杂度） |
| **实时同步** | 轮询（60 秒间隔），不需要 WebSocket 实时订阅 |

## 与桌面端兼容性

- **SQLite 表结构**：`note_folder` / `note_document` / `note_document_version` / `sync_meta` — 完全一致
- **同步协议**：Supabase last-write-wins，三层表 upsert，墓碑传播 — 完全一致
- **内容格式**：HTML 存储 — 兼容（移动端编辑 Markdown → 存为简单 HTML）
- **认证**：共用 Supabase 用户系统（邮箱密码）

## 核心数据流

```
用户操作 → Provider (Riverpod) → Service → AppDatabase (sqflite) → SQLite
                                                 ↓
同步引擎 → Supabase API → 云端（与桌面端双向同步）
```

## 开发

```bash
# 前提：安装 Flutter SDK（https://docs.flutter.dev/get-started/install）
# 然后在 app-mobile 目录下执行：

flutter pub get           # 安装依赖
flutter run               # 运行调试（连接 Android 设备/模拟器）
flutter build apk         # 构建 Android APK

# 初始化 Flutter 工程（如果缺少 android/ 目录）：
flutter create .
# 注意：flutter create 不会覆盖已有的 lib/ 和 pubspec.yaml
```

## 状态

- **MVP 功能**：目录树管理、Markdown 笔记编辑、本地存储、回收站
- **同步**：Supabase 双向同步（与桌面端互通）
- **待完善**：AI 集成、富文本编辑器、主题自定义、生物识别锁

---

> 该项目零运营成本运行。Supabase URL 和 anon key 已公开嵌入（设计如此），数据安全由 Supabase RLS 策略保障。
