<template>
  <section class="edit-area">
    <div v-if="showSheet" class="sheet-wrap">
      <div class="sheet-tools">
        <button class="tool-btn" @click="applyStyle('bold')">B</button>
        <button class="tool-btn" @click="applyStyle('italic')">I</button>
        <button class="tool-btn" @click="setAlign('left')">Left</button>
        <button class="tool-btn" @click="setAlign('center')">Center</button>
        <button class="tool-btn" @click="setAlign('right')">Right</button>
        <button class="tool-btn" @click="toggleTopHeader">Top Header</button>
        <button class="tool-btn" @click="toggleLeftHeader">Left Header</button>
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
          <EditorContent :editor="editor" class="editor-content" :style="editorScaleStyle" />
        </div>
      </template>
      <div v-else-if="store.selectedFolderLocked" class="locked-placeholder">
        <IconLock class="locked-placeholder-icon" />
        <p class="locked-placeholder-text">当前目录已加锁，无法查看文稿</p>
      </div>
      <div v-else class="empty-editor">Please select a document</div>
    </div>

    <div class="editor-statusbar" :class="{ collapsed: !isStatusBarOpen }">
      <div class="statusbar-left">
        <span>Markdown</span>
        <span v-if="currentBlock" class="current-block">{{ currentBlock }}</span>
      </div>
      <div class="statusbar-right">
        <span class="word-count">{{ wordCount }} words</span>
        <span>{{ lineCount }} lines</span>
        <span>{{ zoomPercent }}%</span>
      </div>
    </div>

    <!-- 思维导图预览对话框 -->
    <MindmapDialog
      v-if="showMindmapDialog && mindmapDialogData"
      :data="mindmapDialogData"
      @close="showMindmapDialog = false"
    />
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
import Highlight from '@tiptap/extension-highlight'
import Typography from '@tiptap/extension-typography'
import { Extension } from '@tiptap/vue-3'
import BulletList from '@tiptap/extension-bullet-list'
import { useWorkspaceStore } from '../../stores/workspace'
import { registerScrollHandler, useEditorNavigation } from '../../config/editorNavigation'
import type { TreeNode } from '../../types/mindmap'
import IconLock from '../icons/IconLock.vue'
import MindmapDialog from './MindmapDialog.vue'

const store = useWorkspaceStore()
const { headings } = useEditorNavigation()
const TRASH_FOLDER_ID = '__trash__'

interface Props { isStatusBarOpen?: boolean }
withDefaults(defineProps<Props>(), { isStatusBarOpen: true })

const emit = defineEmits<{
  (e: 'active-heading-change', headingIndex: number | null): void
}>()

const zoomPercent = ref(100)
const showSheet = ref(false)
const rows = 20
const cols = 10
const selected = ref<{ r: number; c: number } | null>(null)
const topHeader = ref(true)
const leftHeader = ref(true)
const cells = ref<string[][]>(Array.from({ length: rows }, () => Array.from({ length: cols }, () => '')))
const styles = ref<Record<string, { bold?: boolean; italic?: boolean; align?: 'left' | 'center' | 'right' }>>({})

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
  const isMod = navigator.platform.includes('Mac') ? event.metaKey : event.ctrlKey
  if (!isMod) return
  if (event.key === '+' || event.key === '=' || event.code === 'NumpadAdd') { event.preventDefault(); increaseZoom(); return }
  if (event.key === '-' || event.code === 'NumpadSubtract') { event.preventDefault(); decreaseZoom(); return }
  if (event.key === '0') { event.preventDefault(); resetZoom(); return }
}

let isSettingContent = false
const IDLE_MS = 3000
const CHAR_DELTA = 100
const LINE_DELTA = 10
let idleTimer: number | null = null
let baselineDocId: string | null = null
let baselineTextLen = 0
let baselineLines = 0
let dirtyAfterBaseline = false
let committing = false

function clearIdleTimer() { if (idleTimer !== null) { clearTimeout(idleTimer); idleTimer = null } }
function resetBaselineFromEditor() {
  if (!editor.value) return
  baselineDocId = store.selectedDocumentId
  baselineTextLen = editor.value.getText().length
  baselineLines = editor.value.getJSON().content?.length || 0
  dirtyAfterBaseline = false
  clearIdleTimer()
}
async function triggerCommit(changeType: 'auto' | 'manual' = 'auto') {
  const docId = baselineDocId || store.selectedDocumentId
  if (!docId || !dirtyAfterBaseline || committing) return
  clearIdleTimer(); committing = true
  try { await store.commitDocumentVersion(docId, changeType) } finally { committing = false }
  resetBaselineFromEditor()
}

