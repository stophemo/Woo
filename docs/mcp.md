# 本地 MCP 服务

Woo 桌面端内置一个本地 **MCP（Model Context Protocol）** 服务，让 Claude Code、Codex、Hermes、Pi 等支持 Streamable HTTP 的 AI 工具搜索、读取和写入笔记。

## 安全模型

- **仅本机**：只监听 `127.0.0.1`，不开放局域网或公网。
- **Bearer Token**：设置页生成并展示令牌；轮换后旧客户端立即 401。
- **默认只读**：读工具始终可用；写工具需要在「设置 → MCP 服务 → 允许 AI 修改笔记」中手动开启。
- **锁定内容不可访问**：锁定文稿/锁定目录下的内容不会通过 MCP 返回，写入也会被拒绝。
- **软删除**：`delete_note` 只把文稿移入回收站，可用 `restore_note` 恢复；不提供硬删除/清空回收站。
- **草稿不可见**：MCP 只操作 SQLite 中的正式文稿，不能访问 `localStorage` 草稿。
- **随应用运行**：服务在 Woo 启动时按配置恢复，Woo 退出后不可用。

## 启用

1. 打开 Woo → 设置 → **MCP 服务** → 启用。
2. 复制设置页显示的地址（形如 `http://127.0.0.1:47823/mcp`）和令牌。
3. 按下面的客户端示例配置 AI 工具。
4. 需要 AI 写入时勾选「允许 AI 修改笔记」；只读时写工具会返回 `READ_ONLY`。

> 端口被占用时 Woo 会在 `47823–47832` 内回退，并在设置页显示实际地址。请以设置页为准。

## 客户端配置

### Claude Code

```bash
claude mcp add --transport http woo http://127.0.0.1:47823/mcp \
  --header "Authorization: Bearer <token>"
```

首次调用时 Claude Code 会请求工具权限，允许 `mcp__woo__*` 即可。

### Codex

编辑 `~/.codex/config.toml`：

```toml
[mcp_servers.woo]
url = "http://127.0.0.1:47823/mcp"
http_headers = { Authorization = "Bearer <token>" }
```

也可以用 `bearer_token_env_var = "WOO_MCP_TOKEN"`，并在 shell 中设置 `WOO_MCP_TOKEN`。

> Codex 的审批策略可能会拦截 MCP 工具调用。若看到 “MCP tool call requires approval”，请在 Codex 的审批设置中允许 Woo 的工具，或在受信任的本地会话中调整审批策略。

### Hermes

编辑 `~/.hermes/config.yaml`：

```yaml
mcp_servers:
  woo:
    url: http://127.0.0.1:47823/mcp
    headers:
      Authorization: Bearer <token>
    enabled: true
```

修改后执行 `hermes mcp test woo` 或 `/reload-mcp`。

### Pi

编辑 `~/.pi/agent/mcp.json` 或项目的 `.pi/mcp.json`：

```json
{
  "mcpServers": {
    "woo": {
      "type": "http",
      "url": "http://127.0.0.1:47823/mcp",
      "headers": { "Authorization": "Bearer <token>" }
    }
  }
}
```

用 `pi mcp list` 检查连接与工具列表。

### 通用

任何支持 Streamable HTTP 的 MCP 客户端：

```text
URL: http://127.0.0.1:47823/mcp
Authorization: Bearer <token>
```

### Claude Desktop（可选桥接）

Claude Desktop 的本地 MCP 配置只接受 stdio 命令，需要用 `mcp-remote` 桥接：

```json
{
  "mcpServers": {
    "woo": {
      "command": "npx",
      "args": [
        "-y", "mcp-remote",
        "http://127.0.0.1:47823/mcp",
        "--allow-http",
        "--header", "Authorization: Bearer <token>"
      ]
    }
  }
}
```

## 工具

### 读工具

| 工具 | 说明 |
|---|---|
| `list_folders` | 目录树（id、名称、父目录、锁定状态） |
| `list_notes` | 文稿元数据 + 短摘要；可按 `folderId` 过滤，支持 `limit`/`offset` |
| `search_notes` | 按标题/正文关键词搜索，返回摘要，不返回全文 |
| `get_note` | 读取单篇；`format` 默认 `markdown`，可选 `html`；返回 `contentHash`/`updateTime` |
| `list_trash` | 回收站文稿 |
| `list_note_versions` | 版本历史摘要 |
| `get_note_version` | 指定版本正文 |

### 写工具（需开启「允许 AI 修改笔记」）

| 工具 | 说明 |
|---|---|
| `create_note` | 在指定目录新建；标题缺省时从正文首行推导 |
| `update_note` | 覆盖正文；默认保留原标题；必须传 `expectedContentHash` |
| `append_note` | 追加正文；保留原标题；必须传 `expectedContentHash` |
| `rename_note` | 只改标题 |
| `move_note` | 移动到另一个目录 |
| `create_folder` | 新建目录 |
| `rename_folder` | 重命名目录 |
| `delete_note` | 软删除到回收站；需要 `confirm: true` |
| `restore_note` | 从回收站恢复 |

`update_note`/`append_note` 的 `expectedContentHash` 来自 `get_note` 的 `contentHash`。若文稿在读取后已被修改，工具返回 `CONFLICT`，并附带当前 hash/updateTime，重新读取后再试即可。

## 并发与前端刷新

- MCP 写入会先保存一条版本快照，再更新正文；之后触发同步并通知 Woo 刷新。
- 如果目标文稿正在 Woo 中编辑且有未保存修改，写入返回 `EDITOR_BUSY`，请先保存或等待自动保存完成。
- 当前正在打开的文稿在无未保存修改时，Woo 会加载 MCP 写入的新内容；按现有同步冲突策略，旧内容可能被保存为 `_conflict` 副本。
- 「全部文稿」和「搜索结果」视图不会自动刷新；重新进入对应视图即可看到更新。

## 故障排查

| 现象 | 处理 |
|---|---|
| 连接被拒绝 | 确认 Woo 正在运行，MCP 服务已启用，地址/端口与设置页一致 |
| 401 Unauthorized | 令牌错误或已轮换；从设置页重新复制 |
| 403 Forbidden | `Origin` 不是本机地址；请使用本机 CLI 客户端 |
| 405 Method Not Allowed | MCP 端点只接受 POST；检查客户端是否按 Streamable HTTP 配置 |
| 端口被占用 | Woo 会自动回退端口；以设置页 URL 为准 |
| 写工具返回 `READ_ONLY` | 在设置中开启「允许 AI 修改笔记」 |
| 写工具返回 `EDITOR_BUSY` | 目标文稿正在 Woo 中编辑且有未保存修改 |
| 写工具返回 `LOCKED` | 文稿或目录已锁定；先在 Woo 中解锁 |
| 看不到最新内容 | 重新进入「全部文稿/搜索结果」，或等待同步刷新 |

日志位于数据目录的 `logs/woo.log`，MCP 相关行以 `[MCP]` 开头；日志不会记录令牌或正文。

## 开发与测试

```bash
# Rust 测试（MCP smoke test 默认 ignore，需显式串行执行）
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo test --manifest-path src-tauri/Cargo.toml --lib mcp_smoke -- --ignored --test-threads=1

# 前端类型检查 + 构建
npm run build
```

MCP smoke test 覆盖：初始化、tools/list、目录/文稿读取、只读拒绝、写入、hash 冲突、软删除与恢复。
