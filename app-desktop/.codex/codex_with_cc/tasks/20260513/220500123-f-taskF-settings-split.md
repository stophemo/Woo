你是在 D:\coding\Woo\app-local 仓库中的 Claude worker。

目标（硬性，必须全部满足）：
1) 文件 -> 设置：只保留“静态资源链接”相关配置内容。
2) AI -> 模型配置（把当前“AI 设置”菜单文案改为“模型配置”）：打开的弹窗只保留 AI 配置区（DeepSeek API Key / Base URL / 模型说明 / 验证 / 保存 / 获取链接），不显示“静态资源链接”。
3) 两个入口打开同一个 SettingsDialog 组件时，必须根据入口上下文显示不同 section（例如 mode: 'file' | 'ai'）。
4) 不改坏现有保存逻辑：AI配置保存继续可用；静态资源链接保存继续可用。
5) 更新菜单文案：AI 菜单中的“AI 设置”改为“模型配置”。
6) 执行 npm run build 并通过。

执行约束：
- 仅做本任务相关最小改动，不回滚他人改动。
- 最终输出必须包含以下标题：
Process Log
Summary
Changed Files
Verification
Final Result
Risks Or Follow-ups
