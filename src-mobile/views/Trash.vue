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
        <span class="empty-btn" @click="onEmpty">清空</span>
      </template>
    </van-nav-bar>

    <van-loading v-if="loading" class="mobile-loading" />

    <div v-else-if="store.currentFolderDocuments.length" class="trash-list">
      <van-swipe-cell v-for="d in store.currentFolderDocuments" :key="d.id" class="trash-swipe-cell">
        <van-cell class="trash-cell">
          <template #title>
            <div class="trash-heading">
              <van-icon name="delete-o" />
              <span>{{ d.title || '无标题' }}</span>
            </div>
          </template>
          <template #label>
            <span class="preview">{{ store.getDocumentPreview(d) || '暂无正文' }}</span>
            <span v-if="d.folderName" class="src">原位置 · {{ d.folderName }}</span>
          </template>
        </van-cell>
        <template #right>
          <van-button square type="primary" text="恢复" class="sw" @click="onRestore(d)" />
          <van-button square type="danger" text="彻底删除" class="sw" @click="onHardDelete(d)" />
        </template>
      </van-swipe-cell>
    </div>

    <van-empty v-else description="回收站为空" />
  </div>
</template>

<style scoped>
.trash-page {
  min-height: 100vh;
  min-height: 100dvh;
  background: var(--van-background);
}

.empty-btn {
  color: var(--van-danger-color);
  font-size: 13px;
}

.trash-list {
  padding: 12px;
}

.trash-swipe-cell {
  margin-bottom: 8px;
  overflow: hidden;
  border: 1px solid var(--van-border-color);
  border-radius: 7px;
  background: var(--van-background-2);
}

.trash-cell {
  min-height: 106px;
  padding: 14px 15px;
}

.trash-cell::after {
  display: none;
}

.trash-heading {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 8px;
  color: var(--van-text-color);
  font-size: 15px;
  font-weight: 650;
}

.trash-heading .van-icon {
  flex-shrink: 0;
  color: var(--van-text-color-3, var(--van-text-color-2));
}

.trash-heading span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.preview {
  display: block;
  overflow: hidden;
  margin: 7px 0 0 23px;
  color: var(--van-text-color-2);
  font-size: 12px;
  line-height: 1.5;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.src {
  display: block;
  margin: 8px 0 0 23px;
  color: var(--van-text-color-3, var(--van-text-color-2));
  font-size: 10px;
}

.sw {
  height: 100%;
}
</style>
