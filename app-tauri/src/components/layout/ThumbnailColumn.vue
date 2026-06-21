<template>
  <section class="thumbnail-column" :class="{ 'collapsed': !isOpen, 'flipped': flipped }">
    <!-- 正面：文稿缩略图列表 -->
    <div class="face face-front">
      <div v-if="store.selectedFolderLocked" class="empty-state">
        <p class="empty-hint">当前目录已加锁，无法查看文稿</p>
      </div>
      <template v-else-if="store.currentFolderDocuments.length > 0">
        <TransitionGroup name="note-item" tag="div" class="doc-list-wrap">
          <div
            v-for="doc in store.currentFolderDocuments"
            :key="doc.id"
            class="note-item"
            :class="{
              'active': store.selectedDocumentId === doc.id,
              'dragging': isDraggingDoc && _dragDocId === doc.id,
              'drag-over-above': dragOverDocId === doc.id && dragOverPos === 'above',
              'drag-over-below': dragOverDocId === doc.id && dragOverPos === 'below'
            }"
            :draggable="!doc.isLocked"
            @click="handleSelectDocument(doc.id)"
            @contextmenu.prevent="handleDocContextMenu($event, doc.id)"
            @dragstart="onDocDragStart($event, doc.id)"
            @dragover="onDocDragOver($event, doc.id)"
            @dragleave="onDocDragLeave"
            @drop="onDocDrop($event, doc.id)"
            @dragend="onDocDragEnd"
          >
            <span class="grip-area">
              <IconGrip />
            </span>
            <h4>
              <template v-if="doc.isLocked">
                <IconLock class="doc-lock-icon" />
                <span class="locked-label">已锁定</span>
              </template>
              <template v-else>
                {{ firstLineOf(doc.content) || '新文稿' }}
              </template>
            </h4>
            <p class="note-meta">{{ formatUpdatedAt(doc.updatedAt) }}</p>
            <span v-if="isAllView && doc.folderName" class="source-badge" :class="{ 'is-draft': doc.folderName === '草稿箱' || doc.folderName === '未分类' }">{{ doc.folderName }}</span>
            <span v-if="isTrashView && isDraftId(doc.id)" class="source-badge is-draft">草稿箱</span>
            <button
              v-if="!doc.isLocked"
              class="flip-btn"
              title="查看详情"
              @click.stop="handleFlip(doc.id)"
            >
              <IconDetail />
            </button>
            <div v-if="isTrashView" class="trash-actions">
              <button class="action-btn" title="恢复文稿" @click.stop="handleRestoreDocument(doc.id)">恢复</button>
              <button class="action-btn danger" title="彻底删除文稿" @click.stop="handleHardDeleteDocument(doc.id)">删除</button>
            </div>
          </div>
        </TransitionGroup>
      </template>

      <div v-else-if="store.selectedFolderId" class="empty-state">
        <p class="empty-hint">{{ emptyHint }}</p>
      </div>

      <div v-else class="empty-state">
        <p class="empty-hint">请在左侧选择一个目录</p>
      </div>
    </div>

    <!-- 背面：返回行 + 上 1/3 版本历史 + 下 2/3 文稿目录 -->
    <div class="face face-back">
      <!-- 独占一行的返回按钮 -->
      <div class="back-row">
        <button class="back-row-btn" title="返回" @click="handleUnflip">
          <IconChevron direction="left" />
        </button>
      </div>

      <!-- 上 1/3：版本历史 -->
      <section class="panel versions-panel" :class="{ 'panel-collapsed': versionsCollapsed }">
        <header class="panel-header" @click="versionsCollapsed = !versionsCollapsed">
          <h5 class="panel-title">版本历史</h5>
          <button
            class="panel-toggle"
            :title="versionsCollapsed ? '展开' : '收起'"
            @click.stop="versionsCollapsed = !versionsCollapsed"
          >
            <span class="panel-chevron-wrap" :class="{ 'is-collapsed': versionsCollapsed }">
              <IconChevron direction="up" />
            </span>
          </button>
        </header>
        <div class="panel-collapse-wrap">
          <div class="panel-body">
            <div v-if="versionsLoading" class="panel-loading">加载中…</div>
            <div v-else-if="versionsError" class="panel-error">{{ versionsError }}</div>
            <div v-else-if="versions.length === 0" class="panel-empty">暂无历史版本</div>
            <TransitionGroup v-else name="version" tag="ul" class="version-list">
              <li
                v-for="v in versions"
                :key="v.id"
                class="version-item"
                @click="handleRestore(v.versionNo)"
              >
                <div class="version-row1">
                  <span class="version-no">v{{ v.versionNo }}</span>
                  <span class="version-type" :class="'type-' + v.changeType">{{ changeTypeLabel(v.changeType) }}</span>
                  <span class="version-time">{{ formatUpdatedAt(v.createTime) }}</span>
                </div>
                <div class="version-preview">{{ v.preview || '（空白）' }}</div>
              </li>
            </TransitionGroup>
          </div>
        </div>
      </section>

      <!-- 下 2/3：文稿目录（大纲） -->
      <section class="panel outline-panel" :class="{ 'panel-collapsed': outlineCollapsed }">
        <header class="panel-header" @click="outlineCollapsed = !outlineCollapsed">
          <h5 class="panel-title">文稿目录</h5>
          <button
            class="panel-toggle"
            :title="outlineCollapsed ? '展开' : '收起'"
            @click.stop="outlineCollapsed = !outlineCollapsed"
          >
            <span class="panel-chevron-wrap" :class="{ 'is-collapsed': outlineCollapsed }">
              <IconChevron direction="up" />
            </span>
          </button>
        </header>
        <div class="panel-collapse-wrap">
          <div class="panel-body">
            <div v-if="outline.length === 0" class="panel-empty">当前文稿没有标题，无法生成目录</div>
            <ul v-else class="outline-list">
              <li
                v-for="(o, idx) in outline"
                :key="idx"
                class="outline-item"
                :class="{ 'outline-item-active': activeHeadingIndex === o.headingIndex }"
                :style="{ paddingLeft: (o.level - 1) * 12 + 4 + 'px' }"
                :title="o.text"
                @click="handleOutlineClick(o.headingIndex)"
              >
                <span class="outline-level">H{{ o.level }}</span>
                <span class="outline-text">{{ o.text }}</span>
              </li>
            </ul>
          </div>
        </div>
      </section>
    </div>

    <ContextMenu
      v-if="showContextMenu"
      :position="contextMenuPosition"
      :items="contextMenuItems"
      @select="handleMenuSelect"
      @close="closeContextMenu"
    />

    <LockPasswordDialog
      v-if="showLockDialog"
      :mode="lockDialogMode"
      @confirm="handleLockConfirm"
      @cancel="handleLockCancel"
    />
  </section>
