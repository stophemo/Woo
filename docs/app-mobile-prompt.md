# app-mobile 实现提示词

> 将此文档交给任何 AI 助手，即可从零实现 Woo 移动端 Flutter 应用。
> 目标：iOS + Android 本地优先笔记 App，与桌面端 (Electron + Vue 3) 完全兼容，零运营成本。

---

## 一、项目背景

Woo（无我笔记）是一款本地优先的 Markdown 笔记应用。现有桌面端 (`app-desktop/`) 基于 Electron + Vue 3 + SQLite (better-sqlite3) 构建，支持可选的 Supabase 云同步。现在需要开发移动端 (`app-mobile/`)。

### 核心约束

- **零成本**：不支付任何服务费。Supabase 免费层（500MB DB，个人笔记足够），无服务器
- **双平台**：iOS + Android（Dart 代码跨平台，仅需各自构建）
- **本地优先**：SQLite 是主数据源，Supabase 仅用于多设备同步
- **桌面兼容**：表结构、同步协议、内容格式必须与桌面端互通

### 关键架构文件（参考）

桌面端关键实现位于：
- `app-desktop/electron/db/schema.cjs` — SQLite 建表脚本（note_folder, note_document, note_document_version, sync_meta）
- `app-desktop/electron/services/syncEngine.cjs` — 同步引擎（push/pull/tombstone/cleanup）
- `app-desktop/electron/services/folderService.cjs` — 目录业务逻辑
- `app-desktop/electron/services/documentService.cjs` — 文稿业务逻辑
- `app-desktop/electron/config/supabase.cjs` — Supabase 配置（URL + anon key）

---

## 二、技术选型

| 维度 | 选择 | 理由 |
|------|------|------|
| 框架 | Flutter | 一套代码输出 Android + iOS |
| 语言 | Dart 3.x | Flutter 官方语言 |
| 数据库 | sqflite | 轻量、无代码生成、与桌面端 better-sqlite3 共用表结构 |
| 状态管理 | Riverpod | 类型安全、无 BuildContext 依赖、适合中型应用 |
| 云同步 | supabase_flutter | 与桌面端共用 Supabase 项目（免费层） |
| 内容编辑 | 纯文本 + Markdown 预览 | MVP 阶段避免引入 flutter_quill 的复杂性 |
| 目标平台 | Android + iOS | Flutter 天然支持，无需额外成本 |

---

## 三、SQLite 表结构（必须与桌面端完全一致）

```sql
-- note_folder: 目录树（层级结构通过 parent_id）
CREATE TABLE note_folder (
    id TEXT PRIMARY KEY,
    parent_id TEXT,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    create_time TEXT NOT NULL,
    update_time TEXT NOT NULL,
    deleted INTEGER NOT NULL DEFAULT 0,      -- 0=正常 1=回收站 2=待清理
    is_locked INTEGER NOT NULL DEFAULT 0
);

-- note_document: 文稿
CREATE TABLE note_document (
    id TEXT PRIMARY KEY,
    folder_id TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT,
    branch_name TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    create_time TEXT NOT NULL,
    update_time TEXT NOT NULL,
    deleted INTEGER NOT NULL DEFAULT 0,
    is_locked INTEGER NOT NULL DEFAULT 0
);

-- note_document_version: 版本历史
CREATE TABLE note_document_version (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL,
    version_no INTEGER NOT NULL,
    title TEXT,
    content TEXT,
    content_hash TEXT,
    change_type TEXT NOT NULL DEFAULT 'auto',  -- auto/manual/restore
    operator_id TEXT,
    create_time TEXT NOT NULL,
    update_time TEXT NOT NULL,
    deleted INTEGER NOT NULL DEFAULT 0,
    UNIQUE(document_id, version_no)
);

-- sync_meta: 同步状态（键值对）
CREATE TABLE sync_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

### 关于 user_id 列的重要说明

Supabase 云端表每行有 `user_id` 列用于 RLS 行级安全。**本地 SQLite 不含此列**（每个库文件已对应单一用户）。同步引擎在拉取时需剥离 `user_id`，推送时需添加。

### 文档内容格式

- 数据库存储格式：**HTML**（与桌面端兼容）
- 移动端编辑：用户输入 **Markdown/纯文本**
- 编辑器保存时：Markdown → 简单 HTML 转换（至少 `<p>`包裹，`<br>`换行）
- 显示预览时：HTML → 纯文本提取 → Markdown 小部件渲染

---

## 四、同步引擎设计（必须与桌面端协议兼容）

同步协议与 `syncEngine.cjs` 一致，每次同步按顺序执行 4 步：

### 步骤 1: PULL 墓碑
```
读取云端 sync_tombstone 表（filter: user_id + deleted_at > last_tombstone_pull）
  → 本地硬删除匹配记录（note_document 需级联删除 note_document_version）
  → 更新 last_tombstone_pull
  → 回收云端 30 天前的旧墓碑
