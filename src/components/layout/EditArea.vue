<template>
  <section class="edit-area">
    <div v-if="showSheet" class="sheet-wrap">
      <div class="sheet-tools">
        <button class="tool-btn" @click="applyStyle('bold')">B</button>
        <button class="tool-btn" @click="applyStyle('italic')">I</button>
        <button class="tool-btn" @click="setAlign('left')">左对齐</button>
        <button class="tool-btn" @click="setAlign('center')">居中</button>
        <button class="tool-btn" @click="setAlign('right')">右对齐</button>
        <button class="tool-btn" @click="toggleTopHeader">首行表头</button>
        <button class="tool-btn" @click="toggleLeftHeader">首列表头</button>
      </div>
      <div class="sheet-grid-wrap">
        <table class="sheet-grid">
          <thead>
            <tr>
              <th class="corner"></th>
              <th v-for="c in cols" :key="`h-${c}`">{{ colName(c) }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="r in rows" :key="`r-${r}`">
              <th>{{ r + 1 }}</th>
              <td
                v-for="c in cols"
                :key="`c-${r}-${c}`"
                :class="cellClass(r, c)"
                @click="selectCell(r, c)"
              >
                <input
                  :value="rawCell(r, c)"
                  @input="updateCell(r, c, ($event.target as HTMLInputElement).value)"
                />
                <div class="computed">{{ computedCell(r, c) }}</div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <div class="editor-body">
      <template v-if="store.currentDocument">
        <div v-if="store.currentDocument.isLocked" class="locked-placeholder">
          <IconLock class="locked-placeholder-icon" />
          <p class="locked-placeholder-text">文档已加锁，请在左侧验证密码后查看</p>
        </div>
        <div v-else class="editor-scale-wrap">
          <div v-if="isExternalFile" class="external-file-badge">
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
              <path d="M10 1h5v5M15 1L6 10M13 9v5a1 1 0 0 1-1 1H2a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
            <span>外部文件</span>
            <span class="external-file-path" :title="externalFilePath">{{ externalFilePath }}</span>
          </div>
          <div v-if="isTableActive" class="table-edit-toolbar" @mousedown.prevent>
            <button title="在下方添加一行" @click="runTableCommand('addRowAfter')">+ 行</button>
            <button title="删除当前行" @click="runTableCommand('deleteRow')">- 行</button>
            <button title="在右侧添加一列" @click="runTableCommand('addColumnAfter')">+ 列</button>
            <button title="删除当前列" @click="runTableCommand('deleteColumn')">- 列</button>
            <button title="切换表头行" @click="runTableCommand('toggleHeaderRow')">表头</button>
            <span class="table-tool-divider"></span>
            <button class="danger" title="删除表格" @click="runTableCommand('deleteTable')">删除表格</button>
          </div>
          <EditorContent :editor="editor" class="editor-content" :style="editorScaleStyle" />
        </div>
      </template>
      <div v-else-if="store.selectedFolderLocked" class="locked-placeholder">
        <IconLock class="locked-placeholder-icon" />
        <p class="locked-placeholder-text">当前目录已加锁，无法查看文稿</p>
      </div>
      <div v-else class="empty-editor">请选择一篇文稿</div>
    </div>

    <!-- 滚动定位按钮 — 放在 editor-body 外部，避免随内容滚动 -->

    <!-- 滚动定位按钮 — 放在 editor-body 外部，避免随内容滚动 -->
    <div class="scroll-nav" :class="{ visible: showScrollToTop || showScrollToBottom }">
      <button v-if="showScrollToTop" class="scroll-nav-btn" title="回到顶部" @click="scrollToTop">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
          <path d="M8 14V3M8 3L4 7M8 3l4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      </button>
      <button v-if="showScrollToBottom" class="scroll-nav-btn" title="滚动到底部" @click="scrollToBottom">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
          <path d="M8 2v11M8 13l-4-4M8 13l4-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      </button>
    </div>

    <div class="editor-statusbar" :class="{ collapsed: !isStatusBarOpen }">
      <div class="statusbar-left">
        <span>Markdown</span>
        <span v-if="currentBlock" class="current-block">{{ currentBlock }}</span>
      </div>
      <div class="statusbar-right">
        <span class="word-count">{{ wordCount }} 字</span>
        <span>{{ lineCount }} 行</span>
        <span>{{ zoomPercent }}%</span>
        <span class="app-version" title="当前应用版本">v{{ appVersion }}</span>
      </div>
    </div>

    <!-- 思维导图预览对话框 -->
    <MindmapDialog
      v-if="showMindmapDialog && mindmapDialogData"
      :data="mindmapDialogData"
      @close="showMindmapDialog = false"
    />

    <ContextMenu
      v-if="showEditorContextMenu"
      :position="editorContextMenuPosition"
      :items="editorContextMenuItems"
      @select="handleEditorContextMenuAction"
      @close="closeEditorContextMenu"
    />

    <Teleport to="body">
      <div v-if="showImageDialog" class="insert-dialog-overlay" @mousedown.self="closeImageDialog">
        <form class="insert-dialog" @submit.prevent="confirmImageUrl">
          <div class="insert-dialog-header">
            <h3>插入图片</h3>
            <button type="button" class="insert-dialog-close" title="关闭" @click="closeImageDialog">
              <IconClose />
            </button>
          </div>
          <label class="insert-dialog-field">
            <span>图片地址或资源路径</span>
            <input v-model="imageSourceInput" type="text" placeholder="https://example.com/image.png" autofocus />
          </label>
          <div class="insert-dialog-actions">
            <button type="button" class="secondary" @click="chooseLocalImage">选择本地图片</button>
            <span class="insert-dialog-spacer"></span>
            <button type="button" class="secondary" @click="closeImageDialog">取消</button>
            <button type="submit" class="primary" :disabled="!imageSourceInput.trim()">插入</button>
          </div>
        </form>
      </div>

      <div v-if="showTableDialog" class="insert-dialog-overlay" @mousedown.self="closeTableDialog">
        <form class="insert-dialog compact" @submit.prevent="confirmInsertTable">
          <div class="insert-dialog-header">
            <h3>插入表格</h3>
            <button type="button" class="insert-dialog-close" title="关闭" @click="closeTableDialog">
              <IconClose />
            </button>
          </div>
          <div class="table-size-fields">
            <label class="insert-dialog-field">
              <span>行数</span>
              <input v-model.number="tableRowsInput" type="number" min="1" max="20" />
            </label>
            <label class="insert-dialog-field">
              <span>列数</span>
              <input v-model.number="tableColsInput" type="number" min="1" max="12" />
            </label>
          </div>
          <div class="insert-dialog-actions">
            <span class="insert-dialog-spacer"></span>
            <button type="button" class="secondary" @click="closeTableDialog">取消</button>
            <button type="submit" class="primary">插入</button>
          </div>
        </form>
      </div>
    </Teleport>

  </section>
</template>

<script setup lang="ts">
import { computed, watch, onBeforeUnmount, onMounted, ref, nextTick } from 'vue'
import { useEditor, EditorContent } from '@tiptap/vue-3'
import StarterKit from '@tiptap/starter-kit'
import Placeholder from '@tiptap/extension-placeholder'
import Underline from '@tiptap/extension-underline'
import TaskList from '@tiptap/extension-task-list'
import TaskItem from '@tiptap/extension-task-item'
import { Table, TableRow, TableHeader, TableCell } from '@tiptap/extension-table'
import Highlight from '@tiptap/extension-highlight'
import Typography from '@tiptap/extension-typography'
import { Extension } from '@tiptap/vue-3'
import { Node as TiptapNode, mergeAttributes } from '@tiptap/core'
import BulletList from '@tiptap/extension-bullet-list'
import Link from '@tiptap/extension-link'
import { Plugin, PluginKey } from '@tiptap/pm/state'
import { DOMParser, DOMSerializer } from '@tiptap/pm/model'
import TurndownService from 'turndown'
import { tables as gfmTable, strikethrough as gfmStrikethrough } from 'turndown-plugin-gfm'
import { useWorkspaceStore } from '../../stores/workspace'
import { registerScrollHandler, useEditorNavigation } from '../../config/editorNavigation'
import type { TreeNode } from '../../types/mindmap'
import IconLock from '../icons/IconLock.vue'
import IconClose from '../icons/IconClose.vue'
import MindmapDialog from './MindmapDialog.vue'
import ContextMenu from '../ui/ContextMenu.vue'
import { log } from '../../services/logger'
import { markdownToEditorHtml } from '../../services/markdown'
import { buildAssetUrl } from '../../services/assetLink'
import { invoke } from '../../services/api'
import type { ContextMenuItem, ContextMenuPosition } from '../../types/folder'
import {
  DEFAULT_EDITOR_SHORTCUTS,
  EDITOR_SHORTCUT_LABELS,
  getEditorShortcut,
  matchesShortcut,
  shortcutForDisplay,
  type EditorShortcutAction,
} from '../../config/editorShortcuts'
import { isMac } from '../../config/shortcutUtils'

// 内置图片节点：避免把图片降级成不可编辑的原始 HTML，Markdown 转换器也能稳定识别 img。
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
/** HTML → Markdown 转换器（复制剪贴板，输出 GFM 风格 Markdown） */
const turndownService = new TurndownService({
  headingStyle: 'atx',
  bulletListMarker: '-',
  codeBlockStyle: 'fenced',
  emDelimiter: '*',
  strongDelimiter: '**',
  linkStyle: 'inlined',
  hr: '---',
})

// 加载 GFM 插件：表格 pipe-table 语法、~~删除线~~ 标记
turndownService.use(gfmTable)
turndownService.use(gfmStrikethrough)

// 任务列表（- [x] / - [ ]）
turndownService.addRule('taskList', {
  filter: (node: HTMLElement) =>
    node.nodeName === 'LI' &&
    (node.getAttribute('data-checked') !== null ||
     (!!node.parentElement && node.parentElement.getAttribute('data-type') === 'taskList')),
  replacement: (content: string, node: HTMLElement) => {
    const checked = node.getAttribute('data-checked') === 'true'
    return `- [${checked ? 'x' : ' '}] ${content.trim()}\n`
  },
})

const store = useWorkspaceStore()
const { headings } = useEditorNavigation()

// 检测当前编辑的是否为外部文件
const isExternalFile = computed(() => {
  if (!store.currentDocument) return false
  return !!store.externalFilePathMap?.[store.currentDocument.id]
})

const externalFilePath = computed(() => {
  if (!store.currentDocument) return ''
  return store.externalFilePathMap?.[store.currentDocument.id] || ''
})
const TRASH_FOLDER_ID = '__trash__'

interface Props { isStatusBarOpen?: boolean; appVersion?: string }
withDefaults(defineProps<Props>(), { isStatusBarOpen: true, appVersion: __APP_VERSION__ })

const emit = defineEmits<{
  (e: 'active-heading-change', headingIndex: number | null): void
}>()


const zoomPercent = ref(100)
const editorStateVersion = ref(0)
const showScrollToTop = ref(false)
const showScrollToBottom = ref(false)
const showSheet = ref(false)
const rows = 20
const cols = 10
const selected = ref<{ r: number; c: number } | null>(null)
const topHeader = ref(true)
const leftHeader = ref(true)
const cells = ref<string[][]>(Array.from({ length: rows }, () => Array.from({ length: cols }, () => '')))
const styles = ref<Record<string, { bold?: boolean; italic?: boolean; align?: 'left' | 'center' | 'right' }>>({})

const showEditorContextMenu = ref(false)
const editorContextMenuPosition = ref<ContextMenuPosition>({ x: 0, y: 0 })
const editorContextMenuItems = ref<ContextMenuItem[]>([])
const contextSelection = ref<{ from: number; to: number } | null>(null)
const showImageDialog = ref(false)
const imageSourceInput = ref('')
const showTableDialog = ref(false)
const tableRowsInput = ref(3)
const tableColsInput = ref(3)

const editorScaleStyle = computed(() => {
  const scale = zoomPercent.value / 100
  return {
    zoom: `${scale}`
  }
})

function cellKey(r: number, c: number) { return `${r}:${c}` }
function colName(c: number) { return String.fromCharCode(65 + c) }
function rawCell(r: number, c: number) { return cells.value[r][c] }
function toNumber(v: string): number {
  const n = Number(v)
  return Number.isFinite(n) ? n : 0
}
function parseRef(refText: string): { r: number; c: number } | null {
  const m = refText.match(/^([A-Z]+)(\d+)$/)
  if (!m) return null
  const colTxt = m[1]
  const row = Number(m[2]) - 1
  let col = 0
  for (let i = 0; i < colTxt.length; i++) col = col * 26 + (colTxt.charCodeAt(i) - 64)
  col -= 1
  if (row < 0 || row >= rows || col < 0 || col >= cols) return null
  return { r: row, c: col }
}
function evalFormula(input: string): string {
  const t = input.trim()
  if (!t.startsWith('=')) return t
  const expr = t.slice(1).trim().toUpperCase()
  const fn = expr.match(/^(SUM|AVG|MIN|MAX)\(([A-Z]\d+):([A-Z]\d+)\)$/)
  if (fn) {
    const start = parseRef(fn[2])
    const end = parseRef(fn[3])
    if (!start || !end) return '#REF!'
    const r1 = Math.min(start.r, end.r)
    const r2 = Math.max(start.r, end.r)
    const c1 = Math.min(start.c, end.c)
    const c2 = Math.max(start.c, end.c)
    const vals: number[] = []
    for (let r = r1; r <= r2; r++) for (let c = c1; c <= c2; c++) vals.push(toNumber(cells.value[r][c]))
    if (!vals.length) return '0'
    if (fn[1] === 'SUM') return String(vals.reduce((a, b) => a + b, 0))
    if (fn[1] === 'AVG') return String(vals.reduce((a, b) => a + b, 0) / vals.length)
    if (fn[1] === 'MIN') return String(Math.min(...vals))
    return String(Math.max(...vals))
  }
  const add = expr.match(/^([A-Z]\d+)\+([A-Z]\d+)$/)
  if (add) {
    const a = parseRef(add[1])
    const b = parseRef(add[2])
    if (!a || !b) return '#REF!'
    return String(toNumber(cells.value[a.r][a.c]) + toNumber(cells.value[b.r][b.c]))
  }
  return '#N/A'
}
function computedCell(r: number, c: number) { return evalFormula(cells.value[r][c]) }
function selectCell(r: number, c: number) { selected.value = { r, c } }
function updateCell(r: number, c: number, val: string) {
  cells.value[r][c] = val
  persistSheet()
}
function applyStyle(kind: 'bold' | 'italic') {
  if (!selected.value) return
  const key = cellKey(selected.value.r, selected.value.c)
  const cur = styles.value[key] || {}
  styles.value[key] = { ...cur, [kind]: !cur[kind] }
  persistSheet()
}
function setAlign(align: 'left' | 'center' | 'right') {
  if (!selected.value) return
  const key = cellKey(selected.value.r, selected.value.c)
  const cur = styles.value[key] || {}
  styles.value[key] = { ...cur, align }
  persistSheet()
}
function toggleTopHeader() { topHeader.value = !topHeader.value; persistSheet() }
function toggleLeftHeader() { leftHeader.value = !leftHeader.value; persistSheet() }
function cellClass(r: number, c: number) {
  const st = styles.value[cellKey(r, c)] || {}
  return {
    active: selected.value?.r === r && selected.value?.c === c,
    bold: !!st.bold,
    italic: !!st.italic,
    'align-left': (st.align || 'left') === 'left',
    'align-center': st.align === 'center',
    'align-right': st.align === 'right',
    'top-header': topHeader.value && r === 0,
    'left-header': leftHeader.value && c === 0
  }
}
function sheetKey() { return `sheet-${store.selectedDocumentId || 'default'}` }
function persistSheet() {
  localStorage.setItem(sheetKey(), JSON.stringify({ cells: cells.value, styles: styles.value, topHeader: topHeader.value, leftHeader: leftHeader.value }))
}
function loadSheet() {
  const raw = localStorage.getItem(sheetKey())
  if (!raw) return
  try {
    const parsed = JSON.parse(raw)
    if (parsed.cells) cells.value = parsed.cells
    if (parsed.styles) styles.value = parsed.styles
    topHeader.value = parsed.topHeader !== false
    leftHeader.value = parsed.leftHeader !== false
  } catch {}
}

function increaseZoom() { zoomPercent.value = Math.min(200, zoomPercent.value + 10) }
function decreaseZoom() { zoomPercent.value = Math.max(60, zoomPercent.value - 10) }
function resetZoom() { zoomPercent.value = 100 }

function handleGlobalKeydown(event: KeyboardEvent) {
  if (handleConfiguredShortcut(event)) return
  const isMod = isMac ? event.metaKey : event.ctrlKey
  if (!isMod) return
  if (event.key === '+' || event.key === '=' || event.code === 'NumpadAdd') { event.preventDefault(); increaseZoom(); return }
  if (event.key === '-' || event.code === 'NumpadSubtract') { event.preventDefault(); decreaseZoom(); return }
  if (event.code === 'Digit0' && event.shiftKey) { event.preventDefault(); resetZoom(); return }
}

let isSettingContent = false

// --- 版本自动保存：基于编辑会话 ---
// 触发条件：
//   1. 手动保存 (Ctrl+S) — 始终立即生效
//   2. 切换文档 — 离开当前文档前保存
//   3. 空闲超时 (IDLE_MS) — 停止输入一段时间后保存
//   4. 最长编辑保护 (MAX_EDIT_MS) — 连续编辑超时后强制保存
// 保护：
//   - 最小保存间隔 (COOLDOWN_MS) — 两次 auto 保存之间至少间隔这么久
//   - committing 标志 — 防止并发

const IDLE_MS = 30_000          // 空闲 30 秒后自动保存
const COOLDOWN_MS = 30_000      // 两次自动保存最小间隔
const MAX_EDIT_MS = 600_000     // 最长连续编辑时间（10 分钟后强制保存）

let idleTimer: number | null = null
let maxEditTimer: number | null = null
let baselineDocId: string | null = null
let dirtyAfterBaseline = false
let committing = false
let lastCommitTime = 0          // 上次自动保存的时间戳

function clearTimers() {
  if (idleTimer !== null) { clearTimeout(idleTimer); idleTimer = null }
  if (maxEditTimer !== null) { clearTimeout(maxEditTimer); maxEditTimer = null }
}
function resetBaseline() {
  baselineDocId = store.selectedDocumentId
  dirtyAfterBaseline = false
  clearTimers()
}

/** 开始计时：用户开始编辑后，启动空闲计时 & 最长编辑保护计时 */
function startTimers() {
  clearTimers()
  // 空闲计时：30 秒无输入后保存
  idleTimer = window.setTimeout(() => void autoCommit(), IDLE_MS)
  // 最长编辑保护：10 分钟无保存则强制保存
  maxEditTimer = window.setTimeout(() => void autoCommit(), MAX_EDIT_MS)
}

/** 执行自动保存，带冷却保护 */
async function autoCommit() {
  const docId = baselineDocId || store.selectedDocumentId
  if (!docId || !dirtyAfterBaseline || committing) return
  const now = Date.now()
  if (lastCommitTime > 0 && now - lastCommitTime < COOLDOWN_MS) {
    // 冷却期内，重新启动定时器等待下一次机会
    startTimers()
    return
  }
  committing = true
  try {
    await store.commitDocumentVersion(docId, 'auto')
    lastCommitTime = Date.now()
  } finally {
    committing = false
  }
  resetBaseline()
}

/** 手动保存/文档切换 — 不受冷却限制，始终立即执行 */
async function forceCommit(changeType: 'auto' | 'manual' = 'auto') {
  const docId = baselineDocId || store.selectedDocumentId
  if (!docId || !dirtyAfterBaseline || committing) return
  clearTimers(); committing = true
  try { await store.commitDocumentVersion(docId, changeType) } finally { committing = false }
  resetBaseline()
}

const CONFIGURED_SHORTCUT_ACTIONS = Object.keys(EDITOR_SHORTCUT_LABELS) as EditorShortcutAction[]

function executeEditorAction(action: EditorShortcutAction): void {
  const ed = editor.value
  if (!ed) return
  switch (action) {
    case 'undo': ed.chain().focus().undo().run(); break
    case 'redo': ed.chain().focus().redo().run(); break
    case 'save': void forceCommit('manual'); break
    case 'h1': ed.chain().focus().toggleHeading({ level: 1 }).run(); break
    case 'h2': ed.chain().focus().toggleHeading({ level: 2 }).run(); break
    case 'h3': ed.chain().focus().toggleHeading({ level: 3 }).run(); break
    case 'h4': ed.chain().focus().toggleHeading({ level: 4 }).run(); break
    case 'h5': ed.chain().focus().toggleHeading({ level: 5 }).run(); break
    case 'h6': ed.chain().focus().toggleHeading({ level: 6 }).run(); break
    case 'paragraph': ed.chain().focus().setParagraph().run(); break
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
    case 'link': toggleLink(); break
    case 'image': void insertImage(); break
    case 'table': insertTable(); break
    case 'hr': ed.chain().focus().setHorizontalRule().run(); break
  }
}

/** 在捕获阶段接管可配置快捷键，覆盖 Tiptap 和浏览器默认 keymap。 */
function handleConfiguredShortcut(event: KeyboardEvent): boolean {
  const ed = editor.value
  const target = event.target as Node | null
  if (!ed || !target || !ed.view.dom.contains(target)) return false

  const configured = CONFIGURED_SHORTCUT_ACTIONS.find(action => matchesShortcut(getEditorShortcut(action), event))
  if (configured) {
    event.preventDefault()
    event.stopImmediatePropagation()
    executeEditorAction(configured)
    return true
  }

  const staleDefault = CONFIGURED_SHORTCUT_ACTIONS.some(action =>
    getEditorShortcut(action) !== DEFAULT_EDITOR_SHORTCUTS[action] &&
    matchesShortcut(DEFAULT_EDITOR_SHORTCUTS[action], event)
  )
  if (staleDefault) {
    event.preventDefault()
    event.stopImmediatePropagation()
    return true
  }
  return false
}

// ═══════════════════════════════════════════════
// BulletList NodeView — 每个无序列表块右侧显示思维导图按钮
// ═══════════════════════════════════════════════

const showMindmapDialog = ref(false)
const mindmapDialogData = ref<TreeNode | null>(null)

function extractListItems(node: any, target: TreeNode[]) {
  if (node.type.name !== 'listItem') return
  let text = ''
  for (let i = 0; i < node.content.childCount; i++) {
    const child = node.content.child(i)
    if (child.type.name === 'paragraph') {
      text = child.textContent.trim()
    } else if (child.type.name === 'bulletList') {
      const subChildren: TreeNode[] = []
      for (let j = 0; j < child.childCount; j++) extractListItems(child.child(j), subChildren)
      if (text || subChildren.length > 0) { target.push({ text, children: subChildren }) }
      text = ''
    }
  }
  if (text) target.push({ text, children: [] })
}

function buildTreeAuto(listNode: any, rootText?: string): TreeNode {
  const items: TreeNode[] = []
  for (let i = 0; i < listNode.childCount; i++) extractListItems(listNode.child(i), items)
  if (items.length === 0) return { text: '', children: [] }
  if (rootText) {
    // 以指定文本为根，所有列表项为子节点
    return { text: rootText, children: items }
  }
  // 无根文本时：第一项为根节点，其余项作为子节点
  const root = items[0]
  for (let i = 1; i < items.length; i++) root.children.push(items[i])
  return root
}

const mmOpenCb = ref<((pos: number, editor: any) => void) | null>(null)
mmOpenCb.value = (pos: number, editor: any) => {
  // 从 editor state 取当前 node（而非闭包中可能过时的 node）
  const currentNode = editor.state.doc.nodeAt(pos)
  if (!currentNode || currentNode.type.name !== 'bulletList') return
  const listNode = currentNode

  // 查找前驱 heading
  let rootText: string | undefined
  const $pos = editor.state.doc.resolve(pos)
  const prev = $pos.nodeBefore
  if (prev?.type.name === 'heading') rootText = prev.textContent

  const tree = buildTreeAuto(listNode, rootText)
  if (!tree.text && tree.children.length === 0) return
  mindmapDialogData.value = tree
  showMindmapDialog.value = true
}

const MindmapBulletList = BulletList.extend({
  addNodeView() {
    return ({ editor, getPos }) => {
      const pos = getPos()
      const resolved = typeof pos === 'number' ? editor.state.doc.resolve(pos) : null

      // 嵌套列表（在 listItem 内）→ 纯 ul，不加按钮
      if (resolved?.parent?.type.name === 'listItem') {
        const ul = document.createElement('ul')
        return { dom: ul, contentDOM: ul }
      }

      // 顶级列表 → wrapper + 按钮
      const wrapper = document.createElement('div')
      wrapper.className = 'bulletlist-wrapper'

      const btn = document.createElement('span')
      btn.className = 'bullet-mm-btn'
      btn.title = '思维导图预览'
      btn.innerHTML = `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="6" cy="6" r="2.5"/><circle cx="18" cy="6" r="2.5"/><circle cx="12" cy="18" r="2.5"/><line x1="8.5" y1="6" x2="15.5" y2="6"/><line x1="8.5" y1="8" x2="11" y2="15.5"/><line x1="15.5" y1="8" x2="13" y2="15.5"/></svg>`

      btn.addEventListener('mousedown', (e: MouseEvent) => {
        e.preventDefault()
        e.stopPropagation()
        const currentPos = getPos()
        if (typeof currentPos === 'number') mmOpenCb.value?.(currentPos, editor)
      })

      const ul = document.createElement('ul')
      wrapper.appendChild(btn)
      wrapper.appendChild(ul)

      return { dom: wrapper, contentDOM: ul }
    }
  },
})

// ── Markdown 粘贴插件 ─────────────────────────────

/**
 * 启发式检测文本是否可能为 Markdown。
 * 快速路径（无额外开销），不会多次调用 marked.parse。
 */
function isMarkdownText(text: string): boolean {
  return /^#{1,6}\s/m.test(text) ||                // 标题
         /^[-*+]\s/m.test(text) ||                  // 无序列表
         /^\d+\.\s/m.test(text) ||                  // 有序列表
         /^\|.+\|/m.test(text) ||                   // 表格
         /^>\s/m.test(text) ||                      // 引用
         /^```/m.test(text) ||                      // 代码块
         /^---$|^\*\*\*$|^___$/m.test(text) ||      // 分割线
         /\*\*|__|~~|`{1,3}/.test(text) ||          // 行内格式
         /\[.+\]\(.+\)/.test(text)                  // 链接或图片
}

/**
 * 通过实际解析验证：用 marked 将文本转为 HTML，
 * 若结果包含结构性元素（标题、列表、表格、引用、代码块、
 * 水平线、加粗、斜体、代码），则认为文本是 Markdown。
 *
 * 比启发式 isMarkdownText 更准确（捕获所有 GFM 模式），
 * 作为 isMarkdownText 未命中时的兜底检查。
 */
function isMarkdownByParsing(text: string): boolean {
  try {
    const html = markdownToEditorHtml(text)
    return /<h[1-6]|<ul|<ol|<table|<blockquote|<pre|<hr|<strong|<em|<code/.test(html)
  } catch {
    return false
  }
}

function hasMarkdownStructure(text: string): boolean {
  return isMarkdownText(text) || isMarkdownByParsing(text)
}

const MarkdownPaste = Extension.create({
  name: 'markdownPaste',
  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey('markdownPaste'),
        props: {
          handlePaste: (view, event) => {
            // 标记为 Woo 自己复制的内容 → 走默认 HTML 解析（完美还原）
            if (event.clipboardData?.types.includes('text/x-woo')) {
              return false
            }

            // 无论 clipboard 是否包含 text/html，都优先尝试解析 text/plain 中的 Markdown
            const text = event.clipboardData?.getData('text/plain')
            if (text && hasMarkdownStructure(text)) {
              event.preventDefault()
              try {
                const html = markdownToEditorHtml(text)
                const div = document.createElement('div')
                div.innerHTML = html

                const { schema } = view.state
                const slice = DOMParser.fromSchema(schema).parseSlice(div, { preserveWhitespace: true })
                view.dispatch(view.state.tr.replaceSelection(slice))
                return true
              } catch (e) {
                log.editor.warn('Markdown paste 解析失败:', e)
                // 解析失败时回退到原生处理
              }
            }

            // 含 text/html 的富文本粘贴 → 由 ProseMirror 原生处理器保留完整格式
            return false
          },
        },
      }),
    ]
  },
})

