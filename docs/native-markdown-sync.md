# 原生 Markdown 与操作记录同步实施方案

> 状态：设计定稿，待排期
> 目标版本：0.9.x 起步，分阶段发布
> 关联模块：`src-tauri/src/services/webdav_space`、`vault_sync`、`woo-sync-core`

## 1. 背景与目标

当前 Woo 的正文以 HTML 存在 SQLite（`note_document.content`），同步传的是 HTML 文档快照，存在三个问题：

1. 不是原生 Markdown，数据不可直接迁移、不能被外部工具和 AI 直接读取。
2. 整篇快照同步，改一个字也要传整篇，浪费流量。
3. 密钥槽（keySlot）用 WebDAV 应用密码包裹 vaultKey，服务商知道应用密码即可解密。

本方案的目标：

- **本地 `.md` 文件是唯一事实源**，SQLite 只做索引、块表和同步队列。
- **云端只存加密的操作记录和内容块**，不存明文 Markdown。
- **增量同步**：只传变化的块和操作，不传整篇。
- **扫码配对**：vaultKey 只在设备之间传递，云端没有 keySlot。
- **恢复码兜底**：没有旧设备时也能恢复。

## 2. 范围

### 本期做

- 本地 Markdown 文件库（打开目录、索引、文件监听）。
- 块级切分、块级 diff、操作生成与应用。
- 加密的操作日志同步、冲突副本、快照、垃圾回收。
- 扫码配对、恢复码、退出同步空间、新建同步空间、旧 keySlot 兼容。
- 旧 SQLite HTML 数据迁移工具（M5）。
- 桌面端与 Android 端随 M1–M4 同步实现；Android 使用应用私有目录 + 导入导出。

### 本期不做

- 实时协作、多人权限。
- 字符级 CRDT 合并。
- 云端明文 Markdown（明确放弃）。
- iOS 文件系统深度集成（后续版本）。
- 旧 HTML 编辑器的完全移除（保留只读/导入）。
- 移除设备功能（共享 vaultKey 下无法真正撤销，做了会误导用户）。

## 3. 总体架构

```
┌──────────────── 本地 ────────────────┐
│  vault/*.md + attachments/            │  ← 事实源，明文，AI/外部编辑器可读
│  SQLite：文件索引、块表、outbox、水位  │
│  文件监听 → 块级 diff → 操作           │
└───────────────┬──────────────────────┘
                │ 加密 ops / blocks
┌───────────────▼──────────────────────┐
│  WebDAV：ops/、blocks/、snapshots/     │  ← 全部密文
│  state.json：设备水位、最新快照         │
└───────────────┬──────────────────────┘
                │ 拉取、校验、应用
┌───────────────▼──────────────────────┐
│  本地块表 → 重建 .md → 抑制监听回环     │
└──────────────────────────────────────┘
```

核心不变式：

1. 块保存 Markdown 原文切片，重建只拼接，不做 `parse → serialize`。
2. 操作只描述差异，不携带全文。
3. 云端没有 keySlot，也没有 vaultKey 明文。

## 4. 本地数据模型

### 4.1 文件库

用户选择一个目录作为 vault：

```
WooNotes/
├── 笔记/
│   ├── 会议记录.md
│   └── 设计.md
└── attachments/
    └── 截图.png
```

- 每篇 `.md` 是普通文件，UTF-8。
- 附件是独立文件，Markdown 里用相对路径引用。
- 文件 `fileId` 用 UUID，与路径解耦；路径是属性，重命名不重传内容。
- 空文件夹也要有 `folderId`，否则结构会丢。

### 4.2 SQLite 表

