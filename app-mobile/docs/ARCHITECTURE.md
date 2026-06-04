# Woo 移动端架构设计

> 状态：设计稿 · 版本 0.1  
> 目标平台：Android · iOS · HarmonyOS（via flutter-ohos）

---

## 设计原则

1. **功能对等**：与桌面端功能一致（创建/编辑/删除/搜索/同步/版本历史），不追求 UI 一致
2. **本地优先**：同桌面端，SQLite 为主数据源，云同步为可选层
3. **维护成本优先**：一套 Flutter 代码跑三端，不单独维护鸿蒙 ArkTS 版（初期）
4. **格式兼容**：移动端存储 Markdown，同步到 Supabase 后桌面端可正常读取（Tiptap 支持 Markdown）

---

## 分层架构

```
┌──────────────────────────────────────────────────────────┐
│  UI 层 — Screens & Widgets                               │
│  HomeScreen · EditorScreen · SearchScreen                │
│  TrashScreen · SettingsScreen · AuthScreen               │
└──────────────────┬───────────────────────────────────────┘
                   │ ref.watch / ref.read
┌──────────────────▼───────────────────────────────────────┐
│  状态层 — Riverpod Providers (Notifier / AsyncNotifier)  │
│  workspaceProvider · authProvider · syncProvider         │
│  themeProvider                                           │
└──────────────────┬───────────────────────────────────────┘
                   │ 调用
┌──────────────────▼───────────────────────────────────────┐
│  服务层 — Services                                        │
│  FolderService · DocumentService · VersionService        │
│  AuthService · SyncEngine · SupabaseService              │
└──────────────────┬───────────────────────────────────────┘
                   │ SQL
┌──────────────────▼───────────────────────────────────────┐
│  数据层 — AppDatabase (sqflite)                          │
│  note_folder · note_document                             │
│  note_document_version · sync_meta                       │
└──────────────────┬───────────────────────────────────────┘
                   │ HTTPS / Realtime
┌──────────────────▼───────────────────────────────────────┐
│  远端 — Supabase                                         │
│  Auth (RLS) · Realtime Sync                              │
└──────────────────────────────────────────────────────────┘
```

---

## 目录结构

```
app-mobile/lib/
├── models/
│   └── models.dart              # Folder · Document · DocumentVersion（与桌面端兼容）
├── database/
│   └── app_database.dart        # DAO + schema + WAL + 按用户分库
├── services/
│   ├── folder_service.dart      # 文件夹 CRUD + 树操作
│   ├── document_service.dart    # 文档 CRUD + 800ms 防抖保存
│   ├── version_service.dart     # 版本快照 + 分层合并策略
│   ├── auth_service.dart        # Supabase 认证封装
│   ├── sync_engine.dart         # 5 步同步引擎（核心）
│   └── supabase_service.dart    # Supabase 客户端单例
├── providers/
│   ├── workspace_provider.dart  # 文件夹树 + 文档列表 + 选中状态（乐观更新）
│   ├── auth_provider.dart       # 登录状态 + 持久化
│   ├── sync_provider.dart       # 同步状态机
│   └── theme_provider.dart      # 亮/暗/系统三态
├── screens/
│   ├── home_screen.dart         # 文件夹树 + 文档列表（手机：底部导航；平板：双栏）
│   ├── editor_screen.dart       # Markdown 编辑 + 预览双模式
│   ├── search_screen.dart       # 全局搜索（标题 + 正文）
│   ├── trash_screen.dart        # 回收站
│   ├── settings_screen.dart     # 账号 · 同步 · 主题 · AI 密钥
│   └── auth_screen.dart         # 登录 / 注册
├── widgets/
│   ├── folder_tree_tile.dart    # 文件夹树节点（展开/折叠/缩进）
│   ├── doc_card.dart            # 文档卡片（标题 + 摘要 + 时间）
│   ├── sync_status_banner.dart  # 同步状态横幅
│   └── editor_toolbar.dart      # 编辑器工具栏（Markdown 快捷输入）
├── config/
│   ├── constants.dart           # DB 名、版本号、同步间隔等常量
│   ├── supabase_config.dart     # Supabase URL + anon key
│   └── router.dart              # go_router 路由配置
├── app.dart                     # MaterialApp + Provider 注入
└── main.dart                    # 初始化入口
```

---

## 关键设计决策

### 编辑器选型：Markdown 双模式

**不使用** WebView 内嵌 Tiptap，理由：
- 功能对等不等于格式对等，手机移除了导出、脑图等复杂功能
- 原生 Flutter 编辑器性能更好，调试更友好

