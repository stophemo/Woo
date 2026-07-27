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

    <van-loading v-if="loading" class="mobile-loading" />

    <div v-else-if="versions.length" class="version-list">
      <button
        v-for="v in versions"
        :key="v.id"
        type="button"
        class="version-item"
        @click="onRestore(v)"
      >
        <span class="version-marker" aria-hidden="true"></span>
        <span class="version-body">
          <span class="version-heading">
            <strong class="v-no">v{{ v.versionNo }}</strong>
            <van-tag :type="v.changeType === 'manual' ? 'primary' : v.changeType === 'restore' ? 'success' : 'default'" class="v-tag">
              {{ typeLabel[v.changeType] || v.changeType }}
            </van-tag>
            <time class="v-time">{{ formatTime(v.createTime) }}</time>
          </span>
          <span class="v-preview">{{ v.preview || '（空白版本）' }}</span>
        </span>
        <van-icon name="arrow" class="version-arrow" />
      </button>
    </div>

    <van-empty v-else description="暂无历史版本" />
  </div>
</template>

<style scoped>
.versions-page {
  min-height: 100vh;
  min-height: 100dvh;
  background: var(--van-background);
}

.save-btn {
  color: var(--van-primary-color);
  font-size: 13px;
  font-weight: 600;
}

.version-list {
  position: relative;
  padding: 14px 12px 24px;
}

.version-list::before {
  position: absolute;
  top: 34px;
  bottom: 44px;
  left: 22px;
  width: 1px;
  background: var(--van-border-color);
  content: '';
}

.version-item {
  position: relative;
  display: flex;
  width: 100%;
  min-height: 92px;
  align-items: flex-start;
  gap: 11px;
  margin: 0 0 8px;
  padding: 14px 13px 13px 9px;
  text-align: left;
  border: 1px solid var(--van-border-color);
  border-radius: 7px;
  background: var(--van-background-2);
  color: var(--van-text-color);
}

.version-item:active {
  background: color-mix(in srgb, var(--van-primary-color) 5%, var(--van-background-2));
}

.version-marker {
  z-index: 1;
  width: 7px;
  height: 7px;
  flex: 0 0 7px;
  margin-top: 6px;
  border: 2px solid var(--van-background-2);
  border-radius: 50%;
  background: var(--van-primary-color);
  box-shadow: 0 0 0 1px var(--van-primary-color);
}

.version-body {
  display: block;
  min-width: 0;
  flex: 1;
}

.version-heading {
  display: flex;
  align-items: center;
  gap: 7px;
}

.v-no {
  color: var(--van-text-color);
  font-size: 14px;
  font-weight: 700;
}

.v-tag {
  vertical-align: middle;
}

.v-preview {
  display: -webkit-box;
  overflow: hidden;
  margin-top: 9px;
  color: var(--van-text-color-2);
  font-size: 12px;
  line-height: 1.5;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.v-time {
  margin-left: auto;
  color: var(--van-text-color-3, var(--van-text-color-2));
  font-size: 9px;
  font-variant-numeric: tabular-nums;
}

.version-arrow {
  align-self: center;
  color: var(--van-text-color-3, var(--van-text-color-2));
  font-size: 13px;
}
</style>
