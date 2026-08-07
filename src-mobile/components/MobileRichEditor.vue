<template>
  <div class="mobile-rich-editor">
    <div class="mobile-editor-toolbar" @mousedown.prevent @touchstart.stop>
      <button type="button" class="editor-tool" title="撤销" aria-label="撤销" @click="runEditorAction('undo')">
        <van-icon name="revoke" />
      </button>
      <button type="button" class="editor-tool" title="重做" aria-label="重做" @click="runEditorAction('redo')">
        <van-icon name="back-top" class="redo-icon" />
      </button>
      <span class="editor-tool-divider"></span>
      <button type="button" class="editor-tool text-tool" :class="{ active: isActive('bold') }" title="粗体" aria-label="粗体" @click="runEditorAction('bold')">B</button>
      <button type="button" class="editor-tool text-tool italic" :class="{ active: isActive('italic') }" title="斜体" aria-label="斜体" @click="runEditorAction('italic')">I</button>
      <button type="button" class="editor-tool text-tool underline" :class="{ active: isActive('underline') }" title="下划线" aria-label="下划线" @click="runEditorAction('underline')">U</button>
      <button type="button" class="editor-tool text-tool strike" :class="{ active: isActive('strikethrough') }" title="删除线" aria-label="删除线" @click="runEditorAction('strikethrough')">S</button>
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

    <div v-if="isTableActive" class="mobile-table-toolbar" @mousedown.prevent @touchstart.stop>
      <button type="button" @click="runTableCommand('addRowAfter')">+ 行</button>
      <button type="button" @click="runTableCommand('deleteRow')">- 行</button>
      <button type="button" @click="runTableCommand('addColumnAfter')">+ 列</button>
      <button type="button" @click="runTableCommand('deleteColumn')">- 列</button>
      <button type="button" @click="runTableCommand('toggleHeaderRow')">表头</button>
      <button type="button" class="danger" @click="runTableCommand('deleteTable')">删除表格</button>
    </div>

    <EditorContent :editor="editor" class="mobile-editor-content" />

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
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
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

onMounted(() => {
  window.addEventListener('keydown', handleConfiguredShortcut, true)
  void nextTick(() => editor.value?.commands.focus())
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleConfiguredShortcut, true)
})
</script>