```

### 步骤 2: PULL 远程变更
```
对 note_folder / note_document / note_document_version 三张表：
  → 查询云端 update_time > last_sync_time 且 user_id = 当前用户
  → 逐行合并到本地：
    - 本地不存在该行 → 直接插入
    - 本地 update_time >= 远程 update_time → 跳过（本地胜出）
    - 本地 update_time < 远程 update_time → 覆盖
  → 记录本轮拉取的 ID（用于步骤 3 的防回显）
```

### 步骤 3: PUSH 本地变更
```
对三张表：
  → 查询本地 update_time > last_sync_time 的记录
  → 过滤掉步骤 2 拉取的 ID（防回显）
  → 为每行加上 user_id → upsert 到云端（onConflict: 'id'）
```

### 步骤 4: 超期清理
```
扫描本地 deleted=2 且 update_time 超过 7 天的记录：
  → 云端硬删除对应行
  → 云端写入 sync_tombstone（通知其他设备）
  → 本地硬删除
→ 更新 last_sync_time
```

### 冲突策略
**最后写入胜出 (Last-Write-Wins)** — 与桌面端完全一致。仅在拉取时做时间戳比较。

---

## 五、已创建的目录结构

```
app-mobile/
├── pubspec.yaml
├── analysis_options.yaml
├── lib/
│   ├── main.dart                        # 入口
│   ├── app.dart                         # MaterialApp + 认证路由 + 主壳
│   ├── config/
│   │   ├── constants.dart               # 全局常量
│   │   └── supabase_config.dart         # Supabase URL + anon key
│   ├── database/
│   │   └── app_database.dart            # SQLite 封装 + DAO
│   ├── models/
│   │   └── models.dart                  # Folder, Document, DocumentVersion
│   ├── services/
│   │   ├── folder_service.dart          # 目录 CRUD
│   │   ├── document_service.dart        # 文稿 CRUD
│   │   ├── supabase_service.dart        # Supabase 认证
│   │   └── sync_engine.dart             # 同步引擎
│   ├── providers/
│   │   └── providers.dart               # Riverpod 状态管理
│   ├── screens/
│   │   ├── auth_screen.dart             # 登录/注册/跳过
│   │   ├── home_screen.dart             # 主页 + 目录树 + 文档列表 + 搜索
│   │   ├── editor_screen.dart           # 编辑器
│   │   ├── trash_screen.dart            # 回收站
│   │   └── settings_screen.dart         # 设置（含登录弹窗）
│   └── widgets/
│       ├── folder_tree_tile.dart        # 目录树节点
│       └── sync_status_banner.dart      # 同步状态条
```

所有 .dart 文件已写好初始代码，`flutter create .` 后可直接编译运行。

---

## 六、数据流

```
用户操作
    ↓
Screen 组件 (ConsumerWidget)
    ↓
Riverpod Provider (FutureProvider / StateNotifierProvider)
    ↓
Service (FolderService / DocumentService)
    ↓
AppDatabase (sqflite DAO)
    ↓
SQLite (本地持久化)
    ↑
SyncEngine (60s 轮询, 仅登录后启动)
    ↓