```sql
CREATE TABLE file_index (
    file_id         TEXT PRIMARY KEY,
    path            TEXT NOT NULL UNIQUE,
    content_hash    TEXT NOT NULL,
    mtime_ms        INTEGER,
    size_bytes      INTEGER,
    manifest_version INTEGER NOT NULL DEFAULT 0,
    deleted         INTEGER NOT NULL DEFAULT 0,
    updated_at      TEXT NOT NULL
);

CREATE TABLE block (
    hash        TEXT PRIMARY KEY,   -- SHA-256(原文切片)
    content     TEXT NOT NULL,      -- 原文，不重新序列化
    plain_bytes INTEGER NOT NULL,
    refcount    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE file_block (
    file_id    TEXT NOT NULL,
    position   INTEGER NOT NULL,
    block_hash TEXT NOT NULL,
    PRIMARY KEY (file_id, position)
);

CREATE TABLE op_outbox (
    op_id      TEXT PRIMARY KEY,
    device_id  TEXT NOT NULL,
    lamport    INTEGER NOT NULL,
    seq        INTEGER NOT NULL,
    kind       TEXT NOT NULL,
    payload    TEXT NOT NULL,       -- 本地明文 JSON，上传前加密
    created_at TEXT NOT NULL,
    uploaded   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE applied_op (
    op_id      TEXT PRIMARY KEY,
    applied_at TEXT NOT NULL
);

CREATE TABLE sync_state (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE conflict (
    id          TEXT PRIMARY KEY,
    file_id     TEXT NOT NULL,
    path        TEXT NOT NULL,
    block_hash  TEXT NOT NULL,
    source_device TEXT NOT NULL,
    created_at  TEXT NOT NULL
);
```

索引：

```sql
CREATE INDEX idx_file_block_hash ON file_block(block_hash);
CREATE INDEX idx_op_outbox_seq ON op_outbox(device_id, seq);
CREATE INDEX idx_block_refcount ON block(refcount);
```

### 4.3 块切分

按行扫描，切成块。每块保存**原始文本，包含尾随空行**，这样拼接就是原文。

| 块类型 | 识别规则 |
|---|---|
| frontmatter | 文件开头 `---` 到下一个 `---` |
| heading | `#` 到 `######` 开头 |
| fenced code | ``` 或 ~~~ 到闭合 |
| blockquote | 连续 `>` 行 |
| list | 连续 `-`/`*`/`+`/`1.` 行，含缩进子项 |
| table | 连续 `|` 行 |
| hr | `---`/`***`/`___` |
| paragraph | 连续非空行 |
| raw | 未识别的连续行，原样保留 |

规则：

- `blockHash = SHA-256(原始切片)`。
- 超大块（> 64 KB）按行再切，避免单块过大。
- 空文件、只有 frontmatter、CRLF、BOM、末尾无换行必须有测试用例。
- **round-trip 必须字节级相等**：切块后不改任何块，直接拼接 == 原文件。

### 4.4 文件清单

```json
{
  "fileId": "f_9c1b",
  "path": "项目/设计.md",
  "blocks": ["sha256:a1...", "sha256:b2...", "sha256:c3..."],
  "version": 12
}
```

清单是操作的依据。文件内容 = 按顺序拼接块内容。

## 5. 变更捕获与操作生成

### 5.1 文件监听

- 桌面端使用跨平台文件监听（`notify` crate），监听 vault 目录。
- 移动端没有可靠的文件监听：用应用内保存钩子 + 启动/回前台扫描。
- 过滤临时文件：`.tmp`、`.swp`、`~`、`.DS_Store`、`.git/`。
- 原子写会产生 rename 事件，按最终路径处理。

### 5.2 防抖与抑制

- 事件防抖 500–1000 ms，等 `mtime`/`size` 稳定后再读。
- 同一路径多事件合并。
- 事件超过 100 个/秒 → 转成一次目录扫描。
- 应用远端操作重建文件时设置 `suppress(path)`，忽略监听回调，写完再解除。

### 5.3 块级 diff

```
读文件 → 算 contentHash
  → 和 file_index.content_hash 比
  → 没变：忽略
  → 变了：切块，得到新 blockHash 序列
  → 和旧序列做 Myers diff
  → 生成 insert / delete / replace / move 操作
