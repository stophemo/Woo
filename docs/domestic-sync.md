# WebDAV 同步空间

Woo 是本地优先的编辑器：所有文稿先写入本机 SQLite。跨设备同步只使用 **WebDAV**：

1. 每台设备填写同一个 WebDAV 账号（服务器地址、账户、应用密码）；
2. 第一台设备保存后自动建立同步空间并开始上传；
3. 同一账号的其他设备保存后自动加入，直接开始同步。

WebDAV 没有服务端推送，**不是实时同步**；同步在启动、本地修改、回到前台和每 15 分钟兜底时进行。

## 安全边界

正文和图片在客户端用 AES-256-GCM 加密，云端只保存密文和同步元数据。
为了让新设备“填完账号就能加入”，`space.json` 里保存了一个用应用密码派生密钥包裹的 `vaultKey`（密钥槽）：

- 防网盘被拖库、防运维随手查看文件；
- **不防**知道你 WebDAV 应用密码的人，包括 WebDAV 供应商（Basic Auth 密码在服务端可见）。

需要更强隔离时，关闭密钥槽并使用六位码配对：密钥只经设备间 X25519 传递，不落云端。

---

## 配置 WebDAV 账号

支持任意 HTTPS WebDAV。供应商示例：

| 供应商 | 地址示例 | 说明 |
| --- | --- | --- |
| 坚果云 | `https://dav.jianguoyun.com/dav/` | 使用「应用密码」，不是登录密码 |
| Nextcloud | `https://cloud.example.com/remote.php/dav/files/用户名/` | 地址需包含用户目录 |
| 群晖 | `https://nas.example.com:5006/woo` | 需启用 WebDAV Server |
| 自定义 | `https://dav.example.com/dav/` | 必须使用 HTTPS |

应用密码与同步身份**分开加密保存**在本机：

- `webdav-account.enc.json`：地址、用户名、应用密码；
- `webdav-identity.enc.json`：`vaultId`、`deviceId`、`vaultKey`；
- 主密钥优先放在系统钥匙串（macOS / Windows / Linux），不可用时退化为应用私有目录下 0600 的随机密钥文件。

账号密码不会进入日志或明文配置；Android 系统云备份已关闭（`android:allowBackup="false"`）。

### 备份与恢复

- 备份应用数据目录不会得到明文账号或 vaultKey；`webdav-account.enc.json` 和 `webdav-identity.enc.json` 都是 AES-GCM 密文。
- 主密钥优先在系统钥匙串中；钥匙串不可用时才退化为应用私有 0600 文件。桌面备份如果同时包含该密钥文件，仍可能解密身份文件，请自行排除应用数据目录或钥匙串。
- 当前没有恢复码。至少保留一台已配对设备，并确认系统钥匙串和本机数据目录可恢复。

## 建立与加入空间

界面只保留 4 个字段：**服务商、服务器地址、账户、应用密码**。选择服务商会自动填入常见地址
（例如坚果云 `https://dav.jianguoyun.com/dav/`），地址仍可手动修改。

### 第一台设备

填写账号 → 保存并同步。云端没有空间时，应用自动建立并上传本机文稿和目录。

### 其他设备

填写同一个账号 → 保存并同步。云端已有唯一空间、且本机没有文稿时，应用自动加入并拉取。
不需要配对码，也不需要审批。

也可以从已加入的设备直接导入账号：

1. 已加入设备点「添加设备」，生成一串 `WOO-SYNC:1:...` 配置串；
2. 新设备点「从另一台设备导入」，粘贴后自动填好地址和账户；
3. 默认不含应用密码，新设备补一次密码即可；勾选「包含应用密码」则零输入，但配置串等于明文密码，只适合自己使用。

以下情况不会自动加入，会给出明确提示：

- 本机已有文稿/目录/版本：避免互相覆盖，先确认以哪边为准；
- 云端有多个同步空间：需要先确认加入哪一个；
- 旧空间没有密钥槽：在任一已加入空间的设备上打开一次「跨设备」设置自动补写，或用一次六位码加入；
- 应用密码与云端密钥槽不匹配。