</template>

<script setup lang="ts">
import { ref, computed, watch, onBeforeUnmount } from 'vue'
import { useWorkspaceStore } from '../../stores/workspace'
import { useLockStore } from '../../stores/lock'
import * as versionApi from '../../services/versionApi'
import type { DocumentVersionSummary } from '../../services/versionApi'
import type { ContextMenuItem, ContextMenuPosition } from '../../types/folder'
import type { Document } from '../../types/document'
import IconDetail from '../icons/IconDetail.vue'
import IconChevron from '../icons/IconChevron.vue'
import { log } from '../../services/logger'
import IconLock from '../icons/IconLock.vue'
import IconGrip from '../icons/IconGrip.vue'
import ContextMenu from '../ui/ContextMenu.vue'
import { useEditorNavigation, scrollToHeading as navScrollToHeading } from '../../config/editorNavigation'
import LockPasswordDialog from './LockPasswordDialog.vue'
import TurndownService from 'turndown'
import { tables as gfmTable, strikethrough as gfmStrikethrough } from 'turndown-plugin-gfm'
import { invoke } from '../../services/api'

interface Props {
  isOpen: boolean
  activeHeading?: number | null
}
const props = defineProps<Props>()

const { headings } = useEditorNavigation()

const store = useWorkspaceStore()
const lockStore = useLockStore()
const TRASH_FOLDER_ID = '__trash__'
const SEARCH_FOLDER_ID = '__search__'
const DRAFT_FOLDER_ID = '__drafts__'
const ALL_FOLDER_ID = '__all__'

// ============ 翻转与背面数据 ============
const flipped = ref(false)
const activeDocId = ref<string | null>(null)
const versions = ref<DocumentVersionSummary[]>([])
const versionsLoading = ref(false)
const versionsError = ref('')

// 两个面板的折叠状态
const versionsCollapsed = ref(false)
const outlineCollapsed = ref(false)
const showContextMenu = ref(false)
const contextMenuPosition = ref<ContextMenuPosition>({ x: 0, y: 0 })
const contextMenuItems = ref<ContextMenuItem[]>([])
const contextMenuDocId = ref<string | null>(null)
const activeHeadingIndex = ref<number | null>(null)

// 加锁弹窗状态
const showLockDialog = ref(false)
const lockDialogMode = ref<'set' | 'verify'>('verify')
const pendingDocAction = ref<{ type: 'select' | 'toggleLock'; docId: string; doc?: Document } | null>(null)

function handleOutlineClick(headingIndex: number) {
  navScrollToHeading(headingIndex)
}

// 当前文档大纲：从 EditArea 的 headings 获取（唯一数据源）
const outline = computed(() => {
  return headings.value.map(h => ({
    level: h.level,
    text: h.text,
    headingIndex: h.pos
  }))
})