const CustomKeymap = Extension.create({
  name: 'customKeymap',
  addKeyboardShortcuts() {
    return {
      'Shift-Alt-1': () => this.editor.chain().focus().toggleHeading({ level: 1 }).run(),
      'Shift-Alt-2': () => this.editor.chain().focus().toggleHeading({ level: 2 }).run(),
      'Shift-Alt-3': () => this.editor.chain().focus().toggleHeading({ level: 3 }).run(),
      'Shift-Alt-4': () => this.editor.chain().focus().toggleHeading({ level: 4 }).run(),
      'Shift-Alt-5': () => this.editor.chain().focus().toggleHeading({ level: 5 }).run(),
      'Shift-Alt-6': () => this.editor.chain().focus().toggleHeading({ level: 6 }).run(),
      'Mod-Shift-h': () => this.editor.chain().focus().toggleHighlight().run(),
      'Mod-Shift-l': () => this.editor.chain().focus().toggleBulletList().run(),
      'Mod-Shift-o': () => this.editor.chain().focus().toggleOrderedList().run(),
      'Mod-Shift-t': () => this.editor.chain().focus().toggleTaskList().run(),
      'Mod-Shift-q': () => this.editor.chain().focus().toggleBlockquote().run(),
      'Mod-Shift-c': () => this.editor.chain().focus().toggleCodeBlock().run(),
      'Mod-Shift-x': () => this.editor.chain().focus().toggleStrike().run(),
      'Mod-Enter': () => this.editor.chain().focus().setHorizontalRule().run(),
      'Mod-s': () => { void triggerCommit('manual'); return true }
    }
  }
})

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