// Markdown 链接自动转换插件：敲入 [text](url) 后转为可交互链接
const MarkdownLinkInput = Extension.create({
  name: 'markdownLinkInput',
  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey('markdownLinkInput'),
        appendTransaction: (_transactions, _oldState, newState) => {
          const { selection } = newState
          const { $from, empty } = selection
          // 仅在段落末尾处理
          if (!empty || $from.parent.type.name !== 'paragraph') return null

          const textBefore = $from.parent.textBetween(0, $from.parentOffset)
          const match = textBefore.match(/\[([^\]]+)\]\(([^)]+)\)$/)
          if (!match) return null

          const linkText = match[1]
          const linkUrl = match[2]
          if (!linkText || !linkUrl) return null

          // # 开头 → 锚点式目录链接；http(s): 或域名 → 外部链接；其余 → 文本匹配目录链接
          const finalHref = linkUrl.startsWith('#')
            ? linkUrl
            : (/^https?:\/\//i.test(linkUrl) || looksLikeUrl(linkUrl)
              ? (/^https?:\/\//i.test(linkUrl) ? linkUrl : `https://${linkUrl}`)
              : `#heading:${linkUrl}`)
          const linkMark = newState.schema.marks.link.create({ href: finalHref })
          const startPos = $from.pos - match[0].length
          const endPos = $from.pos

          const tr = newState.tr
          tr.replaceWith(startPos, endPos, newState.schema.text(linkText, [linkMark]))
          return tr
        },
      }),
    ]
  },
})