```

改一个字：

```
旧：[h1, h2, h3, h4]
新：[h1, h2, hX, h4]
→ replace_block(index=2, old=h3, new=hX)
```

### 5.4 操作类型

| 操作 | 载荷 |
|---|---|
| `create_folder` | `folderId, path` |
| `delete_folder` | `folderId, path` |
| `rename_folder` | `folderId, oldPath, newPath` |
| `create_file` | `fileId, path, blocks[]` |
| `delete_file` | `fileId, path, deletedAt` |
| `rename_file` | `fileId, oldPath, newPath` |
| `replace_block` | `fileId, index, oldHash, newHash` |
| `insert_block` | `fileId, index, newHash` |
| `delete_block` | `fileId, index, oldHash` |
| `move_block` | `fileId, from, to, hash` |
| `replace_file` | `fileId, path, blocks[]`（大改/降级） |
| `purge_file` | `fileId`（清空回收站） |

规则：

- `oldHash` 是前置条件，应用时不匹配即冲突。
- 删除只传墓碑，不传块内容。
- 重命名只传路径，不传块。
- 新建带内容时，块单独上传。

## 6. 同步协议

### 6.1 云端布局

```
WooNotes/
└── v2/
    └── <vaultId>/
        ├── space.json                 # protocolVersion, vaultId, createdAt（无 keySlot）
        ├── state.json                 # 最新快照、各设备水位、maxLamport
        ├── ops/
        │   └── <deviceId>/
        │       ├── 000001.json.enc
        │       └── 000002.json.enc
        ├── blocks/
        │   ├── 0/<hash>.json.enc
        │   └── 1/<hash>.json.enc
        ├── snapshots/
        │   └── <snapshotId>/
        │       ├── manifest.json.enc
        │       ├── blocks-0001.bin
        │       └── checksum.json
        └── devices/
            └── <deviceId>.json.enc
```

旧 `v1/` 保留只读兼容，不强制迁移。

### 6.2 操作包

上传前整体加密；包内每条操作的 AAD 绑定 `vaultId|opId|entityId|kind|lamport|deviceId`。

```json
{
  "protocol": 2,
  "vaultId": "v_...",
  "deviceId": "mac-7f3a",
  "seq": 17,
  "prevSeq": 16,
  "maxLamport": 42,
  "ops": [
    {
      "opId": "01J8Z3K4...",
      "lamport": 41,
      "type": "replace_block",
      "fileId": "f_9c1b",
      "path": "项目/设计.md",
      "index": 2,
      "oldHash": "sha256:3a7f...",
      "newHash": "sha256:9b2e..."
    }
  ]
}
```

### 6.3 上传

1. 从 `op_outbox` 取未上传操作，按 `seq` 打包。
2. 一次打包最多 100 条或 256 KB。
3. 加密后 PUT 到 `ops/<deviceId>/<seq>.json.enc`。
4. 更新 `state.json`（设备水位、maxLamport）。
5. 复用现有请求闸门：同账号串行、最小间隔、429/503 退避。

### 6.4 拉取与应用

1. 读 `state.json`，比较本地各设备水位。
2. 只拉缺失的 op 包。
3. 按 `lamport` 排序，同一 Lamport 按 `deviceId`。
4. 校验 `oldHash`：
   - 匹配 → 应用。
   - 不匹配 → 冲突处理。
5. 按文件分组，批量应用，每个文件只重建一次。
6. 重建：临时文件 + 原子 rename + 抑制监听。
7. 更新 `applied_op`、`file_index`、水位。

### 6.5 幂等与顺序

- `applied_op` 按 `opId` 去重，重复包直接跳过。
- 每设备 `seq` 单调递增，`prevSeq` 用于断点续传。
- 跨设备用 Lamport；不要求全局中心顺序。
- `create_file` 未到时收到块操作 → 暂存，等 create 到达再应用。

## 7. 冲突处理

原则：**自动合并不同块，同一块冲突留副本，永不静默丢数据。**

| 场景 | 处理 |
|---|---|
| 改不同块 | 自动合并 |
| 改同一块 | LWW 选一个，另一个存 conflict |
| 插入同一位置 | 按 Lamport 排序，都保留 |
| 删除同一块 | 幂等 |
| 一端删块，一端改块 | 保留修改为 conflict |
| 一端删文件，一端改文件 | 文件进回收站，修改存副本 |
| 两端新建同名 | 一个保留原名，一个 `_conflict` 后缀 |
| 两端重命名不同 | LWW 选一个，另一个记冲突 |

冲突副本命名：

```
设计.conflict-mac-20261127T103211.md
```

副本内容来自被丢弃的块序列，保证可恢复。

## 8. 快照、恢复与 GC

### 8.1 快照

- 每周一次，或每 1000 条操作一次。
- 内容：全量 manifest + 块打包。
- 块包只包含“当前快照引用、且不在上一个快照包里”的块，复用 `blocks/`。
- 先压缩再加密；分卷 10 MB；每卷有哈希。
- 保留最近 2 个基础快照 + 最近增量。

### 8.2 新设备恢复

```
读 state.json
  → 下载最新快照 manifest
  → 下载快照分卷
  → 解压、解密到临时目录
  → 建索引、块表
  → 原子写入 vault
  → 拉取快照之后的 op 包
  → 应用、重建
  → 注册设备、写心跳
