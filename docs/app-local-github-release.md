# App-Local GitHub 自动发版

工作流文件：`.github/workflows/release-app-local.yml`

## 触发方式

1. 推送 tag 自动发版
- 支持 tag 格式：`v*` 或 `app-local-v*`
- 示例：
```bash
git tag v0.1.1
git push origin v0.1.1
```

2. 手动触发 workflow_dispatch
- 在 GitHub 页面进入 `Actions` → `Release App Local` → `Run workflow`
- 可选参数：
- `tag`：留空则默认 `v<app-local/package.json version>`
- `draft`：是否草稿发布
- `prerelease`：是否预发布

## 工作流会做什么

1. 在 `windows-latest` 执行构建
2. `npm ci` 安装依赖
3. `npm run electron:build` 生成安装包
4. 手动触发时，若 `tag` 不存在会自动创建并推送
5. 创建或更新 GitHub Release，并上传 `app-local/release/` 下产物

## 上传的文件类型

- `*.exe`
- `*.zip`
- `*.dmg`
- `*.AppImage`
- `*.deb`
- `latest*.yml`
- `*.blockmap`

## 权限要求

仓库 `Actions` 需要允许工作流写 `contents`（工作流已声明 `permissions: contents: write`）。