const isTrashView = computed(() => store.selectedFolderId === TRASH_FOLDER_ID)
const isSearchView = computed(() => store.selectedFolderId === SEARCH_FOLDER_ID)
const isAllView = computed(() => store.selectedFolderId === ALL_FOLDER_ID)

const emptyHint = computed(() => {
  if (isTrashView.value) return '废纸篓为空'
  if (isSearchView.value) return '未搜索到文稿'
  if (isAllView.value) return '还没有文稿'
  if (store.selectedFolderId === DRAFT_FOLDER_ID) return '草稿箱为空'
  return '该目录下暂无文稿'
})

function isDraftId(id: string): boolean {
  return id.startsWith('draft_')
}

/* ========== 拖拽排序（文稿略缩图） ========== */
let _dragDocId: string | null = null
const dragOverDocId = ref<string | null>(null)
const dragOverPos = ref<'above' | 'below' | null>(null)
const isDraggingDoc = ref(false)

function onDocDragStart(event: DragEvent, docId: string) {
  const doc = store.currentFolderDocuments.find(d => d.id === docId)
  if (doc?.isLocked) { event.preventDefault(); return }
  _dragDocId = docId
  isDraggingDoc.value = true
  event.dataTransfer!.effectAllowed = 'move'
  event.dataTransfer!.setData('text/plain', docId)
}

function onDocDragOver(event: DragEvent, docId: string) {
  if (!_dragDocId || _dragDocId === docId) return
  event.preventDefault()
  const el = event.currentTarget as HTMLElement
  const rect = el.getBoundingClientRect()
  dragOverDocId.value = docId
  dragOverPos.value = event.clientY < rect.top + rect.height / 2 ? 'above' : 'below'
}

function onDocDragLeave(event: DragEvent) {
  const el = event.currentTarget as HTMLElement
  if (!el.contains(event.relatedTarget as Node)) {
    dragOverDocId.value = null
    dragOverPos.value = null
  }
}

async function onDocDrop(event: DragEvent, docId: string) {
  event.preventDefault()
  dragOverDocId.value = null
  dragOverPos.value = null
  if (!_dragDocId || _dragDocId === docId) return

  const siblings = store.folderDocuments
  const dragIdx = siblings.findIndex(d => d.id === _dragDocId)
  const dropIdx = siblings.findIndex(d => d.id === docId)
  if (dragIdx === -1 || dropIdx === -1) return

  const originalSnapshot = siblings.slice()
  try {
    const el = event.currentTarget as HTMLElement
    const rect = el.getBoundingClientRect()
    const insertAfter = event.clientY >= rect.top + rect.height / 2

    const [moved] = siblings.splice(dragIdx, 1)
    const newDropIdx = siblings.findIndex(d => d.id === docId)
    if (newDropIdx === -1) return
    siblings.splice(insertAfter ? newDropIdx + 1 : newDropIdx, 0, moved)

    await store.reorderDocumentItems(siblings.map(d => d.id))
  } catch {
    siblings.splice(0, siblings.length, ...originalSnapshot)
  }
}

function onDocDragEnd() {
  _dragDocId = null
  dragOverDocId.value = null
  dragOverPos.value = null
  isDraggingDoc.value = false
}

async function loadVersions(docId: string) {
  versions.value = []
  versionsError.value = ''
  if (isDraftId(docId)) {
    versionsError.value = '草稿暂不支持版本历史'
    return
  }
  versionsLoading.value = true
  try {
    versions.value = await versionApi.listVersions(docId)
  } catch (e: any) {
    versionsError.value = e?.message || '加载版本历史失败'
  } finally {
    versionsLoading.value = false
  }
}

/**
 * 增量刷新：不清空数组、不切 loading 状态，让 TransitionGroup 保持存活原地 diff，
 * 从而新增项能走 enter 动画、已有项走 move 动画。仅供 tick 触发的刷新使用。
 */
async function refreshVersions(docId: string) {
  if (isDraftId(docId)) return
  try {
    const list = await versionApi.listVersions(docId)
    // 只在当前还在显示同一个文稿时才替换，避免竞态
    if (activeDocId.value === docId && flipped.value) {
      versions.value = list
    }
  } catch {
    /* 静默失败，不骚扰用户 */
  }
}

async function handleFlip(docId: string) {
  if (isTrashView.value) return
  activeDocId.value = docId
  if (store.selectedDocumentId !== docId) {
    await store.selectDocument(docId)
  }
  flipped.value = true
  await loadVersions(docId)
}

function handleUnflip() {
  flipped.value = false
}

async function handleRestore(versionNo: number) {
  const id = activeDocId.value
  if (!id) return
  if (!window.confirm(`确定回滚到 v${versionNo}？当前内容会作为新的 restore 版本保留。`)) return
  try {
    await versionApi.restoreVersion(id, versionNo)
    await store.selectDocument(id)
    await loadVersions(id)
  } catch (e: any) {
    window.alert(e?.message || '回滚失败')
  }
}

