任务名: right-chat-deepseek-v4pro
工作目录: D:\coding\Woo\app-local
目标:
1) 右侧 AI Chat 从 Gemini 接入 DeepSeek 问答。
2) 模型默认使用 deepseek-chat（如无 v4 pro 官方模型名则在模型名显示为 DeepSeek V4 Pro 映射到可用 API model，并在设置页注明）。
3) 设置页支持保存 DeepSeek API Key 与 Base URL（默认 https://api.deepseek.com）。
4) 保留流式输出与取消生成。
约束:
- 仅修改 AI chat 相关 store/service/types/settings/right sidebar 文件。
- 不要影响编辑器与文档存储。
- 代码需通过 npm run build。
输出:
- 修改文件清单
- 验证命令与结果
- 固定标题报告。
