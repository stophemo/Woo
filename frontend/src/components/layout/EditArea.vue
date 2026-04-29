<template>
  <section class="edit-area">
    <!-- 编辑器主体 -->
    <div class="editor-body">
      <template v-if="store.currentDocument">
        <EditorContent :editor="editor" class="editor-content" />
      </template>
      <div v-else class="empty-editor">
        <p>请选择一个目录和文稿开始编辑</p>
      </div>
    </div>

    <!-- 状态栏 -->
    <div class="editor-statusbar">
      <div class="statusbar-left">
        <span>Markdown</span>
        <span v-if="currentBlock" class="current-block">{{ currentBlock }}</span>
      </div>
      <div class="statusbar-right">
        <span class="word-count">{{ wordCount }} 字</span>
        <span>{{ lineCount }} 行</span>
        <span class="shortcuts-hint">Ctrl+/ 快捷键</span>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, watch, onBeforeUnmount } from 'vue'
import { useEditor, EditorContent } from '@tiptap/vue-3'
import StarterKit from '@tiptap/starter-kit'
import Placeholder from '@tiptap/extension-placeholder'
import Underline from '@tiptap/extension-underline'
import TaskList from '@tiptap/extension-task-list'
import TaskItem from '@tiptap/extension-task-item'
import Highlight from '@tiptap/extension-highlight'
import Typography from '@tiptap/extension-typography'
import { Extension } from '@tiptap/vue-3'
import { useWorkspaceStore } from '../../stores/workspace'

const store = useWorkspaceStore()

// 防抖标记：防止 setContent 触发 onUpdate 反向写回
let isSettingContent = false

// 自定义快捷键扩展
const CustomKeymap = Extension.create({
  name: 'customKeymap',

  addKeyboardShortcuts() {
    return {
      // Shift+Alt+1~6 设置标题
      'Shift-Alt-1': () => this.editor.chain().focus().toggleHeading({ level: 1 }).run(),
      'Shift-Alt-2': () => this.editor.chain().focus().toggleHeading({ level: 2 }).run(),
      'Shift-Alt-3': () => this.editor.chain().focus().toggleHeading({ level: 3 }).run(),
      'Shift-Alt-4': () => this.editor.chain().focus().toggleHeading({ level: 4 }).run(),
      'Shift-Alt-5': () => this.editor.chain().focus().toggleHeading({ level: 5 }).run(),
      'Shift-Alt-6': () => this.editor.chain().focus().toggleHeading({ level: 6 }).run(),
      // Ctrl+0 正文
      'Mod-0': () => this.editor.chain().focus().setParagraph().run(),
      // Ctrl+Shift+H 高亮
      'Mod-Shift-h': () => this.editor.chain().focus().toggleHighlight().run(),
      // Ctrl+Shift+L 无序列表
      'Mod-Shift-l': () => this.editor.chain().focus().toggleBulletList().run(),
      // Ctrl+Shift+O 有序列表
      'Mod-Shift-o': () => this.editor.chain().focus().toggleOrderedList().run(),
      // Ctrl+Shift+T 任务列表
      'Mod-Shift-t': () => this.editor.chain().focus().toggleTaskList().run(),
      // Ctrl+Shift+Q 引用
      'Mod-Shift-q': () => this.editor.chain().focus().toggleBlockquote().run(),
      // Ctrl+Shift+C 代码块
      'Mod-Shift-c': () => this.editor.chain().focus().toggleCodeBlock().run(),
      // Ctrl+Shift+X 删除线（StarterKit 默认也有）
      'Mod-Shift-x': () => this.editor.chain().focus().toggleStrike().run(),
      // Ctrl+Enter 分割线
      'Mod-Enter': () => this.editor.chain().focus().setHorizontalRule().run(),
    }
  },
})

// 初始化编辑器
const editor = useEditor({
  content: '',
  extensions: [
    StarterKit.configure({
      heading: {
        levels: [1, 2, 3, 4, 5, 6],
      },
    }),
    Placeholder.configure({
      placeholder: '开始写作...',
    }),
    Underline,
    TaskList,
    TaskItem.configure({
      nested: true,
    }),
    Highlight.configure({
      multicolor: false,
    }),
    Typography,
    CustomKeymap,
  ],
  editorProps: {
    attributes: {
      class: 'wysiwyg-editor',
      spellcheck: 'false',
    },
  },
  onUpdate: ({ editor: editorInstance }) => {
    // 编辑器内容变化时同步到 store
    if (!isSettingContent && store.selectedDocumentId) {
      store.updateDocumentContent(store.selectedDocumentId, editorInstance.getHTML())
    }
  },
})

