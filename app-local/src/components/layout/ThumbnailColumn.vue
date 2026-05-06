<template>
  <section class="thumbnail-column" :class="{ 'collapsed': !isOpen, 'flipped': flipped }">
    <!-- 正面：文稿缩略图列表 -->
    <div class="face face-front">
      <template v-if="store.currentFolderDocuments.length > 0">
        <div
          v-for="doc in store.currentFolderDocuments"
          :key="doc.id"
          class="note-item"
          :class="{ 'active': store.selectedDocumentId === doc.id }"
          @click="handleSelectDocument(doc.id)"
        >
          <h4>{{ firstLineOf(doc.content) || '新文稿' }}</h4>
          <p class="note-meta">{{ formatUpdatedAt(doc.updatedAt) }}</p>
          <button
            class="flip-btn"
            title="查看详情"
            @click.stop="handleFlip(doc.id)"
          >
            <IconDetail />
          </button>
        </div>
      </template>

      <div v-else-if="store.selectedFolderId" class="empty-state">
        <p class="empty-hint">该目录下暂无文稿</p>
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
        <header class="panel-header">
          <h5 class="panel-title">版本历史</h5>
          <button
            class="panel-toggle"
            :title="versionsCollapsed ? '展开' : '收起'"
            @click="versionsCollapsed = !versionsCollapsed"
          >
            <IconChevron :direction="versionsCollapsed ? 'down' : 'up'" />
          </button>
        </header>
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
      </section>

      <!-- 下 2/3：文稿目录（大纲） -->
      <section class="panel outline-panel" :class="{ 'panel-collapsed': outlineCollapsed }">
        <header class="panel-header">
          <h5 class="panel-title">文稿目录</h5>
          <button
            class="panel-toggle"
            :title="outlineCollapsed ? '展开' : '收起'"
            @click="outlineCollapsed = !outlineCollapsed"
          >
            <IconChevron :direction="outlineCollapsed ? 'down' : 'up'" />
          </button>
        </header>
        <div class="panel-body">
          <div v-if="outline.length === 0" class="panel-empty">当前文稿没有标题，无法生成目录</div>
          <ul v-else class="outline-list">
            <li
              v-for="(o, idx) in outline"
              :key="idx"
              class="outline-item"
              :style="{ paddingLeft: (o.level - 1) * 12 + 4 + 'px' }"
              :title="o.text"
            >
              <span class="outline-level">H{{ o.level }}</span>
              <span class="outline-text">{{ o.text }}</span>
            </li>
          </ul>
        </div>
      </section>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useWorkspaceStore } from '../../stores/workspace'
import * as versionApi from '../../services/versionApi'
import type { DocumentVersionSummary } from '../../services/versionApi'
import IconDetail from '../icons/IconDetail.vue'
import IconChevron from '../icons/IconChevron.vue'

interface Props {
  isOpen: boolean
}
defineProps<Props>()

const store = useWorkspaceStore()

// ============ 翻转与背面数据 ============
const flipped = ref(false)
const activeDocId = ref<string | null>(null)
const versions = ref<DocumentVersionSummary[]>([])
const versionsLoading = ref(false)
const versionsError = ref('')

// 两个面板的折叠状态
const versionsCollapsed = ref(false)
const outlineCollapsed = ref(false)

// 当前文档大纲：从 currentDocument.content 中提取 h1~h6
const outline = computed<{ level: number; text: string }[]>(() => {
  const doc = store.currentDocument
  if (!doc || !doc.content) return []
  if (activeDocId.value && doc.id !== activeDocId.value) return []
  const tmp = document.createElement('div')
  tmp.innerHTML = doc.content
  const headings = tmp.querySelectorAll('h1,h2,h3,h4,h5,h6')
  const list: { level: number; text: string }[] = []
  headings.forEach(h => {
    const level = Number(h.tagName.slice(1)) || 1
    const text = (h.textContent || '').trim()
    if (text) list.push({ level, text })
  })
  return list
})

function isDraftId(id: string): boolean {
  return id.startsWith('draft_')
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

// 当前文稿被删除则自动返回正面
watch(() => store.currentFolderDocuments.map(d => d.id).join(','), () => {
  if (!flipped.value || !activeDocId.value) return
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

const handleSelectDocument = (docId: string) => {
  store.selectDocument(docId)
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

/* ========== 正反两面：横向滑入 + 淡入 ========== */
.face {
  position: absolute;
  inset: 0;
  overflow-y: auto;
  box-sizing: border-box;
  transition: transform 0.35s cubic-bezier(0.4, 0, 0.2, 1), opacity 0.3s ease;
}

.face-front {
  padding: 12px;
  transform: translateX(0);
  opacity: 1;
}

.face-back {
  /* 网格布局固定比例：返回行(auto) + 版本历史(1fr) + 文稿目录(2fr) */
  display: grid;
  grid-template-rows: auto 1fr 2fr;
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
  cursor: pointer;
  border: 2px solid transparent;
  transition: all 0.2s;
}

.note-item:hover {
  border-color: var(--border-primary);
  box-shadow: var(--shadow-card);
}

.note-item.active {
  border-color: var(--accent);
  box-shadow: var(--shadow-card-hover);
}

.note-item h4 {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0 0 8px 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
  min-height: 0; /* 关键：让内部滚动生效 */
  overflow: hidden;
  box-sizing: border-box;
}

.versions-panel {
  border-bottom: 1px solid var(--border-primary);
}

/* 折叠时面板在 grid 中的位置和尺寸不变，只隐藏 body */
.panel.panel-collapsed {
  /* grid 位置不变，仅隐藏 body */
}
.panel.panel-collapsed .panel-body {
  max-height: 0;
  padding-top: 0;
  padding-bottom: 0;
  opacity: 0;
}

.panel-header {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px 4px 10px;
}

.panel-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-secondary);
  margin: 0;
  text-transform: uppercase;
  letter-spacing: 0.5px;
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

.panel-body {
  flex: 1 1 auto;
  min-height: 0;
  padding: 4px 10px 10px 10px;
  overflow-y: auto;
  transition: max-height 0.3s ease, opacity 0.2s ease, padding 0.3s ease;
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
  border-radius: 4px;
  cursor: pointer;
  border: 1px solid transparent;
  transition: all 0.15s;
}

.version-item:hover {
  border-color: var(--accent);
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
  transition: opacity 0.35s ease, transform 0.35s cubic-bezier(0.4, 0, 0.2, 1);
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
  transition: transform 0.35s cubic-bezier(0.4, 0, 0.2, 1);
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
  padding: 3px 4px;
  font-size: 12px;
  color: var(--text-primary);
  cursor: default;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.outline-level {
  font-size: 10px;
  color: var(--text-muted);
  flex-shrink: 0;
}

.outline-text {
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
