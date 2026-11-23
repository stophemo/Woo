# Woo 同步空间后端

运行在 Cloudflare Workers + D1 上的本地优先增量同步服务，供 Woo（无我笔记）的多设备同步空间使用。客户端始终先写本机 SQLite；服务端只保存客户端产生的 AES-256-GCM 密文、分配递增游标并转发变更，不读取笔记正文。

## 安全边界

- 每台设备持有独立的 32 字节随机 Bearer 令牌；服务端用 `TOKEN_PEPPER` 做 HMAC-SHA-256 后只存哈希。
- vault 密钥由客户端生成，只保存在设备本机；服务端永远不持有。
- 六位配对码由服务端生成并只存哈希，默认 5 分钟有效；发起设备仍需人工确认新设备。
- 创建同步空间必须提供部署者设置的邀请码（16–256 位可打印 ASCII），只通过 `X-Woo-Sync-Invite-Code` Header 传输。
- 每个空间最多 4 台未撤销设备。
- HTTP 仍必须使用 Cloudflare 提供的 HTTPS；应用层密文不替代 TLS。
- 服务端不知道正文内容、标题或图片；只看到 vault/device/entity、操作类型、Lamport、密文大小和时间等元数据。

## 本地开发

需要 Node.js 22.18 及以上。

```bash
cd sync-server
npm install
cp .dev.vars.example .dev.vars
# 编辑 .dev.vars，把 TOKEN_PEPPER 和 VAULT_CREATION_INVITE_CODE 换成开发值
npx wrangler d1 migrations apply woo-sync --local
npx wrangler dev
```

运行测试与类型检查：

```bash
npm test
npm run typecheck
```

## 面向个人的 Cloudflare 免费部署

Cloudflare 账号、Workers/D1 免费额度、计费规则和中国大陆网络可达性都由 Cloudflare 决定，可能随时间或地区变化；部署前请在 Cloudflare Dashboard 核对当前计划与用量。项目按个人多端低频负载设计，不承诺永久零费用。

### 1. 创建 D1

```bash
cd sync-server
npm install
npx wrangler login
npx wrangler d1 create woo-sync
```

把输出的 `database_id` 替换到 `wrangler.toml` 的 `[[d1_databases]]` 块，保持 `binding = "DB"`、`database_name = "woo-sync"`、`migrations_dir = "migrations"` 不变。

### 2. 配置 secrets

```bash
npx wrangler secret put TOKEN_PEPPER
npx wrangler secret put VAULT_CREATION_INVITE_CODE
```

- `TOKEN_PEPPER` 至少 32 个字符，建议用 `openssl rand -hex 32` 生成；长期保持不变。更换后所有已有设备令牌无法认证。
- `VAULT_CREATION_INVITE_CODE` 16–256 位无空格可打印 ASCII，是创建新空间的部署级门禁，不是账号；轮换它不影响已有空间。

### 3. 迁移并部署

```bash
npx wrangler d1 migrations apply woo-sync --remote
npx wrangler deploy
```

部署成功后会输出 Worker URL（通常形如 `https://woo-sync.<你的 workers 子域>.workers.dev`）。先检查健康接口：

```bash
curl -fsS 'https://woo-sync.<你的 workers 子域>.workers.dev/health'
```

响应应包含 `"ok": true` 和 `"service": "woo-notes-sync"`。

### 4. 在 Woo 中创建/加入空间

1. 第一台设备：Woo → 设置 → 同步空间 → 填写 Worker 根地址、创建邀请码、本机名称 → 创建空间。
2. 其他设备：填写同一地址与六位配对码 → 加入空间；已有设备会显示新设备名称和六位核对码，确认后完成配对。
3. 加入设备只拉取远端数据；本机独有文稿可在「同步空间」中点击「上传本机独有文稿」按需上传。

## API

除健康检查、创建空间、短码解析和配对结果外，其余接口要求：