const editor = useEditor({
  content: '',
  extensions: [
    StarterKit.configure({ heading: { levels: [1, 2, 3, 4, 5, 6] }, bulletList: false }),
    Placeholder.configure({ placeholder: '开始写作...' }),
    Underline,
    TaskList,
    TaskItem.configure({ nested: true }),
    WooImage,
    Table.configure({ resizable: false, renderWrapper: true }),
    TableRow,
    TableHeader,
    TableCell,
    Highlight.configure({ multicolor: false }),
    Link.configure({ openOnClick: false, autolink: false }),
    Typography,
    MindmapBulletList,
    MarkdownPaste,
    MarkdownLinkInput,
  ],
  editorProps: {
    attributes: { class: 'wysiwyg-editor', spellcheck: 'false' },
    // 复制时：标记此内容来自 Woo，粘贴时据此区分是否走 HTML 完美还原
    handleDOMEvents: {
      // 复制时标记此内容来自 Woo，粘贴时据此区分是否走 HTML 完美还原
      copy: (_view, event) => {
        try { (event as any).clipboardData?.setData('text/x-woo', '1') } catch { /* ignore */ }
        return false // 让 ProseMirror 继续默认的复制处理
      },
      contextmenu: (_view, event) => {
        openEditorContextMenu(event as MouseEvent)
        return true
      },
    },
    clipboardTextSerializer: (slice, view) => {
      try {
        const serializer = DOMSerializer.fromSchema(view.state.schema)
        const fragment = serializer.serializeFragment(slice.content)
        const div = document.createElement('div')
        div.appendChild(fragment)
        const html = div.innerHTML
        let md = turndownService.turndown(html)
        // 保护代码块 → 折叠多余空行（去掉段落间空白行）→ 恢复代码块
        const codeBlocks: string[] = []
        md = md.replace(/(`{3,})[\s\S]*?\1/g, (m) => {
          codeBlocks.push(m)
          return `\x00CODE${codeBlocks.length - 1}\x00`
        })
        md = md.replace(/\n{2,}/g, '\n')
        md = md.replace(/\n[ \t]*\n/g, '\n') // 去掉缩进中的空白行（如嵌套列表内的空行）
        md = md.trim()
        md = md.replace(/\x00CODE(\d+)\x00/g, (_, i) => codeBlocks[+i])
        return md || slice.content.textBetween(0, slice.content.size, '\n')
      } catch (e) {
        log.editor.warn('clipboardTextSerializer 失败:', e)
        return slice.content.textBetween(0, slice.content.size, '\n')
      }
    },
  },
  onUpdate: ({ editor: editorInstance }) => {
    if (isSettingContent || store.isExternalStreaming || !store.selectedDocumentId) return
    if (store.selectedFolderId === TRASH_FOLDER_ID) return
    store.updateDocumentContent(store.selectedDocumentId, editorInstance.getHTML())
    if (baselineDocId !== store.selectedDocumentId) { resetBaseline(); return }
    dirtyAfterBaseline = true
    startTimers()
    updateActiveHeading()
    syncHeadings()
  },
  onSelectionUpdate: () => {
    editorStateVersion.value++
  },
})

const isTableActive = computed(() => {
  void editorStateVersion.value
  return editor.value?.isActive('table') === true
})

const currentBlock = computed(() => {
  if (!editor.value) return ''
  if (editor.value.isActive('heading', { level: 1 })) return 'H1'
  if (editor.value.isActive('heading', { level: 2 })) return 'H2'
  if (editor.value.isActive('heading', { level: 3 })) return 'H3'
  if (editor.value.isActive('heading', { level: 4 })) return 'H4'
  if (editor.value.isActive('heading', { level: 5 })) return 'H5'
  if (editor.value.isActive('heading', { level: 6 })) return 'H6'
  if (editor.value.isActive('bulletList')) return 'Bullet List'
  if (editor.value.isActive('orderedList')) return 'Ordered List'
  if (editor.value.isActive('taskList')) return 'Task List'
  if (editor.value.isActive('blockquote')) return 'Quote'
  if (editor.value.isActive('codeBlock')) return 'Code Block'
  return 'Paragraph'
})

const wordCount = computed(() => {
  if (!editor.value) return 0
  const text = editor.value.getText()
  const chineseChars = (text.match(/[一-龥]/g) || []).length
  const englishWords = (text.match(/[a-zA-Z]+/g) || []).length
  return chineseChars + englishWords
})
const lineCount = computed(() => editor.value?.getJSON().content?.length || 0)

watch(() => store.currentDocument, async (newDoc, oldDoc) => {
  if (!editor.value) return
  if (oldDoc && oldDoc.id !== newDoc?.id && dirtyAfterBaseline && baselineDocId === oldDoc.id) {
    await forceCommit('auto')
  }
  // 内容相同时跳过 setContent，防止同步刷新导致光标跳转
  if (newDoc && oldDoc?.id === newDoc.id && editor.value.getHTML() === newDoc.content) {
    isSettingContent = false
    return
  }
  isSettingContent = true
  if (newDoc && newDoc.isLocked) {
    editor.value.commands.setContent('')
  } else if (newDoc && newDoc.content) {
    let html = newDoc.content
    // 如果内容不含 HTML 标签，很可能是 Markdown 原始文本，需要转为 HTML 再渲染
    if (!/<(p|h[1-6]|table|ul|ol|blockquote|pre|div|span|br|hr|img|a|strong|em|code)\b[^>]*>/i.test(html)) {
      try {
        html = markdownToEditorHtml(html)
      } catch {
        // 转换失败则保持原文
      }
    }
    editor.value.commands.setContent(html)
  } else {
    editor.value.commands.setContent('')
  }
  isSettingContent = false
  // AI 流式编辑时，自动滚动到文档底部展现打字效果
  if (store.isExternalStreaming) {
    nextTick(() => {
      const editorEl = editor.value?.view.dom as HTMLElement | undefined
      if (editorEl) {
        const container = editorEl.closest('.editor-body') as HTMLElement | null
        if (container) container.scrollTop = container.scrollHeight
      }
    })
  }
  resetBaseline()
  loadSheet()
  // 文档切换后重新绑定 scroll 监听并计算当前高亮
  void nextTick(() => {
    // 在编辑器 DOM 上注册捕获阶段 mousedown 监听（直接处理链接点击，不依赖 ProseMirror API）
    const editorEl = editor.value?.view.dom
    if (editorEl) {
      // 防重复锁：mousedown + click 事件都可能触发导航，只处理一次
      let linkClickInProgress = false
      function handleEditorLink(e: MouseEvent) {
        if (linkClickInProgress) { e.preventDefault(); return }
        const el = (e.target as Node).nodeType === 3 ? (e.target as Node).parentElement : e.target as Element
        const linkEl = el?.closest?.('a')
        if (!linkEl) return
        const href = linkEl.getAttribute('href') || ''
        if (!href) return
        e.preventDefault()
        e.stopPropagation()
        linkClickInProgress = true
        setTimeout(() => { linkClickInProgress = false }, 200)
        if (href.startsWith('#heading:')) {
          scrollToHeadingByText(href.slice('#heading:'.length))
        } else if (href.startsWith('#') && href !== '#') {
          scrollToHeadingByAnchor(href.slice(1))
        } else if (/^https?:\/\//i.test(href) || looksLikeUrl(href)) {
          const finalUrl = /^https?:\/\//i.test(href) ? href : `https://${href}`
          window.electronAPI?.openExternalLink?.(finalUrl)
        } else if (href) {
          scrollToHeadingByText(href)
        }
      }
      // 捕获阶段同时拦截 mousedown 和 click，确保浏览器不会触发默认导航
      editorEl.addEventListener('mousedown', handleEditorLink, true)
      editorEl.addEventListener('click', handleEditorLink, true)
    }
    setupScrollListener()
    updateScrollVisibility()
    updateActiveHeading()
    syncHeadings()
  })
}, { immediate: true })

