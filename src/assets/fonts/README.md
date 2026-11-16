# LXGW WenKai (subset)

`lxgw-wenkai-subset.woff2` / `lxgw-wenkai-medium-subset.woff2` 是「霞鹜文楷 Regular / Medium」的子集，只包含 GB2312 常用汉字、
ASCII、全角字符与常用标点（约 1.5MB），用作编辑器可选字体。

- 上游：https://github.com/lxgw/LxgwWenKai
- 许可：SIL Open Font License 1.1，见 `OFL-LXGWWenKai.txt`
- 生成：`pyftsubset LXGWWenKai-Regular.ttf --text-file=chars.txt --flavor=woff2 --layout-features='*' --no-hinting`


# Noto Sans SC / Woo Sans (subset)

`noto-sans-sc-ui-subset.woff2` 是「Noto Sans SC」可变字重子集（GB2312 常用汉字、应用源码中的全部汉字、ASCII、
常用标点，约 1.8MB），用作应用界面字体。选择它是因为它与正文文楷形成干净的「界面黑体 / 正文文楷」分工，
不偏手写；同时保留 100–900 可变字重，界面上的 500/600/700 都是真实字重，不会出现合成粗体发虚。

- 上游：https://github.com/notofonts/noto-cjk
- 许可：SIL Open Font License 1.1，见 `LICENSE-NotoSansSC.txt`
- CSS family：`Woo Sans`（只在本应用内使用，避免和系统已安装的 Noto Sans SC 混用）
- 生成：先用 GB2312 字符集加 `src/`、`src-mobile/` 源码中的汉字生成 `chars.txt`，再执行
  `pyftsubset NotoSansSC-VF.otf --text-file=chars.txt --flavor=woff2 --layout-features='*' --no-hinting`

编辑器正文仍由 `--editor-font-family` 控制（默认文楷）。
