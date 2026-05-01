<template>
  <section class="thumbnail-column" :class="{ 'collapsed': !isOpen }">
    <!-- 有文稿时展示列表 -->
    <template v-if="store.currentFolderDocuments.length > 0">
      <div 
        v-for="doc in store.currentFolderDocuments" 
        :key="doc.id"
        class="note-item" 
        :class="{ 'active': store.selectedDocumentId === doc.id }" 
        @click="handleSelectDocument(doc.id)"
      >
        <h4>{{ doc.title }}</h4>
        <p class="note-meta">{{ formatUpdatedAt(doc.updatedAt) }}</p>
      </div>
    </template>

    <!-- 选中了目录但没有文稿 -->
    <div v-else-if="store.selectedFolderId" class="empty-state">
      <p class="empty-hint">该目录下暂无文稿</p>
    </div>

    <!-- 未选中任何目录 -->
    <div v-else class="empty-state">
      <p class="empty-hint">请在左侧选择一个目录</p>
    </div>
  </section>
</template>

<script setup lang="ts">
import { useWorkspaceStore } from '../../stores/workspace'

interface Props {
  isOpen: boolean
}

defineProps<Props>()

const store = useWorkspaceStore()

const handleSelectDocument = (docId: string) => {
  store.selectDocument(docId)
}

// 列表项不携带 content（按需单独加载），此处仅显示更新时间
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
</script>

<style scoped>
.thumbnail-column {
  width: 250px;
  background-color: var(--bg-tertiary);
  border-right: 1px solid var(--border-primary);
  padding: 12px;
  overflow-y: auto;
  transition: width 0.3s, padding 0.3s, opacity 0.3s, var(--theme-transition);
}

.thumbnail-column.collapsed {
  width: 0;
  padding: 0;
  opacity: 0;
  overflow: hidden;
}

.note-item {
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
</style>