function scrollToHeading(headingIndex: number) {
  const ed = editor.value
  if (!ed) return
  const scrollContainer = ed.view.dom.closest('.editor-body') as HTMLElement | null
  if (!scrollContainer) return
  const doc = ed.state.doc
  let hIdx = 0
  let targetPos: number | null = null
  doc.descendants((node, pos) => {
    if (node.type.name === 'heading') {
      if (hIdx === headingIndex) {
        targetPos = pos
        return false
      }
      hIdx++
    }
  })
  if (targetPos === null) return
  const coords = ed.view.coordsAtPos(targetPos)
  const containerRect = scrollContainer.getBoundingClientRect()
  const offset = coords.top - containerRect.top
  scrollContainer.scrollTop += offset - 16
  ed.commands.focus()
}

/* ======== 编辑器滚动 → 大纲高亮联动 ======== */
let scrollRAF: number | null = null
let lastActiveHeading: number | null = null
let scrollElRef: HTMLElement | null = null

function setupScrollListener() {
  const ed = editor.value
  if (!ed) return
  const editorEl = ed.view.dom
  if (!editorEl || !editorEl.isConnected) return
  const el = editorEl.closest('.editor-body') as HTMLElement | null
  if (!el || el === scrollElRef) return
  if (scrollElRef) scrollElRef.removeEventListener('scroll', onEditorScroll)
  scrollElRef = el
  scrollElRef.addEventListener('scroll', onEditorScroll, { passive: true })
}

