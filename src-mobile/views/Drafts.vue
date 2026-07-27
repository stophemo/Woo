<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { showToast } from 'vant'
import { useWorkspaceStore } from '../../src/stores/workspace'

const router = useRouter()
const store = useWorkspaceStore()
const loading = ref(true)

// 草稿箱 = 本地草稿 + 后端孤儿（无目录/目录已删）
const drafts = computed(() => store.currentFolderDocuments)

onMounted(async () => {
  try {
    await store.openDraftBox()
  } catch {
    showToast('加载草稿失败')
  } finally {
    loading.value = false
  }
})

function openNote(id: string) {
  router.push(`/note/${id}`)
}

async function newDraft() {
  // openDraftBox 已把 selectedFolderId 设为草稿箱 → createNewDocument 走本地草稿分支
  await store.createNewDocument()
  const id = store.selectedDocumentId
  if (id) router.push(`/note/${id}`)
}

function formatTime(ts: string) {
  const d = new Date(ts)
  return d.toLocaleString('zh-CN', { hour12: false })
}
</script>

<template>
  <div class="drafts-page">
    <header class="mobile-page-header">
      <div class="mobile-page-header__main">
        <h1 class="mobile-page-header__title">草稿箱</h1>
        <p class="mobile-page-header__meta">{{ loading ? '正在读取…' : `${drafts.length} 篇未归档文稿` }}</p>
      </div>
      <div class="mobile-page-header__actions">
        <button
          type="button"
          class="mobile-icon-button is-primary"
          title="新建草稿"
          aria-label="新建草稿"
          @click="newDraft"
        >
          <van-icon name="plus" />
        </button>
      </div>
    </header>

    <van-loading v-if="loading" class="mobile-loading" />

    <div v-else-if="drafts.length" class="draft-list">
      <van-cell
        v-for="d in drafts"
        :key="d.id"
        class="draft-cell"
        clickable
        @click="openNote(d.id)"
      >
        <template #title>
          <div class="draft-heading">
            <span class="draft-dot" aria-hidden="true"></span>
            <span class="draft-title">{{ d.title || '无标题' }}</span>
          </div>
        </template>
        <template #label>
          <span class="draft-preview">{{ store.getDocumentPreview(d) || '暂无正文' }}</span>
          <span class="draft-meta">
            <span class="draft-source">{{ d.folderName || '草稿箱' }}</span>
            <time class="draft-time">{{ formatTime(d.updatedAt) }}</time>
          </span>
        </template>
      </van-cell>
    </div>

    <van-empty v-else description="暂无草稿" />
  </div>
</template>

<style scoped>
.drafts-page {
  min-height: 100vh;
  min-height: 100dvh;
  background: var(--van-background);
}

.draft-list {
  padding: 12px;
}

.draft-cell {
  margin-bottom: 8px;
  padding: 14px 15px;
  overflow: hidden;
  border: 1px solid var(--van-border-color);
  border-radius: 7px;
}

.draft-cell::after {
  display: none;
}

.draft-heading {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 8px;
}

.draft-dot {
  width: 7px;
  height: 7px;
  flex: 0 0 7px;
  border-radius: 50%;
  background: #ec7047;
}

.draft-title {
  overflow: hidden;
  color: var(--van-text-color);
  font-size: 15px;
  font-weight: 650;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.draft-preview {
  display: block;
  overflow: hidden;
  margin: 7px 0 0 15px;
  color: var(--van-text-color-2);
  font-size: 12px;
  line-height: 1.5;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.draft-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 9px 0 0 15px;
}

.draft-source {
  padding: 1px 5px;
  border-radius: 3px;
  background: color-mix(in srgb, #ec7047 11%, transparent);
  color: #d5613f;
  font-size: 10px;
}

.draft-time {
  margin-left: auto;
  color: var(--van-text-color-3, var(--van-text-color-2));
  font-size: 10px;
  font-variant-numeric: tabular-nums;
}
</style>
