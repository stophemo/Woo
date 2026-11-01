<template>
  <div class="mobile-rich-editor">
    <EditorContent :editor="editor" class="mobile-editor-content" />

    <!-- 底部工具坞：拇指可达，不遮挡正文（内容区已留出相同高度）。 -->
    <div
      class="mobile-editor-dock"
      :class="{ 'is-keyboard-open': keyboardInset > 0 }"
      :style="{ bottom: `${keyboardInset}px` }"
    >
      <div v-if="isTableActive" class="mobile-table-toolbar" @mousedown.prevent @touchstart.stop>
        <button type="button" @click="runTableCommand('addRowAfter')">+ 行</button>
        <button type="button" @click="runTableCommand('deleteRow')">- 行</button>
        <button type="button" @click="runTableCommand('addColumnAfter')">+ 列</button>
        <button type="button" @click="runTableCommand('deleteColumn')">- 列</button>
        <button type="button" @click="runTableCommand('toggleHeaderRow')">表头</button>
        <button type="button" class="danger" @click="runTableCommand('deleteTable')">删除表格</button>
      </div>

      <div class="mobile-editor-toolbar" @mousedown.prevent @touchstart.stop>
        <button type="button" class="editor-tool text-tool" :class="{ active: isActive('bold') }" title="粗体" aria-label="粗体" @click="runEditorAction('bold')">B</button>
        <button type="button" class="editor-tool text-tool italic" :class="{ active: isActive('italic') }" title="斜体" aria-label="斜体" @click="runEditorAction('italic')">I</button>
        <button type="button" class="editor-tool text-tool underline" :class="{ active: isActive('underline') }" title="下划线" aria-label="下划线" @click="runEditorAction('underline')">U</button>
        <button type="button" class="editor-tool text-tool strike" :class="{ active: isActive('strikethrough') }" title="删除线" aria-label="删除线" @click="runEditorAction('strikethrough')">S</button>
        <span class="editor-tool-divider"></span>
        <button type="button" class="editor-tool heading-tool" :class="{ active: isActive('heading', { level: 1 }) }" title="一级标题" aria-label="一级标题" @click="runEditorAction('h1')">H1</button>
        <button type="button" class="editor-tool heading-tool" :class="{ active: isActive('heading', { level: 2 }) }" title="二级标题" aria-label="二级标题" @click="runEditorAction('h2')">H2</button>
        <button type="button" class="editor-tool" :class="{ active: isActive('bulletList') }" title="无序列表" aria-label="无序列表" @click="runEditorAction('ul')">
          <van-icon name="bars" />
        </button>
        <button type="button" class="editor-tool" :class="{ active: isActive('taskList') }" title="任务列表" aria-label="任务列表" @click="runEditorAction('task')">
          <van-icon name="passed" />
        </button>
        <button type="button" class="editor-tool" :class="{ active: isActive('blockquote') }" title="引用" aria-label="引用" @click="runEditorAction('quote')">
          <van-icon name="description" />
        </button>
        <span class="editor-tool-divider"></span>
        <button type="button" class="editor-tool" title="插入图片" aria-label="插入图片" @click="openImagePanel">
          <van-icon name="photo-o" />
        </button>
        <button type="button" class="editor-tool" title="插入表格" aria-label="插入表格" @click="openTablePanel">
          <van-icon name="apps-o" />
        </button>
        <button type="button" class="editor-tool" title="更多格式" aria-label="更多格式" @click="openMorePanel">
          <van-icon name="ellipsis" />
        </button>
      </div>
    </div>

    <van-popup v-model:show="showImagePanel" position="bottom" round :style="{ minHeight: '238px' }">
      <div class="insert-panel">
        <div class="insert-panel-header">
          <h3>插入图片</h3>
          <button type="button" class="panel-close" title="关闭" aria-label="关闭" @click="showImagePanel = false">
            <van-icon name="cross" />
          </button>
        </div>
        <van-field v-model="imageSource" label="地址" placeholder="图片 URL 或资源路径" clearable />
        <input ref="imageFileInput" class="hidden-file-input" type="file" accept="image/*" @change="handleImageFile" />
        <div class="insert-panel-actions">
          <van-button plain type="primary" size="small" icon="photograph" @click="openImageFilePicker">选择本地图片</van-button>
          <span class="panel-spacer"></span>
          <van-button size="small" @click="showImagePanel = false">取消</van-button>
          <van-button type="primary" size="small" :disabled="!imageSource.trim()" @click="confirmImage">插入</van-button>
        </div>
      </div>
    </van-popup>

    <van-popup v-model:show="showTablePanel" position="bottom" round :style="{ minHeight: '230px' }">
      <div class="insert-panel">
        <div class="insert-panel-header">
          <h3>插入表格</h3>
          <button type="button" class="panel-close" title="关闭" aria-label="关闭" @click="showTablePanel = false">
            <van-icon name="cross" />
          </button>
        </div>
        <div class="table-size-fields">
          <van-field v-model.number="tableRows" type="digit" label="行数" />
          <van-field v-model.number="tableCols" type="digit" label="列数" />
        </div>
        <div class="insert-panel-actions">
          <span class="panel-spacer"></span>
          <van-button size="small" @click="showTablePanel = false">取消</van-button>
          <van-button type="primary" size="small" @click="confirmTable">插入</van-button>
        </div>
      </div>
    </van-popup>

    <van-popup v-model:show="showMorePanel" position="bottom" round :style="{ maxHeight: '72vh' }">
      <div class="more-panel">
        <div class="insert-panel-header">
          <h3>编辑格式</h3>
          <button type="button" class="panel-close" title="关闭" aria-label="关闭" @click="showMorePanel = false">
            <van-icon name="cross" />
          </button>
        </div>
        <div class="format-grid">
          <button type="button" @click="runEditorAction('undo')">撤销</button>
          <button type="button" @click="runEditorAction('redo')">重做</button>
          <button type="button" @click="toggleHeading(1)">一级标题</button>
          <button type="button" @click="toggleHeading(2)">二级标题</button>
          <button type="button" @click="toggleHeading(3)">三级标题</button>
          <button type="button" @click="setParagraph">正文</button>
          <button type="button" @click="runEditorAction('ul')">无序列表</button>
          <button type="button" @click="runEditorAction('ol')">有序列表</button>
          <button type="button" @click="runEditorAction('task')">任务列表</button>
          <button type="button" @click="runEditorAction('quote')">引用</button>
          <button type="button" @click="runEditorAction('code')">行内代码</button>
          <button type="button" @click="runEditorAction('codeblock')">代码块</button>
          <button type="button" @click="openLinkPanel">链接</button>
          <button type="button" @click="runEditorAction('hr')">分割线</button>
          <button type="button" class="wide" @click="runEditorAction('clear')">清除格式</button>
        </div>
      </div>
    </van-popup>

    <van-popup v-model:show="showLinkPanel" position="bottom" round :style="{ minHeight: '238px' }">
      <div class="insert-panel">
        <div class="insert-panel-header">
          <h3>插入链接</h3>
          <button type="button" class="panel-close" title="关闭" aria-label="关闭" @click="showLinkPanel = false">
            <van-icon name="cross" />
          </button>
        </div>
        <van-field v-model="linkText" label="文字" placeholder="选中文本时可留空" />
        <van-field v-model="linkUrl" label="地址" placeholder="https://example.com" clearable />
        <div class="insert-panel-actions">
          <span class="panel-spacer"></span>
          <van-button size="small" @click="showLinkPanel = false">取消</van-button>
          <van-button type="primary" size="small" :disabled="!linkUrl.trim()" @click="confirmLink">插入</van-button>
        </div>
      </div>
    </van-popup>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { EditorContent, useEditor } from '@tiptap/vue-3'
