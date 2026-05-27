# MEMORY.md — 项目记忆索引

记忆文件存储在 `.claude/memory/` 目录下，纳入 git 仓库，跨设备同步。

### 跨设备同步

```bash
# 新设备拉取后，将项目记忆同步到 Claude 系统路径：
./scripts/sync-memory.sh --pull

# 修改/添加记忆后，提交前同步到项目目录：
./scripts/sync-memory.sh
git add .claude/memory/ && git commit -m "chore: 同步记忆" && git push
```