async function handleRestoreDocument(docId: string) {
  await store.restoreDocument(docId)
}

async function handleHardDeleteDocument(docId: string) {
  if (!window.confirm('确认彻底删除该文稿？该操作不可恢复。')) return
  await store.hardDeleteDocument(docId)
}

const _turndown = new TurndownService({
  headingStyle: 'atx',       // ## 标题
  codeBlockStyle: 'fenced',  // ``` 代码块
  emDelimiter: '*',          // *斜体*
  bulletListMarker: '-',     // - 无序列表
})
_turndown.use(gfmTable)
_turndown.use(gfmStrikethrough)
// 任务列表（- [x] / - [ ]），与 EditArea 中的规则保持一致
_turndown.addRule('taskList', {
  filter: (node: HTMLElement) =>
    node.nodeName === 'LI' &&
    (node.getAttribute('data-checked') !== null ||
     (!!node.parentElement && node.parentElement.getAttribute('data-type') === 'taskList')),
  replacement: (content: string, node: HTMLElement) => {
    const checked = node.getAttribute('data-checked') === 'true'
    return `- [${checked ? 'x' : ' '}] ${content.trim()}\n`
  },
})

async function handleExportDoc(docId: string) {
  const doc = store.currentFolderDocuments.find(d => d.id === docId)
  if (!doc) return

  const title = doc.title || '未命名文稿'
  const html = doc.content || ''

  const result = await invoke<{ filePath: string }>('dialog:save-document', {
    defaultName: `${title}.md`,
    filters: [
      { name: 'Markdown 文件', extensions: ['md'] },
      { name: 'PDF 文件', extensions: ['pdf'] },
      { name: '图片文件', extensions: ['webp'] },
    ],
  })
  if (!result?.filePath) return

  const ext = result.filePath.toLowerCase()
  if (ext.endsWith('.md')) {
    const md = _turndown.turndown(html)
    await invoke('file:write', { filePath: result.filePath, data: md, isBase64: false })
  } else if (ext.endsWith('.pdf')) {
    await invoke('document:export-pdf', { filePath: result.filePath, html })
  } else if (ext.endsWith('.webp')) {
    const webpBase64 = await renderHtmlToWebPBase64(html)
    await invoke('file:write', { filePath: result.filePath, data: webpBase64, isBase64: true })
  }
}

async function renderHtmlToWebPBase64(html: string): Promise<string> {
  // Render HTML to canvas using SVG foreignObject, then encode as WebP
  const svg = `
<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="1600">
  <foreignObject width="100%" height="100%">
    <div xmlns="http://www.w3.org/1999/xhtml" style="font-family:-apple-system,&quot;Microsoft YaHei&quot;,sans-serif;padding:20px;background:#fff;color:#333;">
      ${html}
    </div>
  </foreignObject>
</svg>`
  const svgBlob = new Blob([svg], { type: 'image/svg+xml;charset=utf-8' })
  const url = URL.createObjectURL(svgBlob)

  const img = new Image()
  await new Promise<void>((resolve, reject) => {
    img.onload = () => resolve()
    img.onerror = () => reject(new Error('渲染 HTML 为图片失败'))
    img.src = url
  })

  const canvas = document.createElement('canvas')
  canvas.width = img.naturalWidth
  canvas.height = img.naturalHeight
  const ctx = canvas.getContext('2d')!
  ctx.drawImage(img, 0, 0)

  URL.revokeObjectURL(url)

  return new Promise((resolve, reject) => {
    canvas.toBlob(async (blob) => {
      if (!blob) { reject(new Error('WebP 转换失败')); return }
      const buf = await blob.arrayBuffer()
      resolve(btoa(String.fromCharCode(...new Uint8Array(buf))))
    }, 'image/webp', 0.92)
  })
}

function handleDocContextMenu(event: MouseEvent, docId: string) {
  contextMenuDocId.value = docId
  contextMenuPosition.value = { x: event.clientX, y: event.clientY }
  const doc = store.currentFolderDocuments.find(d => d.id === docId)
  if (isTrashView.value) {
    contextMenuItems.value = [
      { label: '清空废纸篓', action: 'emptyTrash' },
      { label: '彻底删除', action: 'hardDelete' }
    ]
  } else {
    const items: ContextMenuItem[] = []
    if (doc?.isLocked !== undefined) {
      items.push({ label: doc.isLocked ? '解锁' : '加锁', action: 'toggleLock' })
    }
    items.push({ label: '导出', action: 'exportDoc' })
    items.push({ label: '删除', action: 'delete' })
    contextMenuItems.value = items
  }
  showContextMenu.value = true
}

function closeContextMenu() {
  showContextMenu.value = false
  contextMenuDocId.value = null
}