```

预估（1 MB/s）：

| 库规模 | 快照大小 | 恢复时间 | 请求数 |
|---|---|---|---|
| 1 千篇 | 6–10 MB | 10–15 秒 | 几个 |
| 1 万篇 | 60–100 MB | 2–3 分钟 | 几十个 |
| 10 万篇 | 600 MB–1 GB | 15–25 分钟 | 几百个 |

### 8.3 垃圾回收

```
标记：当前 file_block + 保留快照 + 未过期 op + 墓碑 引用的块
清除：未标记的块、旧 op、旧快照、过期墓碑
```

- 操作记录：被快照覆盖 + 所有活跃设备水位超过，或超过 30 天。
- 快照：保留 2–3 个。
- 块：`refcount = 0` 才删。
- 墓碑：30 天。
- 冲突副本：30 天或用户手动删。
- 删除先移到 `trash/`，7 天后再硬删。
- 超过 30 天没心跳的设备视为不活跃，不再阻塞 GC。

## 9. 加密与配对

### 9.1 密钥层级

```
vaultKey：32 字节随机，与密码无关
  ├── 数据加密：AES-256-GCM，AAD 绑定 vaultId/opId/entityId/kind/lamport/deviceId
  ├── 本地保存：系统钥匙串 / 0600 加密文件
  ├── 设备传递：扫码一次性二维码
  └── 恢复：恢复码包裹的 vaultKey
```

- 云端**不写 keySlot**（新空间）。
- 应用密码只用于 WebDAV 认证，不参与密钥派生。
- 改应用密码不影响解密。

### 9.2 扫码配对与空间二维码

二维码属于空间，不属于设备。设备在哪个空间，就只能生成哪个空间的加入码。

| 本机状态 | 能生成的二维码 |
|---|---|
| 在空间 A | A 的“添加设备”二维码 |
| 退出后、未建新空间 | 无 |
| 新建了空间 B | 只有 B 的二维码 |
| 加入空间 B | 只有 B 的二维码 |

**添加设备**（加入现有空间）：

```
旧设备：设置 → 添加设备 → 显示二维码（60 秒有效，一次性）
二维码：WOO-PAIR:2:<base64url(JSON{endpoint, username, vaultId, vaultKey, expires})>
新设备：扫码 → 得到连接信息和 vaultKey → 本地安全保存
旧设备：二维码作废
```

**新建同步空间**（生成新空间）：

```
设置 → 新建同步空间 → 生成新 vaultId / vaultKey
  → 云端创建新空间
  → 上传当前文件作为初始快照
  → 显示新空间二维码
  → 其他设备扫码加入新空间
```

**退出同步空间**：

```
设置 → 退出同步空间 → 本机停止同步
  → 清除本机老 vaultKey
  → 本地 .md 保留
  → 老空间写“设备离开”心跳，不删数据
  → 本机不再能生成老空间的二维码