import StarterKit from '@tiptap/starter-kit'
import Placeholder from '@tiptap/extension-placeholder'
import Underline from '@tiptap/extension-underline'
import TaskList from '@tiptap/extension-task-list'
import TaskItem from '@tiptap/extension-task-item'
import { Table, TableRow, TableHeader, TableCell } from '@tiptap/extension-table'
import Highlight from '@tiptap/extension-highlight'
import Typography from '@tiptap/extension-typography'
import Link from '@tiptap/extension-link'
import { Node as TiptapNode, mergeAttributes } from '@tiptap/core'
import { showToast } from 'vant'
import { markdownToEditorHtml } from '../../src/services/markdown'
import { buildAssetUrl } from '../../src/services/assetLink'
import {
  EDITOR_SHORTCUT_LABELS,
  getEditorShortcut,
  matchesShortcut,
  type EditorShortcutAction,
} from '../../src/config/editorShortcuts'

interface Props {
  modelValue: string
}

const props = defineProps<Props>()
const emit = defineEmits<{
  'update:modelValue': [value: string]
  'shortcut-save': []
}>()

const WooImage = TiptapNode.create({
  name: 'image',
  inline: true,
  group: 'inline',
  draggable: true,
  selectable: true,
  atom: true,
  addAttributes() {
    return {
      src: { default: null },
      alt: { default: null },
      title: { default: null },
    }
  },
  parseHTML() {
    return [{ tag: 'img[src]' }]
  },
  renderHTML({ HTMLAttributes }) {
    return ['img', mergeAttributes(HTMLAttributes)]
  },
})

