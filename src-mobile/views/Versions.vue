<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { showToast, showConfirmDialog } from 'vant'
import { listVersions, restoreVersion, saveManualVersion } from '../../src/services/versionApi'
import type { DocumentVersionSummary } from '../../src/services/versionApi'
import { useWorkspaceStore } from '../../src/stores/workspace'

const route = useRoute()
const router = useRouter()
const store = useWorkspaceStore()
const docId = route.params.id as string

const versions = ref<DocumentVersionSummary[]>([])
const loading = ref(true)

const typeLabel: Record<string, string> = { auto: '自动', manual: '手动', restore: '恢复' }

async function load() {
  loading.value = true
  try {
    versions.value = await listVersions(docId)
  } catch {
    showToast('加载版本失败')
  } finally {
    loading.value = false
  }
}
onMounted(load)

async function onSaveManual() {
  try {
    await saveManualVersion(docId)
    showToast('已保存当前版本')
    await load()
  } catch {
    showToast('保存版本失败')
  }
}

async function onRestore(v: DocumentVersionSummary) {
  try {
    await showConfirmDialog({
      title: '恢复此版本',
      message: `将当前正文恢复到 v${v.versionNo}（${typeLabel[v.changeType] || v.changeType}）。`,
    })
    await restoreVersion(docId, v.versionNo)
    await store.selectDocument(docId) // 刷新编辑页的 currentDocument
    showToast('已恢复')
    router.back()
  } catch { /* 取消 */ }
}

function formatTime(ts: string) {
  return new Date(ts).toLocaleString('zh-CN', { hour12: false })
}
</script>

<template>
  <div class="versions-page">
    <van-nav-bar title="版本历史" left-text="返回" left-arrow @click-left="router.back()">
      <template #right>
        <span class="save-btn" @click="onSaveManual">存当前</span>
      </template>
    </van-nav-bar>

    <van-loading v-if="loading" class="loading" />

    <van-cell
      v-for="v in versions"
      :key="v.id"
      is-link
      @click="onRestore(v)"
    >
      <template #title>
        <span class="v-no">v{{ v.versionNo }}</span>
        <van-tag :type="v.changeType === 'manual' ? 'primary' : v.changeType === 'restore' ? 'success' : 'default'" class="v-tag">
          {{ typeLabel[v.changeType] || v.changeType }}
        </van-tag>
      </template>
      <template #label>
        <span class="v-preview">{{ v.preview }}</span>
        <span class="v-time">{{ formatTime(v.createTime) }}</span>
      </template>
    </van-cell>

    <van-empty v-if="!loading && !versions.length" description="暂无历史版本" />
  </div>
</template>

<style scoped>
.loading {
  display: flex;
  justify-content: center;
  margin-top: 40px;
}
.save-btn {
  color: #1989fa;
  font-size: 14px;
}
.v-no {
  font-weight: 600;
  margin-right: 8px;
}
.v-tag {
  vertical-align: middle;
}
.v-preview {
  color: #999;
  font-size: 12px;
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.v-time {
  color: #c8c9cc;
  font-size: 11px;
}
</style>