function updateActiveHeading() {
  scrollRAF = null
  const ed = editor.value
  if (!ed) {
    if (lastActiveHeading !== null) {
      lastActiveHeading = null
      emit('active-heading-change', null)
    }
    return
  }
  if (!scrollElRef) return
  const containerRect = scrollElRef.getBoundingClientRect()
  const referenceTop = containerRect.top + 16
  const doc = ed.state.doc

  let bestIdx: number | null = null
  let bestDist = Infinity
  let hIdx = 0
  doc.descendants((node, pos) => {
    if (node.type.name === 'heading') {
      const text = node.textContent.trim()
      if (text) {
        const coords = ed.view.coordsAtPos(pos)
        const dist = Math.abs(coords.top - referenceTop)
        if (dist < bestDist) {
          bestDist = dist
          bestIdx = hIdx
        }
      }
      hIdx++
    }
  })

  if (bestIdx !== lastActiveHeading) {
    lastActiveHeading = bestIdx
    emit('active-heading-change', bestIdx)
  }
}

function onEditorScroll() {
  if (scrollRAF !== null) return
  scrollRAF = requestAnimationFrame(() => {
    updateScrollVisibility()
    updateActiveHeading()
  })
}

function updateScrollVisibility() {
  const container = scrollElRef || (editor.value?.view.dom?.closest('.editor-body') as HTMLElement | null)
  if (!container) return
  const { scrollTop, scrollHeight, clientHeight } = container
  const hasOverflow = scrollHeight > clientHeight + 2
  showScrollToTop.value = hasOverflow && scrollTop > 120
  showScrollToBottom.value = hasOverflow && scrollHeight - scrollTop - clientHeight > 120
}

function scrollToTop() {
  scrollElRef?.scrollTo({ top: 0, behavior: 'smooth' })
}
function scrollToBottom() {
  if (!scrollElRef) return
  scrollElRef.scrollTo({ top: scrollElRef.scrollHeight, behavior: 'smooth' })
}

/** 从编辑器中提取标题列表 */
function produceHeadings(): { level: number; text: string; pos: number }[] {
  const ed = editor.value
  if (!ed) return []
  const result: { level: number; text: string; pos: number }[] = []
  ed.state.doc.descendants((node, pos) => {
    if (node.type.name === 'heading') {
      const text = node.textContent.trim()
      if (text) result.push({ level: node.attrs.level, text, pos })
    }
  })
  return result
}

/** 更新共享的 headings ref（供大纲面板使用） */
function syncHeadings() {
  headings.value = produceHeadings()
}

function getHeadings(): { level: number; text: string; pos: number }[] {
  return produceHeadings()
}

/** 按 ProseMirror doc position 滚动到指定标题（供 editorNavigation 模块调用） */
/** 根据标题文本滚动编辑器（用于目录链接导航） */
function scrollToHeadingByText(headingText: string) {
  const headings = produceHeadings()
  const t = headingText.trim().toLowerCase()
  const target = headings.find(h => h.text.trim().toLowerCase() === t)
  if (target) scrollToPos(target.pos)
}

/** 通过 slug 锚点名查找并滚动到对应标题 */
function scrollToHeadingByAnchor(anchor: string) {
  const headings = produceHeadings()
  const normalized = anchor.toLowerCase().replace(/-+/g, '-')
  for (const h of headings) {
    const slug = h.text.toLowerCase()
      .replace(/[^\w\u4e00-\u9fff]+/g, '-')
      .replace(/-+/g, '-')
      .replace(/^-|-$/g, '')
    if (slug === normalized) {
      scrollToPos(h.pos)
      return
    }
  }
}

function scrollToPos(pos: number) {
  const ed = editor.value
  if (!ed) return
  ed.commands.focus()
  ed.chain().setTextSelection(pos).scrollIntoView().run()
  try {
    const coords = ed.view.coordsAtPos(pos)
    if (!coords) return
    const container = (ed.view.dom as HTMLElement).closest('.editor-body') as HTMLElement | null
    if (!container) return
    const containerRect = container.getBoundingClientRect()
    const diff = coords.top - containerRect.top
    container.scrollTop = Math.round(container.scrollTop + diff - 24)
  } catch { /* scrollIntoView 已兜底 */ }
}