function normalizeContent(value: string): string {
  const source = value || ''
  if (!source.trim()) return ''
  if (/<(p|h[1-6]|table|ul|ol|blockquote|pre|div|br|hr|img|a|strong|em|code)\b[^>]*>/i.test(source)) {
    return source
  }
  return markdownToEditorHtml(source)
}

// 键盘遮挡高度。
// Android 应用启用 edge-to-edge 后窗口不会因输入法 resize，WebView 的 visualViewport
// 也看不到键盘，因此由原生 MainActivity 通过 `woo:ime-inset` 事件把遮挡高度（CSS px）
// 推过来；其它平台（窗口会 resize 的场景）继续用 visualViewport 兜底，两者取较大值。
const keyboardInset = ref(0)
let nativeImeInset = 0

function applyKeyboardInset() {
  const viewport = window.visualViewport
  const obscuredByViewport = viewport
    ? Math.max(0, window.innerHeight - viewport.height - viewport.offsetTop)
    : 0
  const observed = Math.max(nativeImeInset, obscuredByViewport > 80 ? obscuredByViewport : 0)
  keyboardInset.value = Math.round(observed)
}

function onNativeImeInset(event: Event) {
  const detail = (event as CustomEvent<number>).detail
  nativeImeInset = typeof detail === 'number' && Number.isFinite(detail) && detail > 0 ? detail : 0
  applyKeyboardInset()
}

function syncKeyboardInset() {
  applyKeyboardInset()
}

const editorStateVersion = ref(0)
const showImagePanel = ref(false)
const showTablePanel = ref(false)
const showMorePanel = ref(false)
const showLinkPanel = ref(false)
const imageSource = ref('')
const linkText = ref('')
const linkUrl = ref('')
const tableRows = ref(3)
const tableCols = ref(3)
const imageFileInput = ref<HTMLInputElement | null>(null)
const savedSelection = ref<{ from: number; to: number } | null>(null)