```http
Authorization: Bearer <32字节无填充Base64URL设备令牌>
Content-Type: application/json
```

| 方法与路径 | 鉴权 | 用途 |
|---|---:|---|
| `GET /health` | 否 | Worker 与 D1 健康检查 |
| `POST /v1/vaults` | 创建邀请码 | 创建空间与首台设备 |
| `POST /v1/pairings/code` | 是 | 生成六位配对码（5 分钟） |
| `POST /v1/pairings/resolve` | 否 | 新设备解析六位码 |
| `POST /v1/pairings/:id/claim` | 否 | 新设备认领 |
| `GET /v1/pairings/:id` | 是 | 发起设备查看认领状态 |
| `POST /v1/pairings/:id/confirm` | 是 | 发起设备确认并提交 vault key 密文 |
| `POST /v1/pairings/:id/result` | 否 | 新设备领取确认结果 |
| `POST /v1/sync` | 是 | 原子 push、ack 与 pull |
| `GET /v1/ops/:opId` | 是 | 按需补齐内容块 |
| `GET /v1/devices` | 是 | 查看同空间设备 |
| `POST /v1/devices/:id/revoke` | 是 | 撤销另一台设备 |
| `GET/PUT /v1/relay` | 是 | 预留的 relay 配置 |

### 增量同步

```json
{
  "cursor": 18,
  "ack": 18,
  "pullLimit": 100,
  "push": [
    {
      "opId": "op:设备生成的唯一ID",
      "entityId": "doc:文稿UUID",
      "kind": "upsert",
      "lamport": 42,
      "ciphertext": "AES-GCM 密文",
      "nonce": "12字节nonce的Base64URL"
    }
  ]
}
```

`server_seq` 是 `change_log` 的全局自增序号，不同空间之间可能形成空洞。`cursor = 0` 表示从头同步；非零 cursor 必须对应当前空间真实存在的 `server_seq`，否则返回 `409 CURSOR_NOT_FOUND`；超过当前最大值返回 `409 CURSOR_AHEAD`。客户端只在整页操作落地后推进 cursor。

服务端不执行冲突合并；`(lamport, deviceId)` LWW 与删除语义由客户端在解密后确定。

### 正文分块

Woo 的文稿正文（含 base64 图片）可能达到数 MB。客户端把正文按内容定义边界切成不超过 24 KiB 的块，压缩后 AES-256-GCM 加密为独立操作；文稿操作只包含少量「内容清单分片」引用，因此单条密文不会超过 32 KiB。接收端按哈希缓存内容块，缺块时通过 `GET /v1/ops/:opId` 补齐。

## 固定限制

- 单个 JSON 请求：2 MiB
- 每次 push：最多 50 条（Woo 客户端按 30 条分批）
- 每次 pull：最多 100 条
- 单条操作密文：最多 32 KiB
- AES-GCM nonce：12 字节
- 设备名：最多 80 个字符
- 配对短码：5 分钟有效，每空间最多 3 个活跃码
- 每个同步空间：最多 4 台未撤销设备
- 每个同步空间：最多 200000 条密文操作、300 MiB 解码后密文（适配 D1 免费档单库约 500 MB）

## 已知限制

- 当前没有历史压缩/快照裁剪 API；操作日志会持续增长，接近 300 MiB 时返回 `507 VAULT_CAPACITY_REACHED`。后续版本需要客户端加密快照 + 全设备确认后裁剪日志。
- 新设备从 `cursor = 0` 重放全部历史；空间很大时首次同步会比较慢。
- 一条无法解密的操作会阻塞该空间后续同步；需要后续版本的隔离/跳过协议。
- 没有恢复码；vault key 只在本机配置中。请保留至少一台已配对设备，并备份应用数据目录中的 `vault-sync-config.json`。
- `workers.dev` 在中国大陆的可达性不稳定；如日常使用，请绑定自己的域名并实测，或改用自建兼容服务。
