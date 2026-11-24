# WebDAV 同步空间（多设备六位配对码）

Woo 是本地优先的编辑器：所有文稿先写入本机 SQLite。跨设备同步只使用 **WebDAV 同步空间**：

1. 每台设备第一次使用时配置同一个 WebDAV 账号（地址、用户名、应用密码）；
2. 第一台设备点「建立同步空间」，生成 `vaultId`、`deviceId` 和 32 字节 `vaultKey`；
3. 其他设备只用六位配对码加入。

服务端只保存 AES-256-GCM 密文和必要的同步元数据；WebDAV 供应商无法读取正文、标题或图片内容。
WebDAV 没有服务端推送，**不是实时同步**；同步在启动、本地修改、回到前台和每 15 分钟兜底时进行。

---

## 一、配置 WebDAV 账号

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

账号密码不会进入日志、明文备份或 Android 系统云备份（`android:allowBackup="false"`）。

## 二、建立与加入空间

### 第一台设备

1. 设置 → 跨设备，填写 WebDAV 账号并点「测试连接并保存」。
2. 点「建立同步空间」。本机现有文稿和目录会作为初始内容上传。

### 其他设备

- **已有设备显示码**：已有设备点「显示配对码」，新设备输入六位码并「查找并认领」。
- **新设备显示码**：新设备点「显示我的配对码」，已有设备输入六位码并「查找并核对」。

六位码只负责定位配对请求，**安全依赖两端人工核对的六位核对码和空间指纹**。
新设备只有在点击「核对一致，完成加入」后才会保存身份；已有设备也必须在核对后点击批准。

加入已有空间时不会自动用本机旧副本覆盖远端；首次拉取完成后，仅把远端不存在的本机独有文稿补传上去。

## 三、远端数据布局

```text
v1/<vaultId>/space.json
v1/<vaultId>/ops/<shard>/<opId>.json
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

## 四、同步内核

- **推送**：本地触发器登记脏实体 → outbox 分块/压缩/加密 → `PUT` 到 `ops/<shard>/<opId>.json`。
  `PUT` 使用 `If-None-Match: *`，重试时若远端已有相同内容视为成功，不同内容拒绝覆盖。
- **拉取**：`PROPFIND` 两层列举 `ops` 和 shard 目录，只 `GET` 本地未应用的操作；
  整条操作通过 AES-GCM 校验后应用，成功后写入 applied 表。
- **重试**：HTTPS only、Basic Auth、禁止重定向；429/5xx/网络错误按 1/2/4 秒退避重试。
- **Lamport**：每个实体按 `(lamport, deviceId)` 最后写入胜出；加入已有空间前先扫描远端最大
  lamport 作为水位，避免本地新操作被远端旧操作覆盖。
- **坏操作隔离**：单条 JSON 损坏、密文校验失败或解密失败时，按 `opId` 记录为已处理并告警，
  不让一个坏对象让整个空间永久同步失败。
- **删除**：回收站状态随正文同步；清空回收站后超过清理窗口（默认 7 天）物理删除并生成 tombstone。
- **请求频率**：目录创建结果在进程内缓存；shard 数量为 4；本地修改触发的自动同步最小间隔 30 秒，
  后台每 15 分钟兜底。坚果云等供应商仍可能限速，同步失败时本地写作不受影响。

## 五、配对协议

使用 X25519 + HKDF-SHA256 + AES-256-GCM 传递 `vaultKey`；六位码不用于派生加密密钥。

### Path A：已有设备显示码

1. 已有设备写 `offer`：发起方公钥、`pairingSecret`、`vaultId`、设备信息、TTL。
2. 新设备读 `offer`，生成临时 X25519 密钥，写 `claim`（新设备公钥、deviceId、名称、平台）。
3. 已有设备读 `claim`，两端显示六位核对码；用户核对后已有设备写 `approval`，
   其中 `vaultKey` 用会话密钥封装，AAD 绑定 `pairingId` 和 `claimedDeviceId`。
4. 新设备读 `approval`，校验批准公钥与 `offer` 一致、`vaultId` 一致、`claimedDeviceId` 等于本机，
   解密后显示核对码和空间指纹；**用户点击确认后**才保存身份。

### Path B：新设备显示码

1. 新设备写 `join`：新设备公钥、`joinSecret`、设备信息、TTL。
2. 已有设备读 `join`，生成临时密钥，写 `resolve`（批准方公钥、`vaultId`、设备信息），
   显示六位核对码；用户核对后写 `approval`。
3. 新设备**必须先看到 `resolve`** 才处理 `approval`；只有 approval、没有 resolve 时直接忽略。
   `approval.approverPublicKey` 必须与 `resolve` 完全一致。

### 安全不变量

1. 新设备读到 approval 后绝不自动保存身份；必须人工点击「核对一致，完成加入」。
2. 保存前校验 `pairingId`、`deviceId`、`vaultId`、批准公钥、封装密钥解密结果。
3. 同一六位码匹配到多份 offer/join 时返回「配对码冲突，请重新生成」，不按远端时间自动选择。
4. 有效期 = 本机首次观察时间 + 文件自身 TTL，不使用本地时钟直接比较远端 `createdAt`。
5. 无法解码或未知 `protocolVersion` 的配对文件不删除。
6. 成功后清理该 `pairingId` 的已知配对文件；取消时清理本机写入的文件；过期文件按观察窗口清理。
7. 临时私钥、`vaultKey` 和 `joinSecret` 不写入本机日志或明文配置。

## 六、设备与吊销

WebDAV 没有服务端设备列表，**不能远程吊销设备**。退出空间只清除本机身份，本机笔记仍保留。
如需阻止旧设备继续同步，请修改 WebDAV 应用密码；旧设备无法通过认证后自然停止同步。
不要相信「远程移除设备」之类的假入口。

## 七、迁移

- 旧版 `webdav-config.json`（地址/用户名/应用密码）会在升级后自动迁移为新的加密账号配置；
  成功后删除旧文件，避免清除本机数据后从旧配置回填。
- 如果云端还没有 `v1/<vaultId>`，迁移会建立新空间并上传本机数据；如果云端已有空间，
  不自动建立身份，提示用六位配对码加入。
- 旧 Worker / 局域网凭据保留在磁盘上但**不再启用**；界面会提示用 WebDAV 重新配置。
- 旧 WebDAV 的 `manifest.json` 数据不会被新空间读取；其他旧设备需要重新配对。

## 八、测试

- `cargo test services::webdav_space`：账号加密、身份、配对 Path A/B、approval-only 忽略、
  配对码冲突、vaultId 不一致拒绝、PUT 幂等、重试、重定向拒绝、PROPFIND 解析、
  双设备推送/拉取、坏操作跳过。
- `cargo test services::webdav_space::sync::local_webdav_smoke -- --ignored`：需要本机 WebDAV
  （`wsgidav`）在 `127.0.0.1:18080`，覆盖真实 MKCOL/PROPFIND/PUT/GET 和墓碑传播。
- 真实供应商（坚果云/Nextcloud/群晖）双设备验证需要用户自己的账号和两台设备，当前仓库测试不包含。

## 九、相关代码

- WebDAV 同步空间：`src-tauri/src/services/webdav_space/`
- 共享加密/分块与操作模型：`src-tauri/woo-sync-core/`
- 本地 outbox / applied / LWW：`src-tauri/src/services/vault_sync/store.rs`
- 桌面设置页：`src/components/settings/SyncSpaceSettings.vue`
- 移动端设置页：`src-mobile/views/SyncSpace.vue`
- 同步编排与状态：`src-tauri/src/services/sync_engine.rs`、`src/stores/sync.ts`
