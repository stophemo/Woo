<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { showToast, showConfirmDialog } from 'vant'
import { useWorkspaceStore } from '../../src/stores/workspace'

const router = useRouter()
const store = useWorkspaceStore()
const loading = ref(true)

async function load() {
  loading.value = true
  try {
    await store.openTrashBox()
  } catch {
    showToast('加载回收站失败')
  } finally {
    loading.value = false
  }
}
onMounted(load)

async function onRestore(d: { id: string }) {
  await store.restoreDocument(d.id)
  showToast('已恢复')
  await load()
}

async function onHardDelete(d: { id: string; title?: string }) {
  try {
    await showConfirmDialog({
      title: '彻底删除',
      message: `「${d.title || '无标题'}」将永久删除，不可恢复。`,
    })
    await store.hardDeleteDocument(d.id)
    showToast('已彻底删除')
    await load()
  } catch { /* 取消 */ }
}

function formatTime(iso: string): string {
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return ''
  const now = new Date()
  if (date.toDateString() === now.toDateString()) {
    return date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
  }
  return date.toLocaleDateString('zh-CN', date.getFullYear() === now.getFullYear()
    ? { month: 'numeric', day: 'numeric' }
    : { year: 'numeric', month: 'numeric', day: 'numeric' })
}

async function onEmpty() {
  if (!store.currentFolderDocuments.length) return
  try {
    await showConfirmDialog({
      title: '清空回收站',
      message: '将永久删除回收站内所有内容，不可恢复。',
    })
    await store.emptyTrash()
    showToast('已清空')
    await load()
  } catch { /* 取消 */ }
}
</script>

<template>
  <div class="trash-page">
    <van-nav-bar title="回收站" left-text="返回" left-arrow @click-left="router.back()">
      <template #right>
        <span class="nav-text" @click="onEmpty">清空</span>
      </template>
    </van-nav-bar>

    <van-loading v-if="loading" class="mobile-loading" />

    <template v-else-if="store.currentFolderDocuments.length">
      <p class="hint">左滑可恢复或彻底删除</p>
      <ul class="w-group">
        <li v-for="d in store.currentFolderDocuments" :key="d.id">
          <van-swipe-cell>
            <div class="w-row">
              <span class="w-row__icon"><van-icon name="delete-o" /></span>
              <span class="w-row__main">
                <span class="w-row__title">{{ d.title || '无标题' }}</span>
                <span class="w-row__sub">{{ store.getDocumentPreview(d) || '暂无正文' }}</span>
              </span>
              <span class="w-row__side">
                <time>{{ formatTime(d.updatedAt) }}</time>
                <span v-if="d.folderName" class="src">{{ d.folderName }}</span>
              </span>
            </div>
            <template #right>
              <button type="button" class="swipe-btn" @click="onRestore(d)">恢复</button>
              <button type="button" class="swipe-btn is-danger" @click="onHardDelete(d)">删除</button>
            </template>
          </van-swipe-cell>
        </li>
      </ul>
    </template>

    <van-empty v-else description="回收站为空" image-size="72" />
  </div>
</template>

<style scoped>
.trash-page {
  min-height: 100vh;
  min-height: 100dvh;
  background: var(--w-paper);
}

.nav-text {
  color: var(--w-accent);
  font-size: 14px;
}

.hint {
  margin: 14px 20px 8px;
  color: var(--w-ink-3);
  font-size: 11.5px;
  letter-spacing: 0.02em;
}

.w-group {
  margin: 0 12px 28px;
}

.src {
  color: var(--w-ink-3);
  font-size: 10px;
}

.swipe-btn {
  height: 100%;
  padding: 0 18px;
  border: 0;
  background: var(--w-accent);
  color: #fff;
  font-size: 14px;
}

.swipe-btn.is-danger {
  background: var(--van-danger-color);
}
</style>
