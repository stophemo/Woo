# App-Local 发版说明（GitHub 自动发布）

本文说明下次如何发布 `app-local` 新版本。

## 一次标准发版（推荐）

1. 更新版本号（必须）
- 修改 [app-local/package.json](D:/coding/woo/app-local/package.json) 里的 `version`
- 同步修改 [app-local/package-lock.json](D:/coding/woo/app-local/package-lock.json) 顶层 `version` 和 `packages[""].version`

2. 提交代码
```bash
git add app-local/package.json app-local/package-lock.json
git commit -m "release: vX.Y.Z"
git push origin main
```

3. 打 tag 并推送（触发自动发版）
```bash
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

4. 等待 GitHub Actions 完成
- 工作流：`Release App Local`
- 成功后会自动创建/更新 Release 并上传安装包

## 会自动上传哪些文件

- `app-local/release/*.exe`
- `app-local/release/*.zip`

说明：`*.blockmap` 不再上传（当前不做增量自动更新）。

## 手动触发（备用）

如果不想先打 tag，可在 GitHub 页面手动运行：

1. `Actions` → `Release App Local` → `Run workflow`
2. `tag` 留空时默认使用 `v<app-local/package.json version>`
3. 可选勾选 `draft` / `prerelease`

## 常见问题

1. tag 已存在，想重发同版本
- 删除远端 tag 后重推，或使用新版本号重新发版（推荐新版本号）

2. 工作流成功但 Release 没有新文件
- 检查 `app-local/release/` 是否真的产出 `.exe` / `.zip`
- 检查是否误改了 `electron-builder` 的 `artifactName`