async function handleMenuSelect(action: string) {
  const docId = contextMenuDocId.value
  if (!docId) {
    closeContextMenu()
    return
  }
  if (action === 'delete') {
    await store.deleteDocument(docId)
    closeContextMenu()
    return
  }
  if (action === 'hardDelete') {
    await handleHardDeleteDocument(docId)
    closeContextMenu()
    return
  }
  if (action === 'emptyTrash') {
    if (!window.confirm('确认清空废纸篓？该操作不可恢复。')) {
      closeContextMenu()
      return
    }
    await store.emptyTrash()
    closeContextMenu()
    return
  }
  if (action === 'toggleLock') {
    const doc = store.currentFolderDocuments.find(d => d.id === docId)
    if (doc) {
      if (lockStore.sessionVerified) {
        await performDocToggleLock(doc)
      } else {
        pendingDocAction.value = { type: 'toggleLock', docId, doc }
        lockDialogMode.value = lockStore.hasPassword ? 'verify' : 'set'
        showLockDialog.value = true
      }
    }
    closeContextMenu()
    return
  }
  if (action === 'exportDoc') {
    await handleExportDoc(docId)
    closeContextMenu()
    return
  }
  closeContextMenu()
}

// ============ 文档加锁/解锁 ============
async function handleLockConfirm(_password: string) {
  showLockDialog.value = false
  const action = pendingDocAction.value
  pendingDocAction.value = null
  if (!action) return
  if (action.type === 'select') {
    await store.selectDocument(action.docId)
  } else if (action.type === 'toggleLock' && action.doc) {
    await performDocToggleLock(action.doc)
  }
}

function handleLockCancel() {
  showLockDialog.value = false
  pendingDocAction.value = null
}

async function performDocToggleLock(doc: Document) {
  try {
    if (doc.isLocked) {
      await lockStore.unlockDocument(doc.id)
    } else {
      await lockStore.lockDocument(doc.id)
    }
    await store.syncRefresh()
  } catch (e: any) {
    log.lock.error('Lock operation failed:', e)
  }
}

// 当前文稿被删除则自动返回正面
watch(() => store.currentFolderDocuments.map(d => d.id).join(','), () => {
  if (!flipped.value || !activeDocId.value) return
  // loading 期间（bootstrap 重置列表）跳过，避免因临时空列表误翻转
  if (store.loading) return
  const exists = store.currentFolderDocuments.some(d => d.id === activeDocId.value)
  if (!exists) {
    flipped.value = false
    activeDocId.value = null
  }
})

// 版本列表实时刷新：监听 store.versionRefreshTick，只要当前翻转面板是同一个文稿，就重载
let reloadTimer: number | null = null
function scheduleReload() {
  if (!flipped.value || !activeDocId.value) return
  if (store.lastVersionedDocId && store.lastVersionedDocId !== activeDocId.value) return
  if (reloadTimer !== null) clearTimeout(reloadTimer)
  reloadTimer = window.setTimeout(() => {
    reloadTimer = null
    if (flipped.value && activeDocId.value) {
      // 使用增量刷新：保留 TransitionGroup，仅开启新增项 enter 动画
      void refreshVersions(activeDocId.value)
    }
  }, 300)
}
watch(() => store.versionRefreshTick, () => scheduleReload())
// 翻转打开后若期间有给 tick，也追加一次刷新
watch(flipped, (v) => {
  if (!v && reloadTimer !== null) {
    clearTimeout(reloadTimer)
    reloadTimer = null
  }
})

// 组件卸载时清除待执行的 reloadTimer，防止回调操作已卸载的响应式状态
onBeforeUnmount(() => {
  if (reloadTimer !== null) {
    clearTimeout(reloadTimer)
    reloadTimer = null
  }
})

// 编辑器滚动时同步大纲高亮（仅在背面展开时）
watch(() => props.activeHeading, (idx) => {
  activeHeadingIndex.value = idx ?? null
})

const handleSelectDocument = (docId: string) => {
  const doc = store.currentFolderDocuments.find(d => d.id === docId)
  if (doc?.isLocked && !lockStore.sessionVerified) {
    pendingDocAction.value = { type: 'select', docId }
    lockDialogMode.value = lockStore.hasPassword ? 'verify' : 'set'
    showLockDialog.value = true
    return
  }
  void store.selectDocument(docId)
}

function firstLineOf(content: string | null | undefined): string {
  if (!content) return ''
  const tmp = document.createElement('div')
  tmp.innerHTML = content
  const block = tmp.querySelector('p, h1, h2, h3, h4, h5, h6, li, blockquote, pre')
  let text = (block?.textContent || tmp.textContent || '').replace(/\u00A0/g, ' ')
  text = text.split(/\r?\n/).map(s => s.trim()).find(Boolean) || ''
  return text.length > 40 ? text.slice(0, 40) + '…' : text
}

