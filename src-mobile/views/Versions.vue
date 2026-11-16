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
        <span class="nav-text" @click="onSaveManual">存当前</span>
      </template>
    </van-nav-bar>

    <van-loading v-if="loading" class="mobile-loading" />

    <template v-else-if="versions.length">
      <p class="hint">点击任意版本即可恢复</p>
      <ul class="w-group">
        <li v-for="v in versions" :key="v.id">
          <button type="button" class="w-row" @click="onRestore(v)">
            <span class="w-row__main">
              <span class="w-row__title">
                <span class="v-no">v{{ v.versionNo }}</span>
                <span class="v-type">{{ typeLabel[v.changeType] || v.changeType }}</span>
              </span>
              <span class="w-row__sub">{{ v.preview || '（空白版本）' }}</span>
            </span>
            <span class="w-row__side">
              <time>{{ formatTime(v.createTime) }}</time>
            </span>
            <van-icon name="arrow" class="w-row__chev" />
          </button>
        </li>
      </ul>
    </template>

    <van-empty v-else description="暂无历史版本" image-size="72" />
  </div>
</template>

<style scoped>
.versions-page {
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
}

.w-group {
  margin: 0 12px 28px;
}

.v-no {
  color: var(--w-accent);
  font-size: 15px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.v-type {
  color: var(--w-ink-3);
  font-size: 10.5px;
  font-weight: 500;
  letter-spacing: 0.06em;
}
</style>
