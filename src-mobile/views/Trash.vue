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

    <van-loading v-if="loading" class="loading" />

    <van-swipe-cell v-for="d in store.currentFolderDocuments" :key="d.id">
      <van-cell :title="d.title || '无标题'">
        <template #label>
          <span class="preview">{{ store.getDocumentPreview(d) }}</span>
          <span class="src">{{ d.folderName }}</span>
        </template>
      </van-cell>
      <template #right>
        <van-button square type="primary" text="恢复" class="sw" @click="onRestore(d)" />
        <van-button square type="danger" text="彻底删除" class="sw" @click="onHardDelete(d)" />
      </template>
    </van-swipe-cell>

    <van-empty v-if="!loading && !store.currentFolderDocuments.length" description="回收站为空" />
  </div>
</template>

<style scoped>
.loading {
  display: flex;
  justify-content: center;
  margin-top: 40px;
}
.empty-btn {
  color: #ee0a24;
  font-size: 14px;
}
.preview {
  color: #999;
  font-size: 12px;
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.src {
  color: #c8c9cc;
  font-size: 11px;
}
.sw {
  height: 100%;
}
</style>