defineExpose({ editor, scrollToHeading, getHeadings })

/** 判断文本是否像 URL（含域名字段 .com/.org 等，或 IP 地址） */
function looksLikeUrl(text: string): boolean {
  return /^https?:\/\//i.test(text) ||
         /^[a-zA-Z0-9][-a-zA-Z0-9]*(\.[a-zA-Z][-a-zA-Z0-9]*)+/.test(text) ||
         /^www\.[a-zA-Z0-9]/.test(text) ||
         /^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}(:\d+)?$/.test(text)
}

/** 编辑器链接点击处理（通过 handleDOMEvents，在 ProseMirror 处理前拦截） */
onMounted(() => {
  window.addEventListener('woo-editor-command', menuActionHandler)
  registerScrollHandler(scrollToPos)
  window.addEventListener('keydown', handleGlobalKeydown, true)
  loadSheet()
  void nextTick(() => {
    // 在编辑器 DOM 上注册捕获阶段 mousedown 监听（直接处理链接点击，不依赖 ProseMirror API）
    const editorEl = editor.value?.view.dom
    if (editorEl) {
      // 防重复锁：mousedown + click 事件都可能触发导航，只处理一次
      let linkClickInProgress = false
      function handleEditorLink(e: MouseEvent) {
        if (linkClickInProgress) { e.preventDefault(); return }
        const el = (e.target as Node).nodeType === 3 ? (e.target as Node).parentElement : e.target as Element
        const linkEl = el?.closest?.('a')
        if (!linkEl) return
        const href = linkEl.getAttribute('href') || ''
        if (!href) return
        e.preventDefault()
        e.stopPropagation()
        linkClickInProgress = true
        setTimeout(() => { linkClickInProgress = false }, 200)
        if (href.startsWith('#heading:')) {
          scrollToHeadingByText(href.slice('#heading:'.length))
        } else if (href.startsWith('#') && href !== '#') {
          scrollToHeadingByAnchor(href.slice(1))
        } else if (/^https?:\/\//i.test(href) || looksLikeUrl(href)) {
          const finalUrl = /^https?:\/\//i.test(href) ? href : `https://${href}`
          window.electronAPI?.openExternalLink?.(finalUrl)
        } else if (href) {
          scrollToHeadingByText(href)
        }
      }
      // 捕获阶段同时拦截 mousedown 和 click，确保浏览器不会触发默认导航
      editorEl.addEventListener('mousedown', handleEditorLink, true)
      editorEl.addEventListener('click', handleEditorLink, true)
    }
    setupScrollListener()
    updateScrollVisibility()
    updateActiveHeading()
    syncHeadings()
  })
})

onBeforeUnmount(() => {
  window.removeEventListener('woo-editor-command', menuActionHandler)
  if (dirtyAfterBaseline && baselineDocId) void store.commitDocumentVersion(baselineDocId, 'auto')
  clearTimers()
  window.removeEventListener('keydown', handleGlobalKeydown, true)
  if (scrollRAF !== null) cancelAnimationFrame(scrollRAF)
  if (scrollElRef) {
    scrollElRef.removeEventListener('scroll', onEditorScroll)
    scrollElRef = null
  }
  editor.value?.destroy()
})

/** 切换/设置链接：选中文本时弹窗输入 URL */
function toggleLink() {
  const ed = editor.value
  if (!ed) return
  const { empty } = ed.state.selection
  // 已有链接则取消
  if (ed.isActive('link')) {
    ed.chain().focus().extendMarkRange('link').unsetLink().run()
    return
  }
  const url = prompt('输入链接：\n• http(s):// 开头或域名(google.com) = 外部链接（浏览器打开）\n• 其他文字 = 目录链接（滚动到对应标题）')
  if (!url) return
  // 外部链接直接使用 URL；域名自动补 https://；其余作为目录链接
  let href: string
  if (url.startsWith('#')) {
    // #xxx 格式 → 锚点式目录链接，原样存储
    href = url
  } else if (/^https?:\/\//i.test(url)) {
    href = url
  } else if (looksLikeUrl(url)) {
    href = `https://${url}`
  } else {
    href = `#heading:${url}`
  }
  if (empty) {
    ed.chain().focus().setLink({ href }).insertContent('链接').run()
  } else {
    ed.chain().focus().extendMarkRange('link').setLink({ href }).run()
  }
}

function insertTable() {
  if (!editor.value) return
  tableRowsInput.value = 3
  tableColsInput.value = 3
  showTableDialog.value = true
}

function closeTableDialog() {
  showTableDialog.value = false
}

function confirmInsertTable() {
  const ed = editor.value
  if (!ed) return
  const rows = Math.max(1, Math.min(20, Number(tableRowsInput.value) || 3))
  const cols = Math.max(1, Math.min(12, Number(tableColsInput.value) || 3))
  ed.chain().focus().insertTable({ rows, cols, withHeaderRow: true }).run()
  closeTableDialog()
}

function insertImage() {
  if (!editor.value) return
  imageSourceInput.value = ''
  showImageDialog.value = true
}

function closeImageDialog() {
  showImageDialog.value = false
}

function insertImageNode(src: string, alt = '') {
  const ed = editor.value
  if (!ed || !src) return
  ed.chain().focus().insertContent({ type: 'image', attrs: { src, alt } }).run()
  closeImageDialog()
}

function confirmImageUrl() {
  const source = imageSourceInput.value.trim()
  if (!source) return
  const src = buildAssetUrl(source)
  const alt = src.split('/').pop()?.split('?')[0] || ''
  insertImageNode(src, alt)
}

async function chooseLocalImage() {
  try {
    const selected = await invoke<{ filePath: string }>('dialog:open-image')
    if (!selected?.filePath) return
    const image = await invoke<{ data: string; mimeType: string }>('read:image-file', { filePath: selected.filePath })
    const src = `data:${image.mimeType};base64,${image.data}`
    const alt = selected.filePath.split(/[\\/]/).pop() || ''
    insertImageNode(src, alt)
  } catch (error: any) {
    const message = error?.message || '选择图片失败'
    log.editor.warn('选择图片失败:', error)
    window.dispatchEvent(new CustomEvent('sync:toast', { detail: { message, type: 'error' } }))
  }
}

function runTableCommand(command: string) {
  const ed = editor.value
  if (!ed) return
  const chain = ed.chain().focus() as any
  if (typeof chain[command] !== 'function') return
  chain[command]().run()
}

function restoreContextSelection() {
  const ed = editor.value
  if (!ed || !contextSelection.value) return
  ed.commands.focus()
  ed.commands.setTextSelection(contextSelection.value)
}

function contextShortcut(action: EditorShortcutAction): string {
  return shortcutForDisplay(getEditorShortcut(action))
}

function buildEditorContextMenuItems(): ContextMenuItem[] {
  const ed = editor.value
  if (!ed) return []
  const hasSelection = !ed.state.selection.empty
  const items: ContextMenuItem[] = [
    { label: '撤销', action: 'undo', shortcut: contextShortcut('undo') },
    { label: '重做', action: 'redo', shortcut: contextShortcut('redo') },
    { label: '', action: 'divider-history', divider: true },
    { label: '剪切', action: 'cut', disabled: !hasSelection },
    { label: '复制', action: 'copy', disabled: !hasSelection },
    { label: '粘贴', action: 'paste' },
    { label: '全选', action: 'select-all' },
    { label: '', action: 'divider-clipboard', divider: true },
    { label: '粗体', action: 'bold', shortcut: contextShortcut('bold') },
    { label: '斜体', action: 'italic', shortcut: contextShortcut('italic') },
    { label: '插入图片', action: 'image', shortcut: contextShortcut('image') },
    { label: '插入表格', action: 'table', shortcut: contextShortcut('table') },
  ]
  if (isTableActive.value) {
    items.push(
      { label: '', action: 'divider-table', divider: true },
      { label: '在下方添加行', action: 'table-add-row' },
      { label: '在右侧添加列', action: 'table-add-column' },
      { label: '删除当前行', action: 'table-delete-row' },
      { label: '删除当前列', action: 'table-delete-column' },
      { label: '切换表头行', action: 'table-header' },
      { label: '删除表格', action: 'table-delete' },
    )
  }
  return items
}

function openEditorContextMenu(event: MouseEvent) {
  const ed = editor.value
  if (!ed) return
  event.preventDefault()
  event.stopPropagation()
  contextSelection.value = { from: ed.state.selection.from, to: ed.state.selection.to }
  editorContextMenuPosition.value = { x: event.clientX, y: event.clientY }
  editorContextMenuItems.value = buildEditorContextMenuItems()
  showEditorContextMenu.value = true
}

function closeEditorContextMenu() {
  showEditorContextMenu.value = false
}

