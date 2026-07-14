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

function formatTime(ts: string) {
  const d = new Date(ts)
  return d.toLocaleString('zh-CN', { hour12: false })
}
</script>

<template>
  <div class="drafts-page">
    <van-cell-group title="草稿箱">
      <van-cell
        v-for="d in drafts"
        :key="d.id"
        :title="d.title || '无标题'"
        is-link
        @click="openNote(d.id)"
      >
        <template #label>
          <span class="draft-source">{{ d.folderName || '草稿箱' }}</span>
          <span class="draft-time">{{ formatTime(d.updatedAt) }}</span>
        </template>
      </van-cell>
      <van-empty v-if="!drafts.length && !loading" description="暂无草稿" />
    </van-cell-group>
    <van-loading v-if="loading" class="loading" />
  </div>
</template>

<style scoped>
.loading {
  display: flex;
  justify-content: center;
  margin-top: 40px;
}
.draft-source {
  color: #1989fa;
  font-size: 11px;
  margin-right: 8px;
}
.draft-time {
  color: #ccc;
  font-size: 11px;
}
</style>
