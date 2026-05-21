任务名: editor-multicursor-zoom
工作目录: D:\coding\Woo\app-local
目标:
1) 在中央编辑区支持多光标/多选区编辑（可用 Alt+Click 或 Ctrl+Alt+方向键新增光标，至少支持一种常见方式并有文档说明）。
2) 支持中央编辑区全局放大/缩小，快捷键 Ctrl+'+' 与 Ctrl+'-'，并加 Ctrl+'0' 重置；仅作用于中央编辑区内容，不影响左右栏字体。
3) 在 UI 中给出当前缩放比例提示（可放在状态栏）。
约束:
- 仅修改与编辑区相关文件，避免改动 AI chat 与设置页。
- 代码需通过 npm run build。
输出:
- 修改文件清单
- 验证命令与结果
- 使用 CODEX_WITH_CC.md 规定的固定标题输出报告。