async function handleEditorContextMenuAction(action: string) {
  closeEditorContextMenu()
  restoreContextSelection()
  const ed = editor.value
  if (!ed) return
  if (action === 'copy' || action === 'cut') {
    const copied = document.execCommand(action)
    if (!copied && navigator.clipboard && contextSelection.value) {
      const text = ed.state.doc.textBetween(contextSelection.value.from, contextSelection.value.to, '\n')
      await navigator.clipboard.writeText(text)
    }
    if (action === 'cut' && !copied) ed.commands.deleteSelection()
    return
  }
  if (action === 'paste') {
    try {
      const text = await navigator.clipboard.readText()
      if (text) {
        ed.commands.insertContent(hasMarkdownStructure(text) ? markdownToEditorHtml(text) : text)
        return
      }
    } catch { /* fallback to WebView clipboard */ }
    document.execCommand('paste')
    return
  }
  if (action === 'select-all') { ed.commands.selectAll(); return }
  if (action.startsWith('table-')) {
    const commandMap: Record<string, string> = {
      'table-add-row': 'addRowAfter', 'table-add-column': 'addColumnAfter',
      'table-delete-row': 'deleteRow', 'table-delete-column': 'deleteColumn',
      'table-header': 'toggleHeaderRow', 'table-delete': 'deleteTable',
    }
    runTableCommand(commandMap[action])
    return
  }
  if (action in EDITOR_SHORTCUT_LABELS) executeEditorAction(action as EditorShortcutAction)
}

/** 监听来自 TopMenu 的编辑器命令。 */
const menuActionHandler = (e: Event) => {
  const detail = (e as CustomEvent).detail
  switch (detail?.command) {
    case 'link':
      toggleLink()
      break
    case 'image':
      void insertImage()
      break
    case 'table':
      insertTable()
      break
    case 'hr':
      editor.value?.chain().focus().setHorizontalRule().run()
      break
    case 'zoom-in':
      increaseZoom()
      break
    case 'zoom-out':
      decreaseZoom()
      break
    case 'zoom-reset':
      resetZoom()
      break
    default:
      if (typeof detail?.command === 'string' && detail.command in EDITOR_SHORTCUT_LABELS) {
        executeEditorAction(detail.command as EditorShortcutAction)
      }
      break
  }
}

</script>