const editor = useEditor({
  content: normalizeContent(props.modelValue),
  extensions: [
    StarterKit.configure({ heading: { levels: [1, 2, 3, 4, 5, 6] }, link: false, underline: false }),
    Placeholder.configure({ placeholder: '开始写作…' }),
    Underline,
    TaskList,
    TaskItem.configure({ nested: true }),
    WooImage,
    Table.configure({ resizable: false, renderWrapper: true }),
    TableRow,
    TableHeader,
    TableCell,
    Highlight.configure({ multicolor: false }),
    Typography,
    Link.configure({ openOnClick: false, autolink: false }),
  ],
  editorProps: {
    attributes: {
      class: 'mobile-tiptap-editor',
      spellcheck: 'false',
      autocapitalize: 'sentences',
    },
    handlePaste: (view, event) => {
      const file = Array.from(event.clipboardData?.files || []).find(item => item.type.startsWith('image/'))
      if (!file) return false
      if (file.size > 20 * 1024 * 1024) {
        event.preventDefault()
        showToast('图片超过 20MB，暂不支持插入')
        return true
      }
      event.preventDefault()
      const reader = new FileReader()
      reader.onload = () => {
        if (typeof reader.result !== 'string') return
        const node = view.state.schema.nodes.image.create({ src: reader.result, alt: file.name })
        view.dispatch(view.state.tr.replaceSelectionWith(node).scrollIntoView())
      }
      reader.onerror = () => showToast('读取剪贴板图片失败')
      reader.readAsDataURL(file)
      return true
    },
  },
  onUpdate: ({ editor: ed }) => {
    emit('update:modelValue', ed.getHTML())
  },
  onSelectionUpdate: () => {
    editorStateVersion.value++
  },
})

watch(() => props.modelValue, (value) => {
  const ed = editor.value
  if (!ed) return
  const next = normalizeContent(value)
  if (ed.getHTML() === next) return
  ed.commands.setContent(next, { emitUpdate: false })
})

const isTableActive = computed(() => {
  void editorStateVersion.value
  return editor.value?.isActive('table') === true
})

function isActive(name: string, attrs?: Record<string, unknown>): boolean {
  void editorStateVersion.value
  return editor.value?.isActive(name, attrs) === true
}

function runEditorAction(action: EditorShortcutAction): void {
  const ed = editor.value
  if (!ed) return
  if (showMorePanel.value && !['save', 'image', 'table', 'link'].includes(action)) restoreSelection()
  switch (action) {
    case 'undo': ed.chain().focus().undo().run(); break
    case 'redo': ed.chain().focus().redo().run(); break
    case 'save': emit('shortcut-save'); break
    case 'bold': ed.chain().focus().toggleBold().run(); break
    case 'italic': ed.chain().focus().toggleItalic().run(); break
    case 'underline': ed.chain().focus().toggleUnderline().run(); break
    case 'strikethrough': ed.chain().focus().toggleStrike().run(); break
    case 'highlight': ed.chain().focus().toggleHighlight().run(); break
    case 'clear': ed.chain().focus().clearNodes().unsetAllMarks().run(); break
    case 'ul': ed.chain().focus().toggleBulletList().run(); break
    case 'ol': ed.chain().focus().toggleOrderedList().run(); break
    case 'task': ed.chain().focus().toggleTaskList().run(); break
    case 'quote': ed.chain().focus().toggleBlockquote().run(); break
    case 'code': ed.chain().focus().toggleCode().run(); break
    case 'codeblock': ed.chain().focus().toggleCodeBlock().run(); break
    case 'hr': ed.chain().focus().setHorizontalRule().run(); break
    case 'image': openImagePanel(); break
    case 'table': openTablePanel(); break
    case 'link': openLinkPanel(); break
    case 'h1': toggleHeading(1); break
    case 'h2': toggleHeading(2); break
    case 'h3': toggleHeading(3); break
    case 'h4': toggleHeading(4); break
    case 'h5': toggleHeading(5); break
    case 'h6': toggleHeading(6); break
    case 'paragraph': setParagraph(); break
  }
  if (!['image', 'table', 'link'].includes(action)) showMorePanel.value = false
}

function openMorePanel() {
  rememberSelection()
  showMorePanel.value = true
}

function toggleHeading(level: number) {
  if (showMorePanel.value) restoreSelection()
  editor.value?.chain().focus().toggleHeading({ level: level as 1 | 2 | 3 | 4 | 5 | 6 }).run()
  showMorePanel.value = false
}