**选用方案**：编辑/预览双模式（类 Bear）
- **编辑模式**：`TextField` + Markdown 语法高亮（`re_editor` 或轻量 `code_text_field`）
- **预览模式**：`flutter_markdown` 渲染
- **工具栏**：常用 Markdown 快捷键（粗体、列表、代码块、链接）
- **存储格式**：Markdown 字符串（而非 HTML）

**同步兼容说明**：
- 桌面端目前存 HTML，移动端存 Markdown
- 同步时两端通过 `content` 字段交换内容
- 短期方案：两端各自渲染自己的格式，不做互转
- 长期方案：桌面端支持 Markdown 输入，统一格式

### 按用户分库

与桌面端一致：
- 未登录 → `woo.db`
- 登录后 → `woo-{userId}.db`
- 首次登录：将 `woo.db` 数据复制到用户库

```dart
// AppDatabase 需要支持动态路径切换
static Future<void> switchUser(String? userId) async {
  await close();
  _dbName = userId != null ? 'woo-$userId.db' : 'woo.db';
  _db = null; // 下次访问时重新初始化
}
```

### 同步引擎（SyncEngine）5 步流程

与桌面端完全一致：

```
1. 拉取墓碑  → 硬删除本地对应行 + 级联删除版本
2. 拉取远端  → 合并到本地 SQLite（最后写入胜出）
3. 推送本地  → upsert 到 Supabase
4. 清理      → 扫描 deleted=2 超清理窗口 → 硬删除 + 写入墓碑
5. 墓碑 GC   → 回收 30 天前的 Supabase 墓碑
```

**移动端特有处理**：
- **前台**：`Timer.periodic(60s)` 触发同步
- **后台**（Android）：`WorkManager` + 15 分钟周期任务
- **后台**（iOS）：`BGAppRefreshTask`，系统调度
- **网络感知**：`connectivity_plus` 断线暂停，重连立即补同步
- **冲突处理**：同桌面端，保存本地版本为 `_conflict` 副本

### 状态层：Riverpod Notifier 模式

```dart
// 推荐模式：AsyncNotifier（而非旧版 StateNotifier）
@riverpod
class WorkspaceNotifier extends _$WorkspaceNotifier {
  @override
  Future<WorkspaceState> build() async {
    return WorkspaceState.empty();
  }

  Future<void> selectFolder(String folderId) async {
    // 乐观更新 → 出错回滚
  }
}
```

### 路由：go_router

```dart
// 三级路由结构
/             → HomeScreen（文件夹列表）
/folder/:id   → 文档列表（手机：新页面；平板：侧边栏更新）
/doc/:id      → EditorScreen
/search       → SearchScreen
/trash        → TrashScreen
/settings     → SettingsScreen
/auth         → AuthScreen
```

---

## 依赖列表（更新后）

```yaml
dependencies:
  flutter:
    sdk: flutter

  # 数据库
  sqflite: ^2.3.0
  path_provider: ^2.1.1
  path: ^1.8.3

  # 云同步
  supabase_flutter: ^2.5.0

  # 状态管理
  flutter_riverpod: ^2.5.0
  riverpod_annotation: ^2.3.0  # 代码生成

  # 路由
  go_router: ^13.0.0

  # 编辑器 & Markdown
  flutter_markdown: ^0.6.22
  re_editor: ^0.3.0             # 代码/Markdown 编辑（候选）

  # 网络感知
  connectivity_plus: ^5.0.0

  # UI 辅助
  intl: ^0.19.0
  share_plus: ^7.2.2
  uuid: ^4.3.3

dev_dependencies:
  flutter_test:
    sdk: flutter
  flutter_lints: ^3.0.1
  riverpod_generator: ^2.3.0   # 代码生成
  build_runner: ^2.4.0
```

---

## 不做的事（移动端 MVP 裁减）

| 桌面端功能 | 移动端状态 | 原因 |
|-----------|-----------|------|
| 思维导图导出 | 不做 | 手机屏幕不适合 |
| PDF / WebP 导出 | 不做 | 使用频率低 |
| AI 聊天面板 | 不做（暂） | 架构预留，后期加 |
| 版本历史可视化 | 简化 | 只做列表+恢复，不做 diff 视图 |
| 拖拽排序文件夹 | 简化 | 改为长按菜单+上移/下移 |

---

## 鸿蒙适配策略

初期：使用 `flutter-ohos`（flutter for HarmonyOS Next）一套代码三端运行

关注点：
- `sqflite` 在鸿蒙的适配（可能需要 `sqflite_ohos` 适配包）
- 文件路径权限（鸿蒙 Stage 模型的沙箱路径与 Android 不同）
- `connectivity_plus` 的鸿蒙实现

长期：如鸿蒙市场份额显著，考虑维护独立 ArkTS 版

---

*最后更新：2026-06-02*