<style scoped>
.edit-area { flex: 1; min-width: 0; display: flex; flex-direction: column; background-color: var(--editor-bg); overflow: hidden; position: relative; transition: var(--theme-transition); }
.tool-btn { border: 1px solid var(--border-primary); background: var(--bg-elevated); color: var(--text-primary); border-radius: 4px; height: 24px; padding: 0 8px; cursor: pointer; }
.sheet-wrap { border-bottom: 1px solid var(--border-primary); background: var(--bg-tertiary); }
.sheet-tools { display: flex; gap: 6px; padding: 8px; }
.sheet-grid-wrap { overflow: auto; max-height: 260px; }
.sheet-grid { border-collapse: collapse; width: max-content; min-width: 100%; }
.sheet-grid th, .sheet-grid td { border: 1px solid var(--border-primary); min-width: 110px; height: 36px; position: relative; }
.sheet-grid th { background: var(--bg-toolbar); font-weight: 600; text-align: center; }
.sheet-grid td input { width: 100%; height: 18px; border: none; background: transparent; color: var(--text-primary); padding: 2px 6px; }
.sheet-grid td .computed { font-size: 11px; color: var(--text-muted); padding: 0 6px 3px; }
.sheet-grid td.active { outline: 2px solid var(--accent); outline-offset: -2px; }
.sheet-grid td.bold input { font-weight: 700; }
.sheet-grid td.italic input { font-style: italic; }
.sheet-grid td.align-center input, .sheet-grid td.align-center .computed { text-align: center; }
.sheet-grid td.align-right input, .sheet-grid td.align-right .computed { text-align: right; }
.sheet-grid td.top-header { background: color-mix(in srgb, var(--bg-toolbar) 65%, transparent); }
.sheet-grid td.left-header { background: color-mix(in srgb, var(--bg-toolbar) 40%, transparent); }
.editor-body { flex: 1; min-width: 0; overflow-y: auto; overflow-x: hidden; display: flex; flex-direction: column; position: relative; }
/* 滚动定位按钮组 */
.scroll-nav { position: absolute; right: 10px; bottom: 36px; display: flex; flex-direction: column; gap: 5px; opacity: 0; pointer-events: none; transition: opacity 0.2s ease; z-index: 10; }
.scroll-nav.visible { opacity: 1; pointer-events: auto; }
.scroll-nav-btn { width: 30px; height: 30px; border-radius: 6px; border: 1px solid var(--border-secondary); background: color-mix(in srgb, var(--editor-bg) 88%, var(--bg-toolbar)); color: var(--text-secondary, #999); cursor: pointer; display: flex; align-items: center; justify-content: center; box-shadow: var(--shadow-card); transition: background 0.15s ease, color 0.15s ease, border-color 0.15s ease; }
.scroll-nav-btn:hover { background: color-mix(in srgb, var(--accent, #409eff) 20%, var(--bg-toolbar, #2a2a2a)); color: var(--accent, #409eff); }
.scroll-nav-btn:active { transform: scale(0.95); }
.editor-scale-wrap { flex: 1; min-width: 0; min-height: 0; }
.editor-content { width: 100%; min-width: 0; height: 100%; }
.table-edit-toolbar { position: sticky; top: 10px; z-index: 8; display: flex; align-items: center; gap: 4px; width: max-content; max-width: calc(100% - 24px); margin: 8px auto -42px; padding: 5px; border: 1px solid var(--border-primary); border-radius: 5px; background: color-mix(in srgb, var(--bg-elevated) 94%, transparent); box-shadow: var(--shadow-dropdown); }
.table-edit-toolbar button { border: 0; border-radius: 3px; background: transparent; color: var(--text-secondary); padding: 4px 7px; cursor: pointer; font-size: 12px; white-space: nowrap; }
.table-edit-toolbar button:hover { background: var(--bg-hover); color: var(--text-primary); }
.table-edit-toolbar button.danger:hover { color: var(--danger, #d04444); }
.table-tool-divider { width: 1px; height: 18px; margin: 0 3px; background: var(--border-secondary); }
.empty-editor { display: flex; align-items: center; justify-content: center; height: 100%; color: var(--text-muted); font-size: 14px; }

.locked-placeholder { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; gap: 12px; }
.locked-placeholder-icon { width: 32px; height: 32px; color: var(--accent, #409eff); opacity: 0.6; }
.editor-statusbar { height: 26px; background-color: var(--bg-toolbar); border-top: 1px solid var(--border-secondary); display: flex; align-items: center; justify-content: space-between; padding: 0 14px; font-size: 11px; color: var(--text-muted); flex-shrink: 0; overflow: hidden; transition: var(--theme-transition), height 0.25s ease, padding 0.25s ease, border-top-color 0.25s ease, opacity 0.2s ease, transform 0.25s ease; transform-origin: bottom; }
.editor-statusbar.collapsed { height: 0; padding-top: 0; padding-bottom: 0; border-top-color: transparent; transform: translateY(100%); opacity: 0; pointer-events: none; }
.statusbar-left, .statusbar-right { display: flex; gap: 14px; align-items: center; }
.app-version { padding-left: 14px; border-left: 1px solid var(--border-secondary); color: var(--text-secondary); font-variant-numeric: tabular-nums; }
.current-block { color: var(--text-secondary); padding: 1px 6px; background-color: var(--editor-mark-bg); border-radius: 3px; font-size: 11px; }
.editor-body::-webkit-scrollbar { width: 6px; }
.editor-body::-webkit-scrollbar-track { background: var(--editor-bg); }
.editor-body::-webkit-scrollbar-thumb { background: var(--scrollbar-thumb); border-radius: 3px; }
.editor-body::-webkit-scrollbar-thumb:hover { background: var(--scrollbar-thumb-hover); }
.zoom-control { cursor: pointer; }
.zoom-control:hover { color: var(--accent); }
.insert-dialog-overlay { position: fixed; inset: 0; z-index: 10000; display: flex; align-items: center; justify-content: center; padding: 20px; background: rgba(0, 0, 0, 0.38); }
.insert-dialog { width: min(440px, calc(100vw - 32px)); padding: 18px; border: 1px solid var(--border-primary); border-radius: 8px; background: var(--bg-surface); box-shadow: var(--shadow-dropdown); color: var(--text-primary); }
.insert-dialog.compact { width: min(340px, calc(100vw - 32px)); }
.insert-dialog-header { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 18px; }
.insert-dialog-header h3 { margin: 0; font-size: 16px; font-weight: 650; }
.insert-dialog-close { display: flex; align-items: center; justify-content: center; width: 28px; height: 28px; padding: 4px; border: 0; border-radius: 4px; background: transparent; color: var(--text-secondary); cursor: pointer; }
.insert-dialog-close:hover { background: var(--bg-hover); color: var(--text-primary); }
.insert-dialog-field { display: flex; flex-direction: column; gap: 7px; min-width: 0; color: var(--text-secondary); font-size: 13px; }
.insert-dialog-field input { box-sizing: border-box; width: 100%; height: 34px; padding: 6px 9px; border: 1px solid var(--border-primary); border-radius: 4px; outline: none; background: var(--bg-elevated); color: var(--text-primary); font: inherit; }
.insert-dialog-field input:focus { border-color: var(--accent); }
.table-size-fields { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.insert-dialog-actions { display: flex; align-items: center; gap: 8px; margin-top: 20px; }
.insert-dialog-spacer { flex: 1; }
.insert-dialog-actions button { min-height: 32px; padding: 6px 12px; border-radius: 4px; cursor: pointer; font: inherit; font-size: 13px; }
.insert-dialog-actions .secondary { border: 1px solid var(--border-primary); background: var(--bg-elevated); color: var(--text-primary); }
.insert-dialog-actions .secondary:hover { background: var(--bg-hover); }
.insert-dialog-actions .primary { border: 1px solid var(--accent); background: var(--accent); color: #fff; }
.insert-dialog-actions .primary:hover:not(:disabled) { background: var(--accent-hover); }
.insert-dialog-actions button:disabled { cursor: not-allowed; opacity: 0.45; }
</style>

<style>
/* AI 续写幽灵文字 */
.ai-complete-ghost {
  color: rgba(128, 128, 128, 0.45) !important;
  pointer-events: none;
  user-select: none;
  white-space: pre;
  font-style: italic;
}
</style>

<style>
.wysiwyg-editor {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  padding: 48px 36px 120px;
  max-width: 720px;
  margin: 0 auto;
  min-height: 100%;
  outline: none;
  color: var(--editor-text);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
  font-size: 15px;
  line-height: 1.88;
  caret-color: var(--editor-caret);
  text-rendering: optimizeLegibility;
}
.wysiwyg-editor p.is-editor-empty:first-child::before { content: attr(data-placeholder); float: left; color: var(--editor-placeholder); pointer-events: none; height: 0; }
.wysiwyg-editor h1 { font-size: 32px; font-weight: 720; margin: 8px 0 30px; color: var(--editor-text-heading); line-height: 1.28; }
.wysiwyg-editor h2 { font-size: 20px; font-weight: 680; margin: 34px 0 12px; color: var(--editor-text-heading2); line-height: 1.4; }
.wysiwyg-editor h3 { font-size: 17px; font-weight: 650; margin: 26px 0 10px; color: var(--editor-text); line-height: 1.45; }
.wysiwyg-editor h4, .wysiwyg-editor h5, .wysiwyg-editor h6 { font-size: 15px; font-weight: 650; margin: 22px 0 8px; color: var(--editor-text-secondary); line-height: 1.5; }
.wysiwyg-editor p { margin: 0 0 16px; color: var(--editor-text-secondary); }
.wysiwyg-editor ul, .wysiwyg-editor ol { margin: 14px 0 18px; padding-left: 24px; }
.wysiwyg-editor li { margin: 5px 0; color: var(--editor-text-secondary); }
.wysiwyg-editor li p { margin: 0; }
.wysiwyg-editor ul[data-type="taskList"] { list-style: none; padding-left: 0; }
.wysiwyg-editor ul[data-type="taskList"] li { display: flex; align-items: flex-start; gap: 8px; }
.wysiwyg-editor ul[data-type="taskList"] li > label { flex-shrink: 0; margin-top: 4px; }
.wysiwyg-editor ul[data-type="taskList"] li > label input[type="checkbox"] { accent-color: var(--accent); cursor: pointer; width: 16px; height: 16px; }
.wysiwyg-editor ul[data-type="taskList"] li > div { flex: 1; }
.wysiwyg-editor .tableWrapper { width: 100%; max-width: 100%; margin: 14px 0; overflow-x: auto; overscroll-behavior-x: contain; }
.wysiwyg-editor table { width: max-content; min-width: 100%; max-width: none; border-collapse: collapse; table-layout: auto; margin: 0; }
.wysiwyg-editor th, .wysiwyg-editor td { min-width: 80px; max-width: 320px; padding: 8px 10px; border: 1px solid var(--border-primary); text-align: left; vertical-align: top; overflow-wrap: anywhere; word-break: break-word; }
.wysiwyg-editor th { background: var(--bg-hover); color: var(--text-primary); font-weight: 600; }
.wysiwyg-editor td { color: var(--editor-text-secondary); }
.wysiwyg-editor th p, .wysiwyg-editor td p { margin: 0; }
.wysiwyg-editor code { background-color: var(--editor-code-bg); padding: 2px 6px; border-radius: 3px; font-family: 'Consolas', 'Monaco', 'Courier New', monospace; font-size: 14px; color: var(--editor-code-text); }
.wysiwyg-editor pre { background-color: var(--editor-pre-bg); padding: 16px; border-radius: 6px; overflow-x: auto; margin: 20px 0; border: 1px solid var(--editor-pre-border); }
.wysiwyg-editor pre code { background: none; padding: 0; color: var(--editor-text); font-size: 14px; line-height: 1.6; }
.wysiwyg-editor blockquote { border-left: 3px solid var(--editor-blockquote-border); padding: 14px 16px; margin: 24px 0; background: color-mix(in srgb, var(--editor-code-bg) 72%, transparent); color: var(--editor-blockquote-text); }
.wysiwyg-editor blockquote p { margin: 0; color: var(--editor-blockquote-text); }
.wysiwyg-editor hr { border: none; border-top: 1px solid var(--editor-border); margin: 32px 0; }
.wysiwyg-editor strong { color: inherit; font-weight: 650; }
.wysiwyg-editor em { color: var(--editor-text-secondary); font-style: italic; }
.wysiwyg-editor s { color: var(--text-muted); }
.wysiwyg-editor u { text-decoration-color: var(--text-muted); }
.wysiwyg-editor mark { background-color: var(--editor-highlight-bg); color: var(--editor-highlight-text); padding: 1px 4px; border-radius: 2px; }
.wysiwyg-editor img { display: inline-block; max-width: 100%; height: auto; margin: 8px 0; border-radius: 4px; cursor: pointer; }
.wysiwyg-editor img.ProseMirror-selectednode { outline: 2px solid var(--accent); outline-offset: 2px; }
.wysiwyg-editor a { color: var(--editor-link); text-decoration: none; cursor: pointer; }
.wysiwyg-editor a[href^="#heading:"] { border-bottom: 1px dashed var(--accent); padding-bottom: 1px; }
.wysiwyg-editor a:hover { text-decoration: underline; }
.wysiwyg-editor ::selection { background-color: var(--editor-selection); }

@media (max-width: 640px) {
  .wysiwyg-editor {
    padding: 20px 16px;
    max-width: 100%;
    font-size: 16px;
  }
  .wysiwyg-editor h1 { font-size: 24px; }
  .wysiwyg-editor h2 { font-size: 21px; }
  .wysiwyg-editor h3 { font-size: 18px; }
}

/* ════════════════════════════════════════════════
   BulletList NodeView — 思维导图按钮
   ════════════════════════════════════════════════ */
.bulletlist-wrapper {
  display: flex;
  align-items: flex-start;
}

.bullet-mm-btn {
  flex-shrink: 0;
  order: 2;
  margin-top: 14px;
  margin-left: auto;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  border: 1px solid transparent;
  border-radius: 7px;
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  opacity: 1;
  transition: background 0.15s, border-color 0.15s, color 0.15s;
}

.bulletlist-wrapper ul {
  order: 1;
  flex: 1;
  min-width: 0;
}

.bullet-mm-btn:hover {
  background: var(--bg-elevated, #f5f5f7);
  border-color: var(--border-primary, #d2d2d7);
  color: var(--text-secondary, #6e6e73);
}

.bullet-mm-btn:active {
  background: var(--accent, #0071e3);
  border-color: var(--accent, #0071e3);
  color: #fff;
}

/* ============ 外部文件标识 ============ */
.external-file-badge {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  margin: 0 0 8px 0;
  border-radius: 6px;
  background: var(--bg-elevated, #f5f5f7);
  border: 1px solid var(--border-primary, #d2d2d7);
  font-size: 12px;
  color: var(--text-secondary, #6e6e73);
  flex-shrink: 0;
}

.external-file-badge svg {
  flex-shrink: 0;
}

.external-file-path {
  color: var(--text-muted, #999);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  direction: rtl;  /* 显示文件名部分 */
  text-align: left;
}
</style>