function setParagraph() {
  if (showMorePanel.value) restoreSelection()
  editor.value?.chain().focus().setParagraph().run()
  showMorePanel.value = false
}

function rememberSelection() {
  const ed = editor.value
  if (!ed) return
  savedSelection.value = { from: ed.state.selection.from, to: ed.state.selection.to }
}

function restoreSelection() {
  const ed = editor.value
  const selection = savedSelection.value
  if (!ed || !selection) return
  try { ed.commands.setTextSelection(selection) } catch { /* selection may be a node selection */ }
}

function openImagePanel() {
  rememberSelection()
  imageSource.value = ''
  showImagePanel.value = true
  showMorePanel.value = false
}

function closeImagePanel() {
  showImagePanel.value = false
}

function insertImageNode(src: string, alt = '') {
  const ed = editor.value
  if (!ed || !src) return
  restoreSelection()
  ed.chain().focus().insertContent({ type: 'image', attrs: { src, alt } }).run()
  savedSelection.value = null
  closeImagePanel()
}

function confirmImage() {
  const source = imageSource.value.trim()
  if (!source) return
  const src = buildAssetUrl(source)
  const alt = src.split('/').pop()?.split('?')[0] || ''
  insertImageNode(src, alt)
}

function openImageFilePicker() {
  imageFileInput.value?.click()
}

function handleImageFile(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  if (!file.type.startsWith('image/')) {
    showToast('请选择图片文件')
    return
  }
  if (file.size > 20 * 1024 * 1024) {
    showToast('图片超过 20MB，暂不支持插入')
    return
  }
  const reader = new FileReader()
  reader.onload = () => {
    if (typeof reader.result === 'string') insertImageNode(reader.result, file.name)
  }
  reader.onerror = () => showToast('读取图片失败')
  reader.readAsDataURL(file)
}

function openTablePanel() {
  rememberSelection()
  tableRows.value = 3
  tableCols.value = 3
  showTablePanel.value = true
  showMorePanel.value = false
}

function confirmTable() {
  const ed = editor.value
  if (!ed) return
  const rows = Math.max(1, Math.min(20, Number(tableRows.value) || 3))
  const cols = Math.max(1, Math.min(12, Number(tableCols.value) || 3))
  restoreSelection()
  ed.chain().focus().insertTable({ rows, cols, withHeaderRow: true }).run()
  savedSelection.value = null
  showTablePanel.value = false
}

function runTableCommand(command: string) {
  const ed = editor.value
  if (!ed) return
  const chain = ed.chain().focus() as any
  if (typeof chain[command] !== 'function') return
  chain[command]().run()
}

function openLinkPanel() {
  rememberSelection()
  const ed = editor.value
  linkText.value = ed && !ed.state.selection.empty ? ed.state.doc.textBetween(ed.state.selection.from, ed.state.selection.to, ' ') : ''
  linkUrl.value = ''
  showMorePanel.value = false
  showLinkPanel.value = true
}

function confirmLink() {
  const ed = editor.value
  const url = linkUrl.value.trim()
  if (!ed || !url) return
  restoreSelection()
  const href = /^https?:\/\//i.test(url) || /^[\w.-]+\.[a-z]{2,}(\/|$)/i.test(url) ? ( /^https?:\/\//i.test(url) ? url : `https://${url}` ) : `#heading:${url}`
  const selected = !ed.state.selection.empty
  if (selected) {
    ed.chain().focus().setLink({ href }).run()
  } else {
    ed.chain().focus().insertContent({ type: 'text', text: linkText.value.trim() || '链接' }).setLink({ href }).run()
  }
  savedSelection.value = null
  showLinkPanel.value = false
}

const shortcutActions = Object.keys(EDITOR_SHORTCUT_LABELS) as EditorShortcutAction[]
function handleConfiguredShortcut(event: KeyboardEvent) {
  const ed = editor.value
  if (!ed || !ed.isFocused) return
  const action = shortcutActions.find(candidate => matchesShortcut(getEditorShortcut(candidate), event))
  if (!action) return
  event.preventDefault()
  event.stopImmediatePropagation()
  runEditorAction(action)
}

