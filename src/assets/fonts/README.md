# LXGW WenKai (subset)

`lxgw-wenkai-subset.woff2` / `lxgw-wenkai-medium-subset.woff2` 是「霞鹜文楷 Regular / Medium」的子集，只包含 GB2312 常用汉字、
ASCII、全角字符与常用标点（约 1.5MB），用作编辑器可选字体。

- 上游：https://github.com/lxgw/LxgwWenKai
- 许可：SIL Open Font License 1.1，见 `OFL-LXGWWenKai.txt`
- 生成：`pyftsubset LXGWWenKai-Regular.ttf --text-file=chars.txt --flavor=woff2 --layout-features='*' --no-hinting`


# LXGW Neo XiHei Screen (subset)

`lxgw-neo-xihei-screen-subset.woff2` 是「霞鹜新晰黑 Screen」的子集（GB2312 常用汉字、ASCII、常用标点，约 0.9MB），
用作应用界面字体；编辑器正文仍由 `--editor-font-family` 控制（默认文楷）。

- 上游：https://github.com/lxgw/LxgwNeoXiHei
- 许可：SIL Open Font License 1.1，见 `LICENSE-LXGWNeoXiHei.txt`