### 六位码配对（旧空间兼容）

界面默认不再显示六位码入口，后端仍保留用于旧空间或严格模式：

- **已有笔记的一方**点「让新设备加入（显示配对码）」，新设备输入六位码；
- **反过来也可以**：新设备点「显示我的配对码」，已有笔记的一方输入六位码并批准。

六位码只负责定位配对请求，**安全依赖持有空间的一方人工核对六位核对码、空间指纹和设备名**。
配对码有效期 15 分钟；网络错误会自动重试，不会因为一次失败就让配对作废。
如果提示「没有找到配对请求」，先确认两台设备填写的 WebDAV 地址和应用密码完全一致。

加入已有空间时不会自动用本机旧副本覆盖远端；首次拉取完成后，仅把远端不存在的本机独有文稿补传上去。

### 加入方的数据基准

加入方在写 claim/join 前会检查本机数据：

- 没有身份、没有文稿、没有本地草稿：直接加入。
- 没有身份，但有文稿/目录/版本/草稿：提示加入后会以对方数据为准；确认后**只开始配对**，本机数据暂时保留。
- 已有同步身份：显示本机空间短 ID、最近同步时间、对方设备名/平台，选择：
  - 「以对方为准」：开始配对；对方批准成功并校验通过后才清空本机文稿与身份；
  - 「以本机为准」：取消本次加入，不清数据；请在本机点「显示配对码」，让对方输入本机配对码；
  - 「取消」：不做任何改变。
- 清空只影响本机，不删除 WebDAV 上的旧空间；笔记端会同时清空本地草稿、外部文件映射和表格草稿，但不会删除磁盘上的外部 Markdown 文件。
- 取消、过期、未批准或对方拒绝都不会清空本机数据。批准后清空与绑定在同一 SQLite 事务内完成；清空失败时任务保留，不保存新身份。

## 远端数据布局

```text
v1/<vaultId>/space.json
v1/<vaultId>/state.json
v1/<vaultId>/ops/<shard>/<opId>.json
v1/<vaultId>/devices/<deviceId>.json
v1/pairing/offer-<6位码>-<pairingId>.json
v1/pairing/claim-<pairingId>.json
v1/pairing/join-<6位码>-<pairingId>.json
v1/pairing/resolve-<pairingId>.json
v1/pairing/approval-<pairingId>.json
```

每个操作文件包含：

```json
{
  "protocolVersion": 1,
  "vaultId": "...",
  "deviceId": "...",
  "opId": "...",
  "entityId": "doc:...",
  "kind": "upsert",
  "lamport": 12,
  "nonce": "...",
  "ciphertext": "..."
}
```

`kind` 为 `upsert` / `delete` / `reorder`。`nonce` 和 `ciphertext` 是 Base64URL；
AAD 绑定 `vaultId`、`opId`、`entityId`、`kind`、`lamport` 和 `deviceId`。

操作文件永不删除，其他设备通过本地 `vault_sync_applied_ops` 表按 `opId` 幂等去重。
内容块使用独立 AAD，文件名中的 `:` 编码为 `~`，以兼容严格的 WebDAV 服务器。
笔记正文分块会产生大量 `chunk-*` 操作，因此 shard 使用 `opId` 哈希取模 4，避免全部落在同一个
`ch/` 目录；旧版本留下的其他 shard 目录仍会被列举和读取。

## 同步内核

- **推送**：本地触发器登记脏实体 → outbox 分块/压缩/加密 → `PUT` 到 `ops/<shard>/<opId>.json`。
  `PUT` 使用 `If-None-Match: *`，重试时若远端已有相同内容视为成功，不同内容拒绝覆盖。
- **状态门闩**：`state.json` 记录 `seq` 和 `maxLamport`。任何设备推送后递增 `seq`；
  拉取前先读它（1 个 GET），没变化就跳过整轮列举。旧版本设备不会更新 `state.json`，
  因此每 10 分钟仍做一次全量扫描兼容。