function attachViewportListeners() {
  window.addEventListener('woo:ime-inset', onNativeImeInset)
  const viewport = window.visualViewport
  viewport?.addEventListener('resize', syncKeyboardInset)
  viewport?.addEventListener('scroll', syncKeyboardInset)
  syncKeyboardInset()
}

function detachViewportListeners() {
  window.removeEventListener('woo:ime-inset', onNativeImeInset)
  const viewport = window.visualViewport
  viewport?.removeEventListener('resize', syncKeyboardInset)
  viewport?.removeEventListener('scroll', syncKeyboardInset)
}

onMounted(() => {
  window.addEventListener('keydown', handleConfiguredShortcut, true)
  attachViewportListeners()
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleConfiguredShortcut, true)
  detachViewportListeners()
})
</script>

<style scoped>
.mobile-rich-editor { position: relative; min-height: 100%; background: var(--w-canvas); }

/* ---------- 底部工具坞 ---------- */
.mobile-editor-dock {
  position: fixed;
  right: 0;
  bottom: 0;
  left: 0;
  z-index: 7;
  padding-bottom: env(safe-area-inset-bottom, 0px);
  border-top: 1px solid var(--w-line);
  background: color-mix(in srgb, var(--w-canvas) 94%, transparent);
  backdrop-filter: blur(20px);
}

/* 键盘弹出时系统手势区已被覆盖，不再保留底部安全区，避免工具坞悬空。 */
.mobile-editor-dock.is-keyboard-open { padding-bottom: 0; }

.mobile-editor-toolbar {
  display: flex;
  align-items: center;
  gap: 2px;
  overflow-x: auto;
  padding: 8px 12px;
  scrollbar-width: none;
  -webkit-overflow-scrolling: touch;
}

.mobile-editor-toolbar::-webkit-scrollbar { display: none; }

.editor-tool {
  display: inline-flex;
  flex: 0 0 34px;
  width: 34px;
  height: 34px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: 9px;
  background: transparent;
  color: var(--w-ink-2);
  font-size: 17px;
}

.editor-tool:active { background: var(--w-rise); }

/* 选中态按 Effie 的做法只用颜色表达，不加底色 */
.editor-tool.active { color: var(--w-accent); }

.editor-tool.text-tool {
  font-family: Georgia, "Times New Roman", serif;
  font-weight: 700;
}

.editor-tool.heading-tool {
  font-family: inherit;
  font-size: 11.5px;
  font-weight: 700;
  letter-spacing: 0.02em;
}

.editor-tool.italic { font-style: italic; }
.editor-tool.underline { text-decoration: underline; }
.editor-tool.strike { text-decoration: line-through; }
.editor-tool-divider { width: 1px; height: 18px; flex: 0 0 1px; margin: 0 5px; background: var(--w-line); }

.mobile-table-toolbar {
  display: flex;
  gap: 6px;
  overflow-x: auto;
  padding: 7px 12px;
  border-bottom: 1px solid var(--w-line);
  background: var(--w-paper);
  -webkit-overflow-scrolling: touch;
}

.mobile-table-toolbar button {
  flex: 0 0 auto;
  min-height: 28px;
  padding: 3px 10px;
  border: 1px solid var(--w-line-strong);
  border-radius: 7px;
  background: var(--w-canvas);
  color: var(--w-ink-2);
  font-size: 12px;
  white-space: nowrap;
}

.mobile-table-toolbar button.danger { color: var(--van-danger-color); }

/* ---------- 正文画布（Effie 风格：大行距、蓝色标题、零装饰） ---------- */
.mobile-editor-content {
  min-height: 100%;
  padding: 18px 22px calc(158px + env(safe-area-inset-bottom, 0px));
}

.mobile-editor-content :deep(.mobile-tiptap-editor) {
  min-height: calc(100vh - 260px);
  outline: none;
  color: var(--w-ink);
  font-size: 17px;
  line-height: 1.9;
  letter-spacing: 0.01em;
  overflow-wrap: break-word;
  caret-color: var(--w-accent);
}

