<template>
  <Teleport to="body">
    <Transition name="drawer-fade">
      <div v-if="open" class="mobile-doc-drawer" @click.self="close">
        <div class="mobile-doc-panel" @click.stop>
          <div class="mobile-doc-header">
            <h3 class="mobile-doc-title">{{ title }}</h3>
            <button class="mobile-doc-close" @click="close">
              <IconClose :size="18" />
            </button>
          </div>

          <div class="mobile-doc-actions">
            <button class="mobile-doc-new" @click="handleNewDocument">
              <IconNewDocument :size="16" />
              <span>新建文稿</span>
            </button>
          </div>

          <div class="mobile-doc-list">
            <div
              v-for="doc in store.currentFolderDocuments"
              :key="doc.id"
              class="mobile-doc-item"
              :class="{ active: store.selectedDocumentId === doc.id }"
              @click="handleSelect(doc.id)"
            >
              <div class="mobile-doc-item-title">
                <IconLock v-if="doc.isLocked" :size="14" />
                <span>{{ firstLine(doc) || '新文稿' }}</span>
              </div>
              <div class="mobile-doc-item-meta">
                <span v-if="showFolderBadge && doc.folderName" class="mobile-doc-badge">{{ doc.folderName }}</span>
                <span>{{ formatTime(doc.updatedAt) }}</span>
              </div>
            </div>

            <div v-if="store.currentFolderDocuments.length === 0" class="mobile-doc-empty">
              暂无文稿
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useWorkspaceStore } from '../../stores/workspace'
import IconClose from '../icons/IconClose.vue'
import IconNewDocument from '../icons/IconNewDocument.vue'
import IconLock from '../icons/IconLock.vue'

interface Props {
  open: boolean
}

defineProps<Props>()

const emit = defineEmits<{ 'update:open': [value: boolean] }>()

const store = useWorkspaceStore()

const TRASH_FOLDER_ID = '__trash__'
const SEARCH_FOLDER_ID = '__search__'
const DRAFT_FOLDER_ID = '__drafts__'
const ALL_FOLDER_ID = '__all__'

const title = computed(() => {
  const fid = store.selectedFolderId
  if (fid === ALL_FOLDER_ID) return '全部文稿'
  if (fid === DRAFT_FOLDER_ID) return '草稿箱'
  if (fid === TRASH_FOLDER_ID) return '废纸篓'
  if (fid === SEARCH_FOLDER_ID) return '搜索结果'
  if (!fid) return '文稿列表'
  const folder = store.findFolderById(store.folders, fid)
  return folder ? folder.name : '文稿列表'
})

const showFolderBadge = computed(() => store.selectedFolderId === ALL_FOLDER_ID || store.selectedFolderId === SEARCH_FOLDER_ID)

function close() {
  emit('update:open', false)
}

async function handleSelect(docId: string) {
  await store.selectDocument(docId)
  close()
}

async function handleNewDocument() {
  await store.createNewDocument()
  close()
}

function firstLine(doc: { content?: string | null }): string {
  const content = doc.content || ''
  const tmp = document.createElement('div')
  tmp.innerHTML = content
  const block = tmp.querySelector('p, h1, h2, h3, h4, h5, h6, li, blockquote, pre')
  let text = (block?.textContent || tmp.textContent || '').replace(/\u00A0/g, ' ')
  text = text.split(/\r?\n/).map(s => s.trim()).find(Boolean) || ''
  return text.length > 40 ? text.slice(0, 40) + '…' : text
}

function formatTime(value: string | null | undefined): string {
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
</script>

<style scoped>
.mobile-doc-drawer {
  position: fixed;
  inset: 0;
  z-index: 210;
  background-color: rgba(0, 0, 0, 0.35);
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  padding-top: max(48px, env(safe-area-inset-top));
}

.mobile-doc-panel {
  background-color: var(--bg-surface);
  border-radius: 16px 16px 0 0;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
  box-shadow: 0 -4px 24px rgba(0, 0, 0, 0.2);
  padding-bottom: env(safe-area-inset-bottom);
}

.mobile-doc-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  border-bottom: 1px solid var(--border-secondary);
  flex-shrink: 0;
}

.mobile-doc-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0;
}

.mobile-doc-close {
  background: transparent;
  border: none;
  cursor: pointer;
  padding: 6px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  color: var(--text-secondary);
}

.mobile-doc-close:active {
  background-color: var(--bg-hover);
}

.mobile-doc-actions {
  padding: 10px 16px;
  flex-shrink: 0;
}

.mobile-doc-new {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  width: 100%;
  padding: 10px;
  border: 1px dashed var(--border-primary);
  border-radius: 8px;
  background: transparent;
  color: var(--text-secondary);
  font-size: 14px;
  cursor: pointer;
}

.mobile-doc-new:active {
  background-color: var(--bg-hover);
}

.mobile-doc-list {
  flex: 1;
  overflow-y: auto;
  padding: 0 16px 16px;
}

.mobile-doc-item {
  padding: 12px;
  margin-bottom: 8px;
  border-radius: 8px;
  background-color: var(--bg-elevated);
  border: 1px solid transparent;
  cursor: pointer;
  transition: all 0.15s ease;
}

.mobile-doc-item.active {
  border-color: var(--accent);
  background-color: var(--bg-selected);
}

.mobile-doc-item:active {
  transform: scale(0.985);
}

.mobile-doc-item-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 14px;
  font-weight: 500;
  color: var(--text-primary);
  margin-bottom: 6px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mobile-doc-item-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--text-muted);
}

.mobile-doc-badge {
  padding: 1px 6px;
  border-radius: 3px;
  background-color: var(--accent-light);
  color: var(--accent);
  font-size: 11px;
}

.mobile-doc-empty {
  text-align: center;
  padding: 32px 0;
  color: var(--text-muted);
  font-size: 14px;
}

.drawer-fade-enter-active,
.drawer-fade-leave-active {
  transition: opacity 0.25s ease;
}

.drawer-fade-enter-active .mobile-doc-panel,
.drawer-fade-leave-active .mobile-doc-panel {
  transition: transform 0.25s ease;
}

.drawer-fade-enter-from,
.drawer-fade-leave-to {
  opacity: 0;
}

.drawer-fade-enter-from .mobile-doc-panel,
.drawer-fade-leave-to .mobile-doc-panel {
  transform: translateY(100%);
}
</style>