```

约束：

- 二维码一次性，扫完或过期即作废。
- 同地使用，屏幕 → 摄像头，不经过网络。
- 用户自己截图/转发属于用户责任；产品提示“二维码等同临时钥匙，勿公开发送”。
- 后续可选 X25519 会话确认模式，本期不做。

### 9.3 恢复码

恢复码只覆盖一种场景：**没有任何已授权设备可用**。

- 有另一台设备在场 → 扫码加入，不需要恢复码。
- 卸载/清空数据/换平台，只要还有设备可扫码 → 不需要恢复码。
- 设备全丢、唯一设备坏了、旧设备不在 → 才需要恢复码。

形式（二选一或都提供）：

- **恢复文件** `woo-recovery-<vaultId>.key`：保存到密码管理器/网盘/U 盘，新设备导入。
- **恢复码文本**：24 个词或 32 字节 Base64URL，供纸质备份。

实现：

- 用 Argon2id 从恢复码派生 KEK，包裹 vaultKey，存本地 `recovery.json`；**不上传云端**。
- 恢复码只由用户保存在设备之外的离线介质（密码管理器、U 盘、纸质）；不要只存在唯一设备上。
- 初始化时强烈建议生成；跳过时明确警告“设备全部丢失后无法恢复”。
- 如果恢复码和唯一设备一起丢失，数据无法恢复；设置页必须写清这一点。
- 生成后要求用户确认已保存，至少二次确认。
- 恢复码泄露 = 数据泄露，提示用户离线保存。

### 9.4 空间操作语义

| 操作 | 效果 | 云端 |
|---|---|---|
| 添加设备 | 新设备加入当前空间 | 不变 |
| 新建同步空间 | 新 vaultId + 新 vaultKey，本机切换过去 | 新建空间 |
| 退出同步空间 | 本机离开，停止同步，清除本机老 vaultKey | 老空间不变 |
| 删除云端空间 | 删除整个空间 | **所有设备的数据都没了** |

安全边界：

- 不提供“移除设备”：共享 vaultKey 下无法真正撤销，做了会误导用户。
- 要阻止旧设备访问云端：改 WebDAV 应用密码。
- 不做全空间密钥轮换。

危险提示：

- **退出**：本机停止同步，本地文件保留，老空间数据保留；回老空间需用仍在老空间的设备生成二维码。
- **新建**：全新空间，独立占容量，和老空间互不相通；其他设备必须扫新码。
- **删除云端空间**：所有设备的数据都会丢失，需要输入确认词。

### 9.5 旧 keySlot 兼容

- 旧 `v1` 空间仍可用应用密码解 keySlot 加入。
- 新空间不写 keySlot。
- 设置页提供“升级安全模式”：生成恢复码、删除 keySlot。
- 删除后，应用密码不再能解密。

## 10. 迁移

### 10.1 旧 SQLite 数据

```
note_folder → 目录
note_document.content（HTML）→ .md 文件
内嵌图片（base64）→ attachments/ 文件
版本历史 → 可选保留在 SQLite 或导出
```

- 用 `htmd` 做 HTML → Markdown，明确报告有损项。
- 先 dry-run，输出报告：成功、警告、失败。
- 迁移前自动备份原数据库。
- 迁移可重跑、幂等。

### 10.2 旧 WebDAV 空间

- `v1/` 只读兼容，不强制迁移。
- 新空间使用 `v2/`。
- 同一 vault 的 `v1` 和 `v2` 不并行写入。

## 11. 前端与移动端

### 桌面端

- 设置页：扫码加入、添加设备、新建同步空间、退出同步空间、恢复码、设备列表（只读，不提供移除）、删除云端空间（可选）。
- 每个操作写明“加入现有空间”还是“创建新空间”，避免点错。
- 移除“填 4 项自动加入”（新空间）。
- 迁移向导。
- 文件库选择：打开目录、最近目录。

### 移动端（Android）

- 随 M1–M4 同步实现，不单独放 M5。
- 应用私有目录作为 vault，支持导入/导出。
- 后台同步：前台触发，后台低频。
- 扫码：相机权限 + 二维码解析。
- Android SAF 作为后续增强。

## 12. 分阶段计划

### M0 原型与测试（1–2 周）

- Markdown 切块、round-trip 测试集（共享 Rust 核心，桌面/Android 通用）。
- Myers diff 原型。
- 协议草案、表结构评审。
- 验收：30+ 用例字节级相等。

### M1 本地文件库（2–3 周，桌面 + Android）

- 打开目录、索引、文件监听、保存。
- Android 使用应用私有目录，支持导入/导出。
- 块表、file_block、重建。
- 不含网络同步。
- 验收：保存 P95 < 10 ms；round-trip 全过。

### M2 安全配对（1–2 周，桌面 + Android）

- vaultKey、扫码二维码、恢复码。
- 新空间不写 keySlot。
- 退出同步空间、新建同步空间。
- 旧 keySlot 只读兼容。
- Android 相机扫码。
- 验收：云端无 keySlot；新设备扫码可加入；恢复码可恢复；退出后本机不能生成老空间二维码。

### M3 操作日志同步（3–4 周，双端）

- op 生成、打包、加密、上传。
- 拉取、校验、应用、冲突。
- 复用 WebDAV 请求闸门。
- 桌面与 Android 双端互通验证。
- 灰度：默认 op 同步，失败回退整文件。
- 验收：改一个字上传 < 1 KB；同步请求 ≤ 5/轮。

### M4 快照、恢复与 GC（2 周，双端）

- 快照生成、分卷、恢复流程。
- GC、墓碑、引用计数。
- Android 端恢复验证。
- 验收：1 万篇恢复 < 3 分钟；GC 后无孤立块。

### M5 旧数据迁移与收尾（2–3 周）

M5 不是新功能阶段，只做迁移和收尾：

- 旧 SQLite HTML 数据 → Markdown 文件迁移工具。
- 旧 `v1` WebDAV 空间兼容/迁移。
- 性能调优、文档、发布说明。

为什么放最后：迁移目标格式依赖 M1–M4 定稿，提前做会反复改。老用户可先在 M1 后试用 dry-run 迁移预览。

每阶段独立发布小版本，默认关闭新同步内核，灰度开关控制。

## 13. 验收标准

### 功能

- 本地 `.md` 可直接用外部编辑器修改，改动能同步。
- 新建/删除/重命名文件、文件夹、内容块都能同步。
- 两端同时修改不同块可自动合并。
- 同一块冲突保留副本，不丢数据。
- 新设备扫码加入；无旧设备时恢复码恢复。
- 退出同步空间后本机不再同步，其他设备不受影响。
- 新建空间后本机只显示新空间二维码。

### 性能

- 保存本地处理 P95 < 10 ms。
- 空闲 CPU < 1%，不同步时不扫描。
- 一次日常同步请求 ≤ 5。
- 应用 100 条 op < 100 ms（不含网络）。
- 1 万篇首次恢复 < 3 分钟（1 MB/s）。

### 安全

- 云端无 keySlot，服务商知道应用密码也无法解密。
- 二维码一次性、60 秒失效。
- 恢复码强制生成并二次确认。
- 退出同步空间后，本机不能生成老空间二维码。
- 本地 `.md` 明文，文档中明确提示磁盘加密。

### 兼容

- 旧 `v1` 空间可读、可迁移。
- 旧 keySlot 空间保留加入能力。
- 迁移可 dry-run、可重跑、有报告。

### CI

- `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib` 通过。
- 前端 `vue-tsc` 通过。
- 不在本地跑构建，统一走 GitHub Actions。

## 14. 风险与回滚

| 风险 | 应对 |
|---|---|
| Markdown 块边界不稳定 | 固定解析规则 + 30+ round-trip 用例 |
| 文件监听跨平台差异 | 抽象 watcher 接口；移动端降级为扫描 |
| 事件风暴 | 防抖、合并、限流、转目录扫描 |
| 移动端文件访问 | 先私有目录 + 导入导出，SAF 后置 |
| 迁移有损 | dry-run、报告、备份、可重跑 |
| 旧空间不兼容 | 版本协商，`v1` 只读 |
| 用户丢恢复码 | 强制抄写、二次确认、文档警告 |
| 用户误点“新建空间” | 危险确认 + 二维码页面标明所属空间 |
| 请求限流 | 复用串行闸门、429 退避、打包 |
| 新同步内核出错 | 灰度开关，回退整文件同步 |

回滚：

- 新同步内核用 feature flag 控制，关闭后回到整文件同步。
- 迁移前备份数据库；迁移失败不删除原数据。
- 云端 `v2/` 与 `v1/` 隔离，出问题不影响旧空间。

## 15. 不做的事

- 不做云端明文 Markdown。
- 不做字符级 CRDT。
- 不做异地配对协议；异地由用户自行传递或使用恢复码。
- 不做移除设备功能。
- 本期不做老空间的全量密钥轮换。
- 不把 keySlot 作为新空间的默认。
- 不在本期做 iOS 文件系统深度集成。
