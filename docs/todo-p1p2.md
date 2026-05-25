# P1 / P2 待修复清单

## P1 — 重要

### 1. `updateHeadings()` 在大幅编辑时被跳过

**文件**: `app-desktop/src/components/layout/EditArea.vue` L285

**根因**: `onUpdate` 回调中，当 `charDelta >= CHAR_DELTA || lineDelta >= LINE_DELTA` 时直接 `return`，跳过了 `updateHeadings()` 的调用。

**后果**: 用户输入超过 100 字符或 10 行时大纲标题列表不更新。

**修复方向**: 将 `updateHeadings()` 移到 `triggerCommit()` 之前或条件之外。

---

### 2. `loadVersions()` 缺少竞态防护

**文件**: `app-desktop/src/components/layout/ThumbnailColumn.vue` L195-L209

**根因**: `loadVersions(docId)` 异步请求完成后直接赋值 `versions.value = list`，没有检查 `activeDocId` 是否仍然匹配。

**后果**: 快速连续翻转两个文档时，先请求的后返回，版本列表显示错误数据。

**修复方向**: 在请求返回后加 `if (activeDocId.value !== docId) return` 防护（参考 `refreshVersions` 的写法）。

---

### 3. macOS 原生菜单 `Cmd+,` 快捷键重复注册

**文件**: `app-desktop/electron/main.cjs` L165 L184

**根因**: 系统菜单模板中"设置"项在 appMenu 和 文件菜单中分别注册了 `accelerator: 'Cmd+,'`。

**后果**: macOS 上显示两条"设置"快捷键。

**修复方向**: 移除文件菜单中的 `Cmd+,` 快捷键，appMenu 中的保留即可。

---

## P2 — 代码质量

### 4. `documentApi.ts` 声明了不存在的 `userId` 字段

**文件**: `app-desktop/src/services/documentApi.ts` L8

**根因**: `DocumentDTO` 接口声明了 `userId: string`，但本地 SQLite `note_document` 表没有 `user_id` 列，`toDto()` 也从未返回此字段。

**后果**: TS 类型与运行时数据不一致。

**修复方向**: 移除 `userId` 字段。

---

### 5. `updateHeadings` 在文档切换时调用时机偏早

**文件**: `app-desktop/src/components/layout/EditArea.vue` L329

**根因**: `setContent()` 是 ProseMirror 事务（异步），但 `updateHeadings()` 在下一行同步执行。

**后果**: 少数情况下编辑器尚未完成渲染，`doc.descendants` 可能拿到旧数据。

**修复方向**: 使用 `nextTick` 或在编辑器 `onTransaction` 事件中更新。

---

### 6. 搜索栏失焦态 200ms 延时意外关闭搜索

**文件**: `app-desktop/src/components/layout/LeftSidebar.vue` L121-L128

**根因**: `handleSearchBlur` 用 `setTimeout(200ms)` 关闭搜索模式。

**后果**: 用户在搜索结果列表中点击一项时，输入框失焦 → 200ms 后关闭搜索 → 用户点击的搜索结果消失。

**修复方向**: 改用 `relatedTarget` 判断焦点是否移到了搜索结果区域内，不移出则不关闭。

---

### 7. `activeHeadingIndex` 声明了但从未更新

**文件**: `app-desktop/src/components/layout/ThumbnailColumn.vue` L164

**根因**: `activeHeadingIndex` 只在点击时赋值，没有监听编辑器滚动同步高亮当前章节。

**后果**: 大纲面板无法高亮当前阅读的节标题。

**修复方向**: 在 EditArea 的 `onUpdate` 或滚动事件中计算当前视口顶部对应的标题位置，同步到 `activeHeadingIndex`。

---

### 8. CI Release Notes 中 PowerShell `\n` 转义问题

**文件**: `.github/workflows/release-app-desktop.yml` L228

**根因**: PowerShell 中 `\n` 不会被解释为换行（需要使用反引号 `` `n ``），导致模板中的 `---\n\n` 不产生 Markdown 分割线。

**后果**: Release notes 中"---"显示为文本而非分割线。

**修复方向**: 将 `\n` 替换为 `` `n ``，或使用 `[System.Environment]::NewLine` 拼接多行文本。