<style scoped>
.mobile-rich-editor { min-height: calc(100vh - 110px); background: var(--van-background-2); }
.mobile-editor-toolbar { position: sticky; top: 0; z-index: 6; display: flex; align-items: center; gap: 4px; overflow-x: auto; padding: 7px 10px; border-bottom: 1px solid var(--van-border-color); background: color-mix(in srgb, var(--van-background-2) 94%, transparent); backdrop-filter: blur(16px); -webkit-overflow-scrolling: touch; }
.mobile-editor-toolbar::-webkit-scrollbar { display: none; }
.editor-tool { display: inline-flex; flex: 0 0 34px; width: 34px; height: 32px; align-items: center; justify-content: center; padding: 0; border: 0; border-radius: 6px; background: transparent; color: var(--van-text-color-2); font-size: 17px; }
.editor-tool:active, .editor-tool.active { background: color-mix(in srgb, var(--van-primary-color) 14%, transparent); color: var(--van-primary-color); }
.editor-tool.text-tool { font-weight: 700; font-family: Georgia, serif; }
.editor-tool.italic { font-style: italic; }
.editor-tool.underline { text-decoration: underline; }
.editor-tool.strike { text-decoration: line-through; }
.redo-icon { transform: scaleX(-1); }
.editor-tool-divider { width: 1px; height: 20px; flex: 0 0 1px; margin: 0 3px; background: var(--van-border-color); }
.mobile-table-toolbar { display: flex; gap: 6px; overflow-x: auto; padding: 7px 10px; border-bottom: 1px solid var(--van-border-color); background: var(--van-background); -webkit-overflow-scrolling: touch; }
.mobile-table-toolbar button { flex: 0 0 auto; min-height: 28px; padding: 3px 9px; border: 1px solid var(--van-border-color); border-radius: 5px; background: var(--van-background-2); color: var(--van-text-color-2); font-size: 12px; white-space: nowrap; }
.mobile-table-toolbar button.danger { color: var(--van-danger-color); }
.mobile-editor-content { min-height: calc(100vh - 164px); padding: 22px 18px 126px; }
.mobile-editor-content :deep(.mobile-tiptap-editor) { min-height: calc(100vh - 220px); outline: none; color: var(--van-text-color); font-size: 16px; line-height: 1.78; overflow-wrap: break-word; caret-color: var(--van-primary-color); }
.mobile-editor-content :deep(.mobile-tiptap-editor p) { margin: 0 0 15px; }
.mobile-editor-content :deep(.mobile-tiptap-editor h1), .mobile-editor-content :deep(.mobile-tiptap-editor h2), .mobile-editor-content :deep(.mobile-tiptap-editor h3), .mobile-editor-content :deep(.mobile-tiptap-editor h4), .mobile-editor-content :deep(.mobile-tiptap-editor h5), .mobile-editor-content :deep(.mobile-tiptap-editor h6) { color: var(--van-text-color); letter-spacing: 0; }
.mobile-editor-content :deep(.mobile-tiptap-editor h1) { margin: 0 0 20px; font-size: 28px; line-height: 1.35; }
.mobile-editor-content :deep(.mobile-tiptap-editor h2) { margin: 28px 0 10px; font-size: 21px; line-height: 1.4; }
.mobile-editor-content :deep(.mobile-tiptap-editor h3) { margin: 24px 0 8px; font-size: 18px; line-height: 1.45; }
.mobile-editor-content :deep(.mobile-tiptap-editor ul), .mobile-editor-content :deep(.mobile-tiptap-editor ol) { margin: 12px 0 18px; padding-left: 22px; }
.mobile-editor-content :deep(.mobile-tiptap-editor li) { margin: 5px 0; }
.mobile-editor-content :deep(.mobile-tiptap-editor blockquote) { margin: 20px 0; padding: 11px 14px; border-left: 3px solid var(--van-primary-color); background: color-mix(in srgb, var(--van-primary-color) 7%, var(--van-background)); color: var(--van-text-color-2); }
.mobile-editor-content :deep(.mobile-tiptap-editor pre) { overflow-x: auto; margin: 18px 0; padding: 14px; border: 1px solid var(--van-border-color); border-radius: 6px; background: var(--van-background); font-size: 13px; line-height: 1.6; }
.mobile-editor-content :deep(.mobile-tiptap-editor code) { padding: 1px 4px; border-radius: 3px; background: var(--van-background); font-family: "SFMono-Regular", Consolas, monospace; font-size: .88em; }
.mobile-editor-content :deep(.mobile-tiptap-editor pre code) { padding: 0; background: transparent; }
.mobile-editor-content :deep(.mobile-tiptap-editor a) { color: var(--van-primary-color); }
.mobile-editor-content :deep(.mobile-tiptap-editor hr) { margin: 28px 0; border: 0; border-top: 1px solid var(--van-border-color); }
.mobile-editor-content :deep(.mobile-tiptap-editor img) { display: block; max-width: 100%; height: auto; margin: 10px 0; border-radius: 6px; }
.mobile-editor-content :deep(.mobile-tiptap-editor .tableWrapper) { overflow-x: auto; margin: 18px 0; }
.mobile-editor-content :deep(.mobile-tiptap-editor table) { width: 100%; min-width: 420px; border-collapse: collapse; font-size: 13px; }
.mobile-editor-content :deep(.mobile-tiptap-editor th), .mobile-editor-content :deep(.mobile-tiptap-editor td) { min-width: 78px; padding: 8px; border: 1px solid var(--van-border-color); text-align: left; vertical-align: top; overflow-wrap: anywhere; }
.mobile-editor-content :deep(.mobile-tiptap-editor th) { background: var(--van-background); }
.mobile-editor-content :deep(.mobile-tiptap-editor ul[data-type="taskList"]) { padding-left: 0; list-style: none; }
.mobile-editor-content :deep(.mobile-tiptap-editor ul[data-type="taskList"] li) { display: flex; gap: 8px; align-items: flex-start; }
.mobile-editor-content :deep(.mobile-tiptap-editor ul[data-type="taskList"] input) { width: 16px; height: 16px; margin-top: 5px; accent-color: var(--van-primary-color); }
.insert-panel, .more-panel { padding: 16px 16px calc(20px + env(safe-area-inset-bottom, 0px)); }
.insert-panel-header { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 10px; }
.insert-panel-header h3 { margin: 0; color: var(--van-text-color); font-size: 17px; }
.panel-close { display: inline-flex; width: 32px; height: 32px; align-items: center; justify-content: center; padding: 0; border: 0; border-radius: 6px; background: transparent; color: var(--van-text-color-2); }
.hidden-file-input { position: absolute; width: 1px; height: 1px; opacity: 0; pointer-events: none; }
.insert-panel-actions { display: flex; align-items: center; gap: 8px; margin-top: 14px; }
.panel-spacer { flex: 1; }
.table-size-fields { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
.format-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; padding-bottom: 4px; }
.format-grid button { min-height: 40px; padding: 6px 4px; border: 1px solid var(--van-border-color); border-radius: 6px; background: var(--van-background-2); color: var(--van-text-color-2); font-size: 12px; }
.format-grid button:active { background: color-mix(in srgb, var(--van-primary-color) 10%, var(--van-background-2)); color: var(--van-primary-color); }
.format-grid button.wide { grid-column: 1 / -1; }
</style>