function formatUpdatedAt(value: string | null | undefined): string {
  if (!value) return ''
  const d = new Date(value)
  if (isNaN(d.getTime())) return ''
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  const hh = String(d.getHours()).padStart(2, '0')
  const mm = String(d.getMinutes()).padStart(2, '0')
  return `${y}-${m}-${day} ${hh}:${mm}`
}

function changeTypeLabel(t: string): string {
  if (t === 'manual') return '手动'
  if (t === 'restore') return '回滚'
  return '自动'
}
</script>

<style scoped>
.thumbnail-column {
  position: relative;
  width: 250px;
  background-color: var(--bg-tertiary);
  border-right: 1px solid var(--border-primary);
  overflow: hidden;
  transition: width 0.3s, padding 0.3s, opacity 0.3s, var(--theme-transition);
}

.thumbnail-column.collapsed {
  width: 0;
  padding: 0;
  opacity: 0;
}

/* ========== 正反两面：横向滑入 + 淡入（Apple 风格） ========== */
.face {
  position: absolute;
  inset: 0;
  overflow-y: auto;
  box-sizing: border-box;
  transition: transform 0.5s cubic-bezier(0.42, 0, 0.28, 1), opacity 0.4s ease;
}

.face-front {
  padding: 12px;
  transform: translateX(0);
  opacity: 1;
}

.face-back {
  display: flex;
  flex-direction: column;
  transform: translateX(24px);
  opacity: 0;
  pointer-events: none;
  background-color: var(--bg-tertiary);
}

.thumbnail-column.flipped .face-front {
  transform: translateX(-24px);
  opacity: 0;
  pointer-events: none;
}

.thumbnail-column.flipped .face-back {
  transform: translateX(0);
  opacity: 1;
  pointer-events: auto;
}

/* ========== 正面：缩略图 ========== */
.note-item {
  position: relative;
  padding: 12px;
  margin-bottom: 12px;
  background-color: var(--bg-elevated);
  border-radius: 6px;
  cursor: grab;
  border: 2px solid transparent;
  transition:
    transform 0.3s cubic-bezier(0.42, 0, 0.28, 1),
    border-color 0.3s ease,
    box-shadow 0.3s ease,
    background-color 0.25s ease;
}

.note-item:active {
  cursor: grabbing;
  transform: scale(0.985);
}

.note-item:hover {
  border-color: var(--border-primary);
  box-shadow: var(--shadow-card);
  transform: translateY(-1px);
}

.note-item.active {
  border-color: var(--accent);
  box-shadow: var(--shadow-card-hover);
}

.note-item.dragging {
  opacity: 0.4;
}
.note-item.dragging:active {
  transform: none;
}

/* 文稿拖拽放置指示线 */
.note-item.drag-over-above::before {
  content: '';
  position: absolute;
  top: -7px;
  left: 4px;
  right: 4px;
  height: 3px;
  background: var(--accent);
  border-radius: 2px;
  pointer-events: none;
  z-index: 1;
}
.note-item.drag-over-below::after {
  content: '';
  position: absolute;
  bottom: -7px;
  left: 4px;
  right: 4px;
  height: 3px;
  background: var(--accent);
  border-radius: 2px;
  pointer-events: none;
  z-index: 1;
}

/* 拖拽手柄：默认隐藏，hover 时显示 */
.note-item .grip-area {
  position: absolute;
  top: 2px;
  right: 2px;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 4px;
  opacity: 0;
  transition: opacity 0.2s ease, background-color 0.2s ease;
  color: var(--text-muted);
  z-index: 1;
}
.note-item:hover .grip-area {
  opacity: 0.4;
}
.note-item .grip-area:hover {
  opacity: 0.8;
  background-color: var(--bg-hover);
}

.note-item h4 {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0 0 8px 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  display: flex;
  align-items: center;
  gap: 4px;
}