// 当前块类型（状态栏显示）
const currentBlock = computed(() => {
  if (!editor.value) return ''
  if (editor.value.isActive('heading', { level: 1 })) return 'H1'
  if (editor.value.isActive('heading', { level: 2 })) return 'H2'
  if (editor.value.isActive('heading', { level: 3 })) return 'H3'
  if (editor.value.isActive('heading', { level: 4 })) return 'H4'
  if (editor.value.isActive('heading', { level: 5 })) return 'H5'
  if (editor.value.isActive('heading', { level: 6 })) return 'H6'
  if (editor.value.isActive('bulletList')) return '无序列表'
  if (editor.value.isActive('orderedList')) return '有序列表'
  if (editor.value.isActive('taskList')) return '任务列表'
  if (editor.value.isActive('blockquote')) return '引用'
  if (editor.value.isActive('codeBlock')) return '代码块'
  return '正文'
})

// 字数统计
const wordCount = computed(() => {
  if (!editor.value) return 0
  const text = editor.value.getText()
  const chineseChars = (text.match(/[\u4e00-\u9fa5]/g) || []).length
  const englishWords = (text.match(/[a-zA-Z]+/g) || []).length
  return chineseChars + englishWords
})

// 行数统计
const lineCount = computed(() => {
  if (!editor.value) return 0
  const json = editor.value.getJSON()
  return json.content?.length || 0
})

// 监听选中文稿变化，加载对应内容到编辑器
watch(() => store.currentDocument, (newDoc) => {
  if (!editor.value) return

  if (newDoc) {
    isSettingContent = true
    editor.value.commands.setContent(newDoc.content)
    isSettingContent = false
  } else {
    isSettingContent = true
    editor.value.commands.setContent('')
    isSettingContent = false
  }
}, { immediate: true })

// 暴露方法供父组件调用
defineExpose({
  editor,
})

onBeforeUnmount(() => {
  editor.value?.destroy()
})
</script>

<style scoped>
.edit-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  background-color: var(--editor-bg);
  overflow: hidden;
  transition: var(--theme-transition);
}

/* 编辑器主体 */
.editor-body {
  flex: 1;
  overflow-y: auto;
}

.editor-content {
  height: 100%;
}

.empty-editor {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--text-muted);
  font-size: 14px;
}

/* 状态栏 */
.editor-statusbar {
  height: 28px;
  background-color: var(--bg-toolbar);
  border-top: 1px solid var(--border-primary);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 16px;
  font-size: 12px;
  color: var(--text-muted);
  flex-shrink: 0;
}

.statusbar-left,
.statusbar-right {
  display: flex;
  gap: 12px;
  align-items: center;
}

.current-block {
  color: var(--text-secondary);
  padding: 1px 6px;
  background-color: var(--editor-mark-bg);
  border-radius: 3px;
  font-size: 11px;
}

.shortcuts-hint {
  color: var(--text-disabled);
}

/* 滚动条 */
.editor-body::-webkit-scrollbar {
  width: 6px;
}

.editor-body::-webkit-scrollbar-track {
  background: var(--editor-bg);
}

.editor-body::-webkit-scrollbar-thumb {
  background: var(--scrollbar-thumb);
  border-radius: 3px;
}

.editor-body::-webkit-scrollbar-thumb:hover {
  background: var(--scrollbar-thumb-hover);
}
</style>

<style>
/* tiptap 编辑器全局样式（不能 scoped） */
.wysiwyg-editor {
  padding: 32px;
  max-width: 800px;
  margin: 0 auto;
  min-height: 100%;
  outline: none;
  color: var(--editor-text);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
  font-size: 15px;
  line-height: 1.8;
  caret-color: var(--editor-caret);
}

/* 占位符 */
.wysiwyg-editor p.is-editor-empty:first-child::before {
  content: attr(data-placeholder);
  float: left;
  color: var(--editor-placeholder);
  pointer-events: none;
  height: 0;
}