const editor = useEditor({
  content: '',
  extensions: [
    StarterKit.configure({ heading: { levels: [1, 2, 3, 4, 5, 6] }, bulletList: false }),
    Placeholder.configure({ placeholder: 'Start writing...' }),
    Underline,
    TaskList,
    TaskItem.configure({ nested: true }),
    Highlight.configure({ multicolor: false }),
    Typography,
    CustomKeymap,
    MindmapBulletList,
  ],
  editorProps: { attributes: { class: 'wysiwyg-editor', spellcheck: 'false' } },
  onUpdate: ({ editor: editorInstance }) => {
    if (isSettingContent || !store.selectedDocumentId) return
    if (store.selectedFolderId === TRASH_FOLDER_ID) return
    store.updateDocumentContent(store.selectedDocumentId, editorInstance.getHTML())
    if (baselineDocId !== store.selectedDocumentId) { resetBaselineFromEditor(); return }
    dirtyAfterBaseline = true
    const charDelta = Math.abs(editorInstance.getText().length - baselineTextLen)
    const lineDelta = Math.abs((editorInstance.getJSON().content?.length || 0) - baselineLines)
    if (charDelta >= CHAR_DELTA || lineDelta >= LINE_DELTA) { void triggerCommit('auto'); return }
    clearIdleTimer(); idleTimer = window.setTimeout(() => void triggerCommit('auto'), IDLE_MS)
    updateActiveHeading()
    syncHeadings()
  },
  onBlur: () => { void triggerCommit('auto') }
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
    const oldId = baselineDocId
    clearIdleTimer(); dirtyAfterBaseline = false
    try { await store.commitDocumentVersion(oldId, 'auto') } catch {}
  }
  isSettingContent = true
  if (newDoc && newDoc.isLocked) {
    editor.value.commands.setContent('')
  } else {
    editor.value.commands.setContent(newDoc ? newDoc.content : '')
  }
  isSettingContent = false
  resetBaselineFromEditor()
  loadSheet()
  // 文档切换后重新绑定 scroll 监听并计算当前高亮
  void nextTick(() => {
    setupScrollListener()
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
  scrollRAF = requestAnimationFrame(updateActiveHeading)
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

onMounted(() => {
  registerScrollHandler(scrollToPos)
  window.addEventListener('keydown', handleGlobalKeydown)
  loadSheet()
  void nextTick(() => {
    setupScrollListener()
    updateActiveHeading()
    syncHeadings()
  })
})

onBeforeUnmount(() => {
  if (dirtyAfterBaseline && baselineDocId) void store.commitDocumentVersion(baselineDocId, 'auto')
  clearIdleTimer()
  window.removeEventListener('keydown', handleGlobalKeydown)
  if (scrollRAF !== null) cancelAnimationFrame(scrollRAF)
  if (scrollElRef) {
    scrollElRef.removeEventListener('scroll', onEditorScroll)
    scrollElRef = null
  }
  editor.value?.destroy()
})
</script>

<style scoped>
.edit-area { flex: 1; display: flex; flex-direction: column; background-color: var(--editor-bg); overflow: hidden; position: relative; transition: var(--theme-transition); }
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
.editor-body { flex: 1; overflow-y: auto; }
.editor-scale-wrap { display: block; }
.editor-content { height: 100%; }
.empty-editor { display: flex; align-items: center; justify-content: center; height: 100%; color: var(--text-muted); font-size: 14px; }

.locked-placeholder { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; gap: 12px; }
.locked-placeholder-icon { width: 32px; height: 32px; color: var(--accent, #409eff); opacity: 0.6; }
.editor-statusbar { height: 28px; background-color: var(--bg-toolbar); border-top: 1px solid var(--border-primary); display: flex; align-items: center; justify-content: space-between; padding: 0 16px; font-size: 12px; color: var(--text-muted); flex-shrink: 0; overflow: hidden; transition: var(--theme-transition), height 0.25s ease, padding 0.25s ease, border-top-color 0.25s ease, opacity 0.2s ease, transform 0.25s ease; transform-origin: bottom; }
.editor-statusbar.collapsed { height: 0; padding-top: 0; padding-bottom: 0; border-top-color: transparent; transform: translateY(100%); opacity: 0; pointer-events: none; }
.statusbar-left, .statusbar-right { display: flex; gap: 12px; align-items: center; }
.current-block { color: var(--text-secondary); padding: 1px 6px; background-color: var(--editor-mark-bg); border-radius: 3px; font-size: 11px; }
.editor-body::-webkit-scrollbar { width: 6px; }
.editor-body::-webkit-scrollbar-track { background: var(--editor-bg); }
.editor-body::-webkit-scrollbar-thumb { background: var(--scrollbar-thumb); border-radius: 3px; }
.editor-body::-webkit-scrollbar-thumb:hover { background: var(--scrollbar-thumb-hover); }
</style>

<style>
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
.wysiwyg-editor p.is-editor-empty:first-child::before { content: attr(data-placeholder); float: left; color: var(--editor-placeholder); pointer-events: none; height: 0; }
.wysiwyg-editor h1 { font-size: 28px; font-weight: 700; margin: 24px 0 16px; padding-bottom: 8px; border-bottom: 1px solid var(--editor-border); color: var(--editor-text-heading); line-height: 1.4; }
.wysiwyg-editor h2 { font-size: 24px; font-weight: 600; margin: 20px 0 12px; padding-bottom: 6px; border-bottom: 1px solid var(--border-secondary); color: var(--editor-text-heading2); line-height: 1.4; }
.wysiwyg-editor h3 { font-size: 20px; font-weight: 600; margin: 16px 0 10px; color: var(--editor-text); line-height: 1.4; }
.wysiwyg-editor h4, .wysiwyg-editor h5, .wysiwyg-editor h6 { font-size: 16px; font-weight: 600; margin: 12px 0 8px; color: var(--editor-text-secondary); line-height: 1.4; }
.wysiwyg-editor p { margin: 12px 0; color: var(--editor-text-secondary); }
.wysiwyg-editor ul, .wysiwyg-editor ol { margin: 12px 0; padding-left: 24px; }
.wysiwyg-editor li { margin: 4px 0; color: var(--editor-text-secondary); }
.wysiwyg-editor li p { margin: 0; }
.wysiwyg-editor ul[data-type="taskList"] { list-style: none; padding-left: 0; }
.wysiwyg-editor ul[data-type="taskList"] li { display: flex; align-items: flex-start; gap: 8px; }
.wysiwyg-editor ul[data-type="taskList"] li > label { flex-shrink: 0; margin-top: 4px; }
.wysiwyg-editor ul[data-type="taskList"] li > label input[type="checkbox"] { accent-color: var(--accent); cursor: pointer; width: 16px; height: 16px; }
.wysiwyg-editor ul[data-type="taskList"] li > div { flex: 1; }
.wysiwyg-editor code { background-color: var(--editor-code-bg); padding: 2px 6px; border-radius: 3px; font-family: 'Consolas', 'Monaco', 'Courier New', monospace; font-size: 14px; color: var(--editor-code-text); }
.wysiwyg-editor pre { background-color: var(--editor-pre-bg); padding: 16px; border-radius: 6px; overflow-x: auto; margin: 12px 0; border: 1px solid var(--editor-pre-border); }
.wysiwyg-editor pre code { background: none; padding: 0; color: var(--editor-text); font-size: 14px; line-height: 1.6; }
.wysiwyg-editor blockquote { border-left: 4px solid var(--editor-blockquote-border); padding-left: 16px; margin: 12px 0; color: var(--editor-blockquote-text); font-style: italic; }
.wysiwyg-editor blockquote p { color: var(--editor-blockquote-text); }
.wysiwyg-editor hr { border: none; border-top: 1px solid var(--editor-border); margin: 20px 0; }
.wysiwyg-editor strong { color: var(--editor-text-heading); font-weight: 700; }
.wysiwyg-editor em { color: var(--editor-text-secondary); font-style: italic; }
.wysiwyg-editor s { color: var(--text-muted); }
.wysiwyg-editor u { text-decoration-color: var(--text-muted); }
.wysiwyg-editor mark { background-color: var(--editor-highlight-bg); color: var(--editor-highlight-text); padding: 1px 4px; border-radius: 2px; }
.wysiwyg-editor a { color: var(--editor-link); text-decoration: none; }
.wysiwyg-editor a:hover { text-decoration: underline; }
.wysiwyg-editor ::selection { background-color: var(--editor-selection); }

/* ═══════════════════════════════════════════════
   BulletList NodeView — 思维导图按钮
   ═══════════════════════════════════════════════ */
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
</style>
