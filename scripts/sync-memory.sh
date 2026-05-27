#!/usr/bin/env bash
# sync-memory.sh — 跨设备 Claude 记忆同步
#
# 用法:
#   ./scripts/sync-memory.sh          将系统记忆同步到项目目录（提交前执行）
#   ./scripts/sync-memory.sh --pull   将项目记忆同步到系统路径（新设备拉取后执行）
#
# Claude Code 将记忆存储在 ~/.claude/projects/<projectDir>/memory/
# 本脚本将其与 git 项目目录下的 .claude/memory/ 同步，实现跨设备共享。

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
PROJECT_NAME="D--dev-project-Woo"

SYSTEM_MEMORY="$HOME/.claude/projects/$PROJECT_NAME/memory"
PROJECT_MEMORY="$PROJECT_DIR/.claude/memory"

case "${1:-}" in
  --pull|-p)
    echo "← 从项目记忆同步到系统路径..."
    mkdir -p "$SYSTEM_MEMORY"
    if [ -d "$PROJECT_MEMORY" ]; then
      cp -r "$PROJECT_MEMORY"/* "$SYSTEM_MEMORY/" 2>/dev/null || true
      echo "  完成！$PROJECT_MEMORY → $SYSTEM_MEMORY"
    else
      echo "  项目记忆目录不存在，跳过"
    fi
    ;;
  *)
    echo "→ 从系统记忆同步到项目目录..."
    mkdir -p "$PROJECT_MEMORY"
    if [ -d "$SYSTEM_MEMORY" ]; then
      cp -r "$SYSTEM_MEMORY"/* "$PROJECT_MEMORY/" 2>/dev/null || true
      echo "  完成！$SYSTEM_MEMORY → $PROJECT_MEMORY"
      echo ""
      echo "  现在可以提交推送记忆文件："
      echo "    git add .claude/memory/ && git commit -m 'chore: 同步记忆' && git push"
    else
      echo "  系统记忆目录不存在，跳过"
    fi
    ;;
esac