.doc-lock-icon {
  flex-shrink: 0;
  color: var(--accent, #409eff);
  opacity: 0.85;
}

.locked-label {
  color: var(--text-muted);
  font-weight: 400;
  font-style: italic;
}

.note-item p {
  font-size: 12px;
  color: var(--text-secondary);
  margin: 0;
  line-height: 1.5;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.flip-btn {
  position: absolute;
  right: 6px;
  bottom: 6px;
  width: 22px;
  height: 22px;
  border-radius: 4px;
  border: none;
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
  transition: opacity 0.15s, background-color 0.15s, color 0.15s;
}

.note-item:hover .flip-btn,
.note-item.active .flip-btn {
  opacity: 1;
}

.flip-btn:hover {
  background-color: var(--bg-hover, rgba(0, 0, 0, 0.06));
  color: var(--accent);
}

.trash-actions {
  margin-top: 10px;
  display: flex;
  gap: 8px;
}

.action-btn {
  border: 1px solid var(--border-primary);
  border-radius: 4px;
  background: var(--bg-tertiary);
  color: var(--text-primary);
  cursor: pointer;
  font-size: 12px;
  padding: 2px 8px;
}

.action-btn:hover {
  border-color: var(--accent);
}

.action-btn.danger {
  color: #d9534f;
}

/* 全部视图中文稿来源标记 */
.source-badge {
  display: inline-block;
  font-size: 10px;
  padding: 1px 6px;
  margin-top: 6px;
  border-radius: 3px;
  background-color: var(--bg-tertiary);
  color: var(--text-muted);
  line-height: 1.6;
}
.source-badge.is-draft {
  background-color: var(--accent-light);
  color: var(--accent);
}

.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 120px;
}

.empty-hint {
  font-size: 13px;
  color: var(--text-muted);
  text-align: center;
}

/* ========== 背面 ========== */
/* 返回按钮独占一行 */
.back-row {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  padding: 6px 10px;
  border-bottom: 1px solid var(--border-primary);
}

.back-row-btn {
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background-color 0.15s, color 0.15s;
}
.back-row-btn:hover {
  background-color: var(--bg-hover, rgba(0, 0, 0, 0.06));
  color: var(--accent);
}

.panel {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  box-sizing: border-box;
}

/*
 * Apple 风格缓动曲线：cubic-bezier(0.42, 0, 0.28, 1)
 * 起止平滑、无突兀感，配合 0.5s 时长营造流畅的物理感
 *
 * 折叠/展开使用 grid-template-rows 过渡，
 * 让高度变化跟随内容实际尺寸而非固定的 max-height 值，
 * 消除「加速-减速」的突兀感。
 */

.versions-panel {
  flex-shrink: 0;
  max-height: 33.33%;
  border-bottom: 1px solid var(--border-primary);
  transition: border-color 0.35s ease,
              var(--theme-transition);
}

.outline-panel {
  flex: 1;
  min-height: 0;
  transition: var(--theme-transition);
}

/* 折叠：border 淡出 */
.versions-panel.panel-collapsed {
  border-color: transparent;
}

/* ---- collapse-wrap：grid-template-rows 驱动高度过渡 ---- */
.panel-collapse-wrap {
  display: grid;
  grid-template-rows: 1fr;
  min-height: 0;
  overflow: hidden;
  transition: grid-template-rows 0.5s cubic-bezier(0.42, 0, 0.28, 1),
              opacity 0.25s ease;
}
.panel:not(.panel-collapsed) .panel-collapse-wrap {
  flex: 1;
}

.panel.panel-collapsed .panel-collapse-wrap {
  grid-template-rows: 0fr;
}

/* ---- panel-body：内容 fade + slide ---- */
.panel-body {
  overflow: hidden;
  padding: 4px 10px 10px 10px;
  opacity: 1;
  transform: translateY(0);
  transition:
    opacity 0.25s ease,
    transform 0.35s cubic-bezier(0.42, 0, 0.28, 1),
    padding 0.3s ease;
}
.panel:not(.panel-collapsed) .panel-body {
  overflow-y: auto;
}
.panel.panel-collapsed .panel-body {
  padding-top: 0;
  padding-bottom: 0;
  opacity: 0;
  transform: translateY(-6px);
}

/* ---- panel-header：hover 反馈 + 折叠时轻微下沉阴影 ---- */
.panel-header {
  flex-shrink: 0;
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px 4px 10px;
  cursor: pointer;
  user-select: none;
  border-radius: 4px;
  margin: 0 4px;
  transition:
    background-color 0.15s,
    box-shadow 0.5s cubic-bezier(0.42, 0, 0.28, 1);
}
.panel-header:hover {
  background-color: var(--bg-hover);
}
.panel-header:active {
  background-color: var(--bg-active);
}
/* 折叠后 header 增加轻微阴影，营造「浮起」层次感（Apple 风格） */
.panel-collapsed .panel-header {
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
}

.panel-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-secondary);
  margin: 0;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  transition: color 0.35s ease;
}
.panel-collapsed .panel-title {
  color: var(--text-muted);
}

/* ---- chevron 旋转 ----
 * 使用 wrapper 旋转避免与 IconChevron 内部样式冲突 */
.panel-chevron-wrap {
  display: flex;
  align-items: center;
  justify-content: center;
  transition: transform 0.5s cubic-bezier(0.42, 0, 0.28, 1);
}
.panel-chevron-wrap.is-collapsed {
  transform: rotate(180deg);
}

.panel-toggle {
  border: none;
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  width: 20px;
  height: 20px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background-color 0.15s, color 0.15s;
}
.panel-toggle:hover {
  background-color: var(--bg-hover, rgba(0, 0, 0, 0.06));
  color: var(--accent);
}

.panel-loading,
.panel-empty,
.panel-error {
  font-size: 12px;
  color: var(--text-muted);
  padding: 6px 0;
}