.mobile-editor-content :deep(.mobile-tiptap-editor p) { margin: 0 0 1.05em; }

.mobile-editor-content :deep(.mobile-tiptap-editor p.is-editor-empty:first-child::before) {
  content: attr(data-placeholder);
  float: left;
  height: 0;
  color: var(--w-ink-3);
  pointer-events: none;
}

.mobile-editor-content :deep(.mobile-tiptap-editor h1),
.mobile-editor-content :deep(.mobile-tiptap-editor h2),
.mobile-editor-content :deep(.mobile-tiptap-editor h3),
.mobile-editor-content :deep(.mobile-tiptap-editor h4),
.mobile-editor-content :deep(.mobile-tiptap-editor h5),
.mobile-editor-content :deep(.mobile-tiptap-editor h6) {
  color: var(--w-accent);
  font-weight: 600;
  letter-spacing: -0.01em;
  line-height: 1.45;
}

/* 标题前的小号级别标记，呼应 Effie 的 H1/H2 提示 */
.mobile-editor-content :deep(.mobile-tiptap-editor h1)::before,
.mobile-editor-content :deep(.mobile-tiptap-editor h2)::before,
.mobile-editor-content :deep(.mobile-tiptap-editor h3)::before {
  margin-right: 9px;
  color: var(--w-ink-3);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.1em;
  vertical-align: 3px;
}

.mobile-editor-content :deep(.mobile-tiptap-editor h1)::before { content: "H1"; }
.mobile-editor-content :deep(.mobile-tiptap-editor h2)::before { content: "H2"; }
.mobile-editor-content :deep(.mobile-tiptap-editor h3)::before { content: "H3"; }

.mobile-editor-content :deep(.mobile-tiptap-editor h1) { margin: 0 0 0.55em; font-size: 23px; }
.mobile-editor-content :deep(.mobile-tiptap-editor h2) { margin: 1.5em 0 0.5em; font-size: 20px; }
.mobile-editor-content :deep(.mobile-tiptap-editor h3) { margin: 1.3em 0 0.45em; font-size: 18px; }
.mobile-editor-content :deep(.mobile-tiptap-editor h4) { margin: 1.2em 0 0.4em; font-size: 17px; }

.mobile-editor-content :deep(.mobile-tiptap-editor ul),
.mobile-editor-content :deep(.mobile-tiptap-editor ol) { margin: 0 0 1.05em; padding-left: 1.35em; }

.mobile-editor-content :deep(.mobile-tiptap-editor li) { margin: 2px 0; }
.mobile-editor-content :deep(.mobile-tiptap-editor li p) { margin: 0 0 0.35em; }

.mobile-editor-content :deep(.mobile-tiptap-editor ol > li::marker) {
  color: var(--w-accent);
  font-weight: 600;
}

.mobile-editor-content :deep(.mobile-tiptap-editor blockquote) {
  margin: 1.2em 0;
  padding: 2px 0 2px 14px;
  border-left: 2px solid var(--w-line-strong);
  color: var(--w-ink-2);
}

.mobile-editor-content :deep(.mobile-tiptap-editor blockquote p:last-child) { margin-bottom: 0; }

.mobile-editor-content :deep(.mobile-tiptap-editor code) {
  padding: 1px 5px;
  border-radius: 5px;
  background: var(--w-rise);
  color: var(--w-ink);
  font-family: "SFMono-Regular", Consolas, monospace;
  font-size: 0.86em;
}

.mobile-editor-content :deep(.mobile-tiptap-editor pre) {
  overflow-x: auto;
  margin: 1.2em 0;
  padding: 14px 16px;
  border-radius: 12px;
  background: var(--w-paper);
  font-size: 13px;
  line-height: 1.75;
}

.mobile-editor-content :deep(.mobile-tiptap-editor pre code) { padding: 0; background: transparent; }