Supabase API (云端同步)
```

- **UI 层**：所有页面用 `ConsumerWidget` 或 `ConsumerStatefulWidget`
- **状态层**：Riverpod 管理
- **服务层**：纯 Dart 类，无 Flutter 依赖
- **数据层**：`sqflite` 直接操作，DAO 模式封装

---

## 七、零成本策略

| 成本项 | 策略 |
|--------|------|
| Apple Developer ($99/年) | 个人开发者可跳过（侧载），如需上架 App Store 则需要 |
| Google Play ($25 一次性) | 可跳过，通过 GitHub Releases 分发 APK |
| 服务器 | 无 — Supabase 免费层（与桌面端共用，500MB DB 足够） |
| 推送通知 | 不实现（避免 FCM/APNs 复杂度） |
| 实时同步 | 60s 轮询，无 WebSocket 订阅 |

---

## 八、UI 设计指南

### 8.1 设计原则

- Material Design 3，`ColorScheme.fromSeed()` 生成主题色
- NavigationBar（底部 3 tab）+ Drawer（目录树）
- 简洁、内容优先

### 8.2 页面结构

```
底部导航栏
├── 笔记页 (HomeScreen)
│   ├── AppBar: 标题 + 目录按钮(左) + 搜索(右) + 新建(右)
│   ├── 抽屉: 目录树（可展开、选中高亮、新建目录）
│   ├── 内容: 全部笔记 / 按目录过滤
│   └── 空状态: "暂无笔记"
│
├── 回收站 (TrashScreen)
│   ├── AppBar: 标题 + 清空按钮
│   ├── 列表: 标题 + 删除时间 + 恢复/永久删除
│   └── 空状态
│
└── 设置页 (SettingsScreen)
    ├── 账户: 登录/退出（登录弹窗）
    ├── 同步: 状态 + 手动同步
    ├── 存储: 数据库信息
    └── 关于: 版本号
```

### 8.3 编辑器

```
EditorScreen
├── AppBar: 文档标题(自动提取) + 预览按钮
├── 提示条: "输入纯文本/Markdown，自动保存"
└── 编辑区 (多行 TextField, 1.5s 防抖自动保存)
    └── 预览: BottomSheet + flutter_markdown 渲染
```

---

## 九、依赖清单

```yaml
dependencies:
  flutter: sdk: flutter
  sqflite: ^2.3.0
  path_provider: ^2.1.1
  path: ^1.8.3
  supabase_flutter: ^2.5.0
  flutter_riverpod: ^2.4.9
  flutter_markdown: ^0.6.22
  intl: ^0.19.0
  share_plus: ^7.2.2
  uuid: ^4.3.3
  html: ^0.15.4
```

---

## 十、实现顺序

| 阶段 | 任务 |
|------|------|
| 1 | `flutter create .` + `flutter pub get` 初始化平台目录 |
| 2 | 数据模型 + 数据库层 |
| 3 | 服务层（FolderService + DocumentService） |
| 4 | Supabase 认证 |
| 5 | 状态管理（Riverpod providers） |
| 6 | UI 页面（HomeScreen + EditorScreen） |
| 7 | 回收站 + 设置页面 |
| 8 | 同步引擎 |
| 9 | UI 打磨 + 错误处理 |
| 10 | 构建测试（`flutter build apk` / `flutter build ios`） |

---

## 十一、双平台注意事项

### iOS
- `flutter build ios` 需要 macOS + Xcode
- 真机部署需要 Apple Developer 账号（$99/年）
- 模拟器测试免费，`flutter run` 即可
- iOS 14+ 最低部署目标（supabase_flutter 要求）
- `ios/Podfile` 中需设置 `platform :ios, '14.0'`

### Android
- 构建命令：`flutter build apk --split-per-abi`
- 开发者模式 + USB 调试即可真机运行
- minSdkVersion 需 ≥ 21（supabase_flutter 要求）

---

## 十二、与桌面端的差异点

### 直接复用
- SQLite 表结构（字段名、类型、索引完全一致）
- 同步协议（pull/push/tombstone/cleanup）
- 软删除三态（0/1/2）
- Supabase 项目（URL + anon key）

### 需要重写
- **UI**：Vue 组件 → Flutter Widget
- **数据层**：better-sqlite3 (Node) → sqflite (Dart)
- **编辑器**：Tiptap (ProseMirror) → TextField + Markdown
- **IPC**：preload 桥接 → 直接函数调用

### 移动端特有
- 底部导航栏（3 tab）代替桌面端侧边栏
- 抽屉式目录树代替桌面端常驻侧栏
- SearchDelegate 搜索
- RefreshIndicator 下拉刷新

---

## 十三、潜在陷阱

1. **HTML 内容兼容**：桌面端 HTML 结构可能复杂，移动端预览用 `flutter_markdown` 渲染纯文本提取物
2. **同步冲突**：同一文档在桌面和移动端同时编辑 → 最后一次同步者胜出（已处理）
3. **首次启动**：需 `await Supabase.initialize()` 再运行 App
4. **Supabase 不可用**：sync engine 有 try-catch，纯本地模式不受影响
5. **iOS 构建**：无 macOS 时可用 `flutter build apk` 只构建 Android；iOS 需 macOS 环境
6. **sqflite iOS**：iOS 模拟器/真机均正常，无需额外配置
