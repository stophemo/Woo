# sync-memory — Claude 记忆跨设备同步 skill

## 概述

自动同步 Claude 记忆：`~/.claude/projects/<projectDir>/memory/` ↔ 仓库 `.claude/memory/`

记忆在 **系统路径**（Claude 读写）和 **项目路径**（git 跟踪）之间双向同步，实现多设备共享。

## 使用方式

### 自动触发（推荐）

日常使用只需正常与我对话，以下操作会自动处理记忆同步：

| 你说 | 我自动做 |
|---|---|
| "拉取代码" / "pull" | `git pull` → 自动 `./scripts/sync-memory.sh --pull` |
| "提交推送" / "push" | 自动 `./scripts/sync-memory.sh` → `git add .claude/memory/` → commit → push |

### 手动触发

```
/sync-memory
```

等同于手动执行：
```bash
# 系统 → 项目（提交前同步记忆到仓库）
./scripts/sync-memory.sh
git add .claude/memory/ && git commit -m "chore: 同步记忆"

# 项目 → 系统（新设备拉取后首次写入本地）
./scripts/sync-memory.sh --pull
```

## 原理

```
┌─────────────────────────────────────────┐
│  ~/.claude/projects/D--dev-project-Woo/ │  ← Claude 直接读写（系统路径）
│    memory/                               │
└──────────────┬──────────────────────────┘
               │  sync-memory.sh（双向同步）
               ▼
┌─────────────────────────────────────────┐
│  .claude/memory/                         │  ← git 跟踪（项目路径）
└─────────────────────────────────────────┘
```

新设备拉取后，仓库中的 `.claude/memory/` 通过 `--pull` 写入系统路径，Claude 才能读取到记忆。

## 文件结构

```
.claude/
├── memory/              # 记忆文件（git 跟踪）
│   ├── MEMORY.md        # 记忆索引
│   └── *.md             # 各条记忆
├── skills/
│   └── sync-memory.md   # skill 定义
└── settings.local.json  # 本地配置（git 忽略）
scripts/
└── sync-memory.sh       # 同步脚本
```
