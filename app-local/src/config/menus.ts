// 菜单项类型定义
export type MenuItemType = 'item' | 'divider' | 'submenu'

export interface MenuItem {
  type: MenuItemType
  label?: string
  action?: string
  isHtml?: boolean  // 支持自定义 HTML 内容
  children?: MenuItem[]  // 子菜单
}

// 文件菜单
export const fileMenuItems: MenuItem[] = [
  { type: 'item', label: '新建文稿', action: 'new-document' },
  { type: 'item', label: '新建文件夹', action: 'new-folder' },
  { type: 'item', label: '删除文稿', action: 'delete-document' },
  { type: 'divider' },
  { type: 'item', label: '版本历史', action: 'version-history' },
  { type: 'divider' },
  {
    type: 'submenu',
    label: '导入',
    children: [
      { type: 'item', label: 'Markdown', action: 'import-markdown' },
      { type: 'item', label: 'TXT', action: 'import-txt' }
    ]
  },
  {
    type: 'submenu',
    label: '导出',
    children: [
      { type: 'item', label: '图片', action: 'export-image' },
      { type: 'item', label: 'PDF', action: 'export-pdf' },
      { type: 'item', label: 'Markdown', action: 'export-markdown' },
      { type: 'item', label: 'TXT', action: 'export-txt' }
    ]
  },
  { type: 'divider' },
  { type: 'item', label: '设置', action: 'settings' },
  { type: 'item', label: '检查更新', action: 'check-update' },
  { type: 'item', label: '退出', action: 'exit' }
]

// 编辑菜单
export const editMenuItems: MenuItem[] = [
  { type: 'item', label: '撤销', action: 'undo' },
  { type: 'item', label: '反撤销', action: 'redo' },
  { type: 'divider' },
  { type: 'item', label: '查找', action: 'find' },
  { type: 'item', label: '查找并替换', action: 'find-replace' }
]

// AI 菜单
export const aiMenuItems: MenuItem[] = [
  { type: 'item', label: 'Open Chat', action: 'open-chat' }
]

// 标记菜单（Markdown 语法）
export const markMenuItems: MenuItem[] = [
  { type: 'item', label: '# 一级标题', action: 'h1' },
  { type: 'item', label: '## 二级标题', action: 'h2' },
  { type: 'item', label: '### 三级标题', action: 'h3' },
  { type: 'item', label: '#### 四级标题', action: 'h4' },
  { type: 'item', label: '##### 五级标题', action: 'h5' },
  { type: 'item', label: '###### 六级标题', action: 'h6' },
  { type: 'divider' },
  { type: 'item', label: '<strong>粗体</strong>', action: 'bold', isHtml: true },
  { type: 'item', label: '<em>斜体</em>', action: 'italic', isHtml: true },
  { type: 'item', label: '<s>删除线</s>', action: 'strikethrough', isHtml: true },
  { type: 'divider' },
  { type: 'item', label: '- 无序列表', action: 'ul' },
  { type: 'item', label: '1. 有序列表', action: 'ol' },
  { type: 'item', label: '- [ ] 任务列表', action: 'task' },
  { type: 'divider' },
  { type: 'item', label: '> 引用', action: 'quote' },
  { type: 'item', label: '<code> 行内代码', action: 'code' },
  { type: 'item', label: '``` 代码块', action: 'codeblock' },
  { type: 'divider' },
  { type: 'item', label: '[链接](url)', action: 'link' },
  { type: 'item', label: '![图片](url)', action: 'image' },
  { type: 'item', label: '| 表格 |', action: 'table' },
  { type: 'divider' },
  { type: 'item', label: '--- 分割线', action: 'hr' }
]

// 查看菜单
export const viewMenuItems: MenuItem[] = [
  { type: 'item', label: '隐藏左侧菜单栏', action: 'toggle-left' },
  { type: 'item', label: '隐藏文稿缩略图栏', action: 'toggle-thumbnail' },
  { type: 'item', label: '隐藏右侧AI栏', action: 'toggle-right' },
  { type: 'divider' },
  { type: 'item', label: '外观', action: 'appearance' },
  { type: 'item', label: '主题', action: 'theme' },
  { type: 'item', label: '语言', action: 'language' }
]

// 帮助菜单
export const helpMenuItems: MenuItem[] = [
  { type: 'item', label: '文档介绍', action: 'docs' },
  { type: 'divider' },
  { type: 'item', label: 'GitHub 链接', action: 'github' }
]