/* 标题 */
.wysiwyg-editor h1 {
  font-size: 28px;
  font-weight: 700;
  margin: 24px 0 16px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--editor-border);
  color: var(--editor-text-heading);
  line-height: 1.4;
}

.wysiwyg-editor h2 {
  font-size: 24px;
  font-weight: 600;
  margin: 20px 0 12px;
  padding-bottom: 6px;
  border-bottom: 1px solid var(--border-secondary);
  color: var(--editor-text-heading2);
  line-height: 1.4;
}

.wysiwyg-editor h3 {
  font-size: 20px;
  font-weight: 600;
  margin: 16px 0 10px;
  color: var(--editor-text);
  line-height: 1.4;
}

.wysiwyg-editor h4,
.wysiwyg-editor h5,
.wysiwyg-editor h6 {
  font-size: 16px;
  font-weight: 600;
  margin: 12px 0 8px;
  color: var(--editor-text-secondary);
  line-height: 1.4;
}

/* 段落 */
.wysiwyg-editor p {
  margin: 12px 0;
  color: var(--editor-text-secondary);
}

/* 列表 */
.wysiwyg-editor ul,
.wysiwyg-editor ol {
  margin: 12px 0;
  padding-left: 24px;
}

.wysiwyg-editor li {
  margin: 4px 0;
  color: var(--editor-text-secondary);
}

.wysiwyg-editor li p {
  margin: 0;
}

/* 任务列表 */
.wysiwyg-editor ul[data-type="taskList"] {
  list-style: none;
  padding-left: 0;
}

.wysiwyg-editor ul[data-type="taskList"] li {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.wysiwyg-editor ul[data-type="taskList"] li > label {
  flex-shrink: 0;
  margin-top: 4px;
}

.wysiwyg-editor ul[data-type="taskList"] li > label input[type="checkbox"] {
  accent-color: var(--accent);
  cursor: pointer;
  width: 16px;
  height: 16px;
}

.wysiwyg-editor ul[data-type="taskList"] li > div {
  flex: 1;
}

/* 行内代码 */
.wysiwyg-editor code {
  background-color: var(--editor-code-bg);
  padding: 2px 6px;
  border-radius: 3px;
  font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
  font-size: 14px;
  color: var(--editor-code-text);
}

/* 代码块 */
.wysiwyg-editor pre {
  background-color: var(--editor-pre-bg);
  padding: 16px;
  border-radius: 6px;
  overflow-x: auto;
  margin: 12px 0;
  border: 1px solid var(--editor-pre-border);
}

.wysiwyg-editor pre code {
  background: none;
  padding: 0;
  color: var(--editor-text);
  font-size: 14px;
  line-height: 1.6;
}

/* 引用 */
.wysiwyg-editor blockquote {
  border-left: 4px solid var(--editor-blockquote-border);
  padding-left: 16px;
  margin: 12px 0;
  color: var(--editor-blockquote-text);
  font-style: italic;
}

.wysiwyg-editor blockquote p {
  color: var(--editor-blockquote-text);
}

/* 分割线 */
.wysiwyg-editor hr {
  border: none;
  border-top: 1px solid var(--editor-border);
  margin: 20px 0;
}

/* 加粗 */
.wysiwyg-editor strong {
  color: var(--editor-text-heading);
  font-weight: 700;
}

/* 斜体 */
.wysiwyg-editor em {
  color: var(--editor-text-secondary);
  font-style: italic;
}

/* 删除线 */
.wysiwyg-editor s {
  color: var(--text-muted);
}

/* 下划线 */
.wysiwyg-editor u {
  text-decoration-color: var(--text-muted);
}

/* 高亮 */
.wysiwyg-editor mark {
  background-color: var(--editor-highlight-bg);
  color: var(--editor-highlight-text);
  padding: 1px 4px;
  border-radius: 2px;
}

/* 链接 */
.wysiwyg-editor a {
  color: var(--editor-link);
  text-decoration: none;
}

.wysiwyg-editor a:hover {
  text-decoration: underline;
}

/* 选中文本的样式 */
.wysiwyg-editor ::selection {
  background-color: var(--editor-selection);
}
</style>