.panel-error {
  color: #d9534f;
}

/* 版本列表 */
.version-list {
  list-style: none;
  padding: 0;
  margin: 0;
  position: relative; /* 供 leave 时 absolute 定位使用 */
}

.version-item {
  padding: 6px 8px;
  margin-bottom: 4px;
  background-color: var(--bg-elevated);
  border-radius: 5px;
  cursor: pointer;
  border: 1px solid transparent;
  transition:
    border-color 0.25s ease,
    background-color 0.25s ease,
    transform 0.25s cubic-bezier(0.42, 0, 0.28, 1);
}

.version-item:hover {
  border-color: var(--accent);
  background-color: var(--bg-hover);
}

.version-item:active {
  transform: scale(0.98);
}

.version-row1 {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--text-primary);
  margin-bottom: 2px;
}

.version-no {
  font-weight: 600;
  color: var(--accent);
}

.version-type {
  font-size: 10px;
  padding: 1px 5px;
  border-radius: 3px;
  background-color: var(--bg-tertiary);
  color: var(--text-secondary);
}
.version-type.type-manual {
  background-color: rgba(102, 187, 106, 0.15);
  color: #43a047;
}
.version-type.type-restore {
  background-color: rgba(255, 167, 38, 0.15);
  color: #fb8c00;
}

.version-time {
  margin-left: auto;
  font-size: 11px;
  color: var(--text-muted);
}

.version-preview {
  font-size: 11px;
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ===== 版本列表过渡动画 ===== */
/* 新项进入：从上方淡入下滑 + 短暂强调高亮 */
.version-enter-active {
  transition: opacity 0.4s ease, transform 0.45s cubic-bezier(0.42, 0, 0.28, 1);
  animation: version-pulse 1.2s ease;
}
.version-enter-from {
  opacity: 0;
  transform: translateY(-12px);
}
.version-enter-to {
  opacity: 1;
  transform: translateY(0);
}

/* 移出：淡出 + 向左滑 */
.version-leave-active {
  transition: opacity 0.25s ease, transform 0.25s ease;
  position: absolute;
  left: 0;
  right: 0;
  pointer-events: none;
}
.version-leave-to {
  opacity: 0;
  transform: translateX(-12px);
}

/* 保留项下移：平滑让位 */
.version-move {
  transition: transform 0.5s cubic-bezier(0.42, 0, 0.28, 1);
}

@keyframes version-pulse {
  0% {
    background-color: rgba(var(--accent-rgb, 64, 158, 255), 0.22);
    box-shadow: 0 0 0 1px rgba(var(--accent-rgb, 64, 158, 255), 0.35);
  }
  60% {
    background-color: rgba(var(--accent-rgb, 64, 158, 255), 0.10);
    box-shadow: 0 0 0 1px rgba(var(--accent-rgb, 64, 158, 255), 0);
  }
  100% {
    background-color: var(--bg-elevated);
    box-shadow: none;
  }
}

/* 大纲列表 */
.outline-list {
  list-style: none;
  padding: 0;
  margin: 0;
}

.outline-item {
  display: flex;
  align-items: baseline;
  gap: 6px;
  padding: 4px 6px;
  font-size: 12px;
  color: var(--text-primary);
  cursor: pointer;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  border-radius: 5px;
  transition:
    background-color 0.25s ease,
    padding-left 0.3s ease;
}

.outline-item:hover {
  background-color: var(--bg-hover);
}

.outline-item:active {
  background-color: var(--bg-active);
  transform: scale(0.985);
}

.outline-item-active {
  background-color: var(--bg-active);
}

.outline-level {
  font-size: 10px;
  color: var(--text-muted);
  flex-shrink: 0;
  font-weight: 500;
  letter-spacing: 0.3px;
}

.outline-text {
  overflow: hidden;
  text-overflow: ellipsis;
}

.doc-list-wrap {
  position: relative;
}

/* ===== 文稿列表缓入/缓出动画（与 FolderTree TransitionGroup 风格一致） ===== */
.note-item-enter-active {
  transition:
    opacity 0.35s cubic-bezier(0.25, 0.1, 0.25, 1),
    transform 0.35s cubic-bezier(0.25, 0.1, 0.25, 1);
}
.note-item-leave-active {
  transition:
    opacity 0.25s cubic-bezier(0.25, 0.1, 0.25, 1),
    transform 0.25s cubic-bezier(0.25, 0.1, 0.25, 1);
  position: absolute;
  left: 12px;
  right: 12px;
  pointer-events: none;
}
.note-item-move {
  transition: transform 0.35s cubic-bezier(0.25, 0.1, 0.25, 1);
}
.note-item-enter-from {
  opacity: 0;
  transform: translateY(-10px);
}
.note-item-leave-to {
  opacity: 0;
  transform: translateY(10px);
}
</style>