- **拉取**：`PROPFIND` 两层列举 `ops` 和 shard 目录，只 `GET` 本地未应用的操作；
  整条操作通过 AES-GCM 校验后应用，成功后写入 applied 表。
- **重试与限流**：HTTPS only、Basic Auth、禁止重定向。同一账号的请求全局串行，
  相邻请求至少间隔 120ms；429 读取 `Retry-After` 并冷却，本轮直接结束交给下一轮，
  不再原地硬等；5xx/网络错误短退避后重试。
- **Lamport**：每个实体按 `(lamport, deviceId)` 最后写入胜出；加入已有空间时优先用
  `state.json` 的 `maxLamport` 作为水位（旧空间才回退逐条扫描），避免加入时打爆请求配额。
- **坏操作隔离**：单条 JSON 损坏、密文校验失败或解密失败时，按 `opId` 记录为已处理并告警，
  不让一个坏对象让整个空间永久同步失败。
- **删除**：回收站状态随正文同步；清空回收站后超过清理窗口（默认 7 天）物理删除并生成 tombstone。
- **请求频率**：目录创建结果在进程内缓存；shard 数量为 4；本地修改触发的自动同步最小间隔 60 秒，
  后台每 15 分钟兜底。无变化的一轮通常只有 1 个 `state.json` 读取。
- **设备心跳**：6 小时写一次加密心跳到 `devices/<deviceId>.json`；心跳失败只告警，不阻塞同步。

## 配对协议

使用 X25519 + HKDF-SHA256 + AES-256-GCM 传递 `vaultKey`；六位码不用于派生加密密钥。
同一个 WebDAV 账号的访问者能看到配对临时文件和六位定位码，因此批准前必须核对设备名、平台、六位核对码和空间指纹；人工批准是唯一防线。

如果无我笔记和无我待办共用同一个 WebDAV 账号，`v1/pairing/` 是账号级共享命名空间，
两边的六位码可能互相干扰；建议给两个应用使用不同的 WebDAV 账号或不同的子路径。

### Path A：已有设备显示码

1. 已有设备写 `offer`：发起方公钥、`pairingSecret`、`vaultId`、设备信息、TTL。
2. 新设备读 `offer`，生成临时 X25519 密钥，写 `claim`（新设备公钥、deviceId、名称、平台）。
3. 已有设备读 `claim`，两端显示六位核对码；用户核对后已有设备写 `approval`，
   其中 `vaultKey` 用会话密钥封装，AAD 绑定 `pairingId` 和 `claimedDeviceId`。
4. 新设备读 `approval`，校验批准公钥与 `offer` 一致、`vaultId` 一致、`claimedDeviceId` 等于本机，
   解密后自动保存身份并开始同步。人工确认只发生在已有设备批准前。

### Path B：新设备显示码

1. 新设备写 `join`：新设备公钥、`joinSecret`、设备信息、TTL。
2. 已有设备读 `join`，生成临时密钥，写 `resolve`（批准方公钥、`vaultId`、设备信息），
   显示六位核对码；用户核对后写 `approval`。
3. 新设备**必须先看到 `resolve`** 才处理 `approval`；只有 approval、没有 resolve 时直接忽略。
   `approval.approverPublicKey` 必须与 `resolve` 完全一致。

### 安全不变量

1. 人工确认只保留在持有空间的一方；已有设备必须显示加入方设备名/平台、六位核对码和空间指纹，确认后新设备自动完成。
2. 新设备保存前校验 `pairingId`、`deviceId`、`vaultId`、批准公钥、封装密钥解密结果。
3. 同一六位码匹配到多份 offer/join 时返回「配对码冲突，请重新生成」，不按远端时间自动选择。
4. 有效期 = 本机首次观察时间 + 文件自身 TTL（默认 15 分钟），不使用本地时钟直接比较远端 `createdAt`。
   新设备在绑定阶段遇到 429/5xx 时保留会话并自动重试，不会要求重新生成配对码。
