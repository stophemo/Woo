---
name: sync-memory
description: 跨设备记忆同步 — 将 ~/.claude/projects/D--dev-project-Woo/memory/ 与仓库 .claude/memory/ 双向同步
---

# sync-memory

跨设备同步 Claude 记忆：`~/.claude/projects/<projectDir>/memory/` ↔ `.claude/memory/`

## 用法

由 CLAUDE.md 规则自动触发，无需手动调用。

- **拉取代码后**: 自动执行 `scripts/sync-memory.sh --pull`，将远端记忆写入本地系统路径
- **推送代码前**: 自动执行 `scripts/sync-memory.sh`，将本地系统记忆写回 `.claude/memory/` 并纳入提交