.mobile-editor-content :deep(.mobile-tiptap-editor a) {
  color: var(--w-accent);
  text-decoration: underline;
  text-decoration-thickness: 1px;
  text-underline-offset: 2px;
}

.mobile-editor-content :deep(.mobile-tiptap-editor hr) {
  margin: 2em 0;
  border: 0;
  border-top: 1px solid var(--w-line);
}

.mobile-editor-content :deep(.mobile-tiptap-editor img) {
  display: block;
  max-width: 100%;
  height: auto;
  margin: 1.1em 0;
  border-radius: 10px;
}

.mobile-editor-content :deep(.mobile-tiptap-editor mark) {
  padding: 0 2px;
  border-radius: 3px;
  background: color-mix(in srgb, var(--w-accent) 18%, transparent);
  color: inherit;
}

/* 表格：只保留发丝线 */
.mobile-editor-content :deep(.mobile-tiptap-editor .tableWrapper) { overflow-x: auto; margin: 1.2em 0; }

.mobile-editor-content :deep(.mobile-tiptap-editor table) {
  width: 100%;
  min-width: 420px;
  border-collapse: collapse;
  font-size: 14px;
  line-height: 1.65;
}

.mobile-editor-content :deep(.mobile-tiptap-editor th),
.mobile-editor-content :deep(.mobile-tiptap-editor td) {
  min-width: 78px;
  padding: 9px 10px;
  border: 1px solid var(--w-line);
  text-align: left;
  vertical-align: top;
  overflow-wrap: anywhere;
}

.mobile-editor-content :deep(.mobile-tiptap-editor th) {
  color: var(--w-ink-2);
  font-weight: 600;
  background: transparent;
}

.mobile-editor-content :deep(.mobile-tiptap-editor .selectedCell) {
  background: color-mix(in srgb, var(--w-accent) 8%, transparent);
}

/* 任务列表 */
.mobile-editor-content :deep(.mobile-tiptap-editor ul[data-type='taskList']) {
  padding-left: 0;
  list-style: none;
}

.mobile-editor-content :deep(.mobile-tiptap-editor ul[data-type='taskList'] li) {
  display: flex;
  align-items: flex-start;
  gap: 9px;
}

.mobile-editor-content :deep(.mobile-tiptap-editor ul[data-type='taskList'] input) {
  width: 17px;
  height: 17px;
  margin-top: 6px;
  accent-color: var(--w-accent);
}

/* ---------- 底部弹出面板 ---------- */
.insert-panel,
.more-panel {
  padding: 16px 16px calc(20px + env(safe-area-inset-bottom, 0px));
  background: var(--w-paper);
}

.insert-panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
}

.insert-panel-header h3 {
  margin: 0;
  color: var(--w-ink);
  font-size: 16px;
  font-weight: 600;
}

.panel-close {
  display: inline-flex;
  width: 32px;
  height: 32px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--w-ink-2);
  font-size: 17px;
}

.hidden-file-input {
  position: absolute;
  width: 1px;
  height: 1px;
  opacity: 0;
  pointer-events: none;
}

.insert-panel-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 14px;
}

.panel-spacer { flex: 1; }

.table-size-fields {
  display: grid;
  gap: 10px;
  grid-template-columns: 1fr 1fr;
}

/* 格式网格：白色圆角卡片 + 字形在上、名称在下（Effie 的面板结构） */
.format-grid {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  padding-bottom: 4px;
}

.format-grid button {
  display: flex;
  min-height: 56px;
  align-items: center;
  justify-content: center;
  padding: 8px 4px;
  border: 0;
  border-radius: 12px;
  background: var(--w-canvas);
  color: var(--w-ink);
  font-size: 13px;
  box-shadow: var(--w-shadow);
}

.format-grid button:active {
  background: color-mix(in srgb, var(--w-accent) 8%, var(--w-canvas));
  color: var(--w-accent);
}

.format-grid button.wide { grid-column: 1 / -1; }
</style>
