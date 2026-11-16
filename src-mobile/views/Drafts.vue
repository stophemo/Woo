<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { showToast } from 'vant'
import { useWorkspaceStore } from '../../src/stores/workspace'
import type { Document } from '../../src/types/document'
import FolderPicker from '../components/FolderPicker.vue'
import MIcon from '../components/MIcon.vue'

const router = useRouter()
const store = useWorkspaceStore()
const loading = ref(true)
const showFolderPicker = ref(false)
const activeDraft = ref<Document | null>(null)

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

function formatTime(ts: string): string {
  const date = new Date(ts)
  if (Number.isNaN(date.getTime())) return ''
  const now = new Date()
  if (date.toDateString() === now.toDateString()) {
    return date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
  }
  return date.toLocaleDateString('zh-CN', date.getFullYear() === now.getFullYear()
    ? { month: 'numeric', day: 'numeric' }
    : { year: 'numeric', month: 'numeric', day: 'numeric' })
}

function openPicker(draft: Document) {
  activeDraft.value = draft
  showFolderPicker.value = true
}

async function onFolderPicked(folderId: string) {
  const draft = activeDraft.value
  if (!draft) return
  const moved = await store.moveDocument(draft.id, folderId)
  if (!moved) {
    showToast(store.error || '移动失败')
    return
  }
  showToast('已归档到目录')
  await load()
}

async function load() {
  try {
    await store.openDraftBox()
  } catch {
    showToast('加载草稿失败')
  }
}
</script>

<template>
  <div class="drafts-page">
    <header class="w-head">
      <div class="w-head__row">
        <div class="w-head__titles">
          <h1 class="w-head__title">草稿箱</h1>
        </div>
        <button type="button" class="w-icon-btn" aria-label="新建草稿" @click="newDraft">
          <van-icon name="plus" />
        </button>
      </div>
      <div class="w-head__sub">
        <p class="w-head__meta">
          {{ loading ? '正在读取…' : `${drafts.length} 篇未归档文稿` }}
        </p>
      </div>
    </header>

    <van-loading v-if="loading" class="mobile-loading" />

    <section v-else-if="drafts.length" class="w-section">
      <ul class="w-group">
        <li v-for="d in drafts" :key="d.id">
          <button type="button" class="w-row draft-row" @click="openNote(d.id)">
            <span class="w-row__icon"><MIcon name="pencil" :size="17" /></span>
            <span class="w-row__main">
              <span class="w-row__title">{{ d.title || '无标题' }}</span>
              <span class="w-row__sub">{{ store.getDocumentPreview(d) || '暂无正文' }}</span>
            </span>
            <span class="w-row__side">
              <time>{{ formatTime(d.updatedAt) }}</time>
            </span>
          </button>
          <button
            type="button"
            class="row-action"
            title="归档到目录"
            aria-label="归档到目录"
            @click.stop="openPicker(d)"
          >
            <MIcon name="folder" :size="17" />
          </button>
        </li>
      </ul>
    </section>

    <van-empty v-else description="暂无草稿" image-size="72" />

    <FolderPicker
      v-model:show="showFolderPicker"
      title="归档到目录"
      @select="onFolderPicked"
    />
  </div>
</template>

<style scoped>
.drafts-page {
  min-height: 100vh;
  min-height: 100dvh;
  background: var(--w-paper);
}

.draft-row {
  padding-right: 42px;
}

.row-action {
  position: absolute;
  top: 50%;
  right: 8px;
  display: inline-flex;
  width: 30px;
  height: 34px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--w-ink-3);
  font-size: 18px;
  transform: translateY(-50%);
}

.row-action:active {
  background: var(--w-rise);
  color: var(--w-accent);
}
</style>