5. 无法解码或未知 `protocolVersion` 的配对文件不删除。
6. 成功后清理该 `pairingId` 的已知配对文件；取消时清理本机写入的文件；过期文件按观察窗口清理。
7. 临时私钥、`vaultKey` 和 `joinSecret` 不写入本机日志或明文配置。

## 设备列表与吊销

每台设备的心跳文件是 `v1/<vaultId>/devices/<deviceId>.json`：

```json
{
  "format": "woo-notes-webdav-device",
  "protocolVersion": 1,
  "deviceId": "...",
  "nonce": "...",
  "ciphertext": "..."
}
```

明文只有 `name`、`platform`、`lastSeenAt`（毫秒）和 `appVersion`，用 vaultKey 加密；
AAD 为 `woo-notes-webdav-device-v1|<vaultId>|<deviceId>`。设备列表只读：按最近同步时间降序展示，
当前设备单独标记，不提供移除按钮。

旧版 0.9.x 设备不会写心跳；升级后需要成功同步一次才会出现在列表里。
WebDAV 没有服务端设备列表，**不能远程吊销设备**。退出空间只清除本机身份并尽力删除自己的心跳，
本机笔记仍保留。如需阻止旧设备继续同步，请修改 WebDAV 应用密码；旧设备无法通过认证后自然停止同步。
不要相信「远程移除设备」之类的假入口。供应商能看到设备数量和时间，但看不到设备名/平台。

## 迁移

- 旧版 `webdav-config.json`（地址/用户名/应用密码）会在升级后自动迁移为新的加密账号配置；
  成功后删除旧文件，避免清除本机数据后从旧配置回填。
- 如果云端还没有 `v1/<vaultId>`，迁移会建立新空间并上传本机数据；如果云端已有空间，
  不自动建立身份，提示用六位配对码加入。
- 旧 Worker / 局域网凭据保留在磁盘上但**不再启用**；界面会提示用 WebDAV 重新配置。
- 旧 WebDAV 的 `manifest.json` 数据不会被新空间读取；其他旧设备需要重新配对。

## 测试

- `cargo test services::webdav_space`：账号加密、身份、配对 Path A/B、offer 预览不写 claim、
  approval-only 忽略、配对码冲突、vaultId 不一致拒绝、PUT 幂等、重试、重定向拒绝、
  PROPFIND 解析、双设备推送/拉取、坏操作跳过、设备心跳加解密/排序/坏文件跳过、
  批准后清空本机文稿、绑定新空间并写入 Lamport 水位。
- `npm run test:pairing-ui`：检查桌面/移动端六位码样式使用前景色和明确背景，防止深色模式对比度回退。
- 加入方自动完成已由本地 WebDAV 测试覆盖；设备列表 UI 通过构建验证，项目尚未引入前端组件测试框架。
- `cargo test services::webdav_space::sync::local_webdav_smoke -- --ignored`：需要本机 WebDAV
  （`wsgidav`）在 `127.0.0.1:18080`，覆盖真实 MKCOL/PROPFIND/PUT/GET 和墓碑传播。
- `cargo test services::webdav_space::pairing::local_webdav_smoke -- --ignored`：覆盖真实 WebDAV
  上的 Path A / Path B 配对文件读写。
- `cargo test services::webdav_space::tests::auto_join_saves_identity_after_host_approval -- --ignored`：
  覆盖已有设备批准后加入方自动保存身份、写入 Lamport 水位并绑定本地库。
- 真实供应商（坚果云/Nextcloud/群晖）双设备验证需要用户自己的账号和两台设备，当前仓库测试不包含。

## 相关代码

- WebDAV 同步空间：`src-tauri/src/services/webdav_space/`
- 共享加密/分块与操作模型：`src-tauri/woo-sync-core/`
- 本地 outbox / applied / LWW：`src-tauri/src/services/vault_sync/store.rs`
- 桌面设置页：`src/components/settings/SyncSpaceSettings.vue`
- 移动端设置页：`src-mobile/views/SyncSpace.vue`
- 同步编排与状态：`src-tauri/src/services/sync_engine.rs`、`src/stores/sync.ts`
