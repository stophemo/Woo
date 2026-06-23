<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { showToast } from 'vant'
import { listOrphans } from '../../src/services/documentApi'
import type { DocumentDTO } from '../../src/services/documentApi'

const router = useRouter()
const drafts = ref<DocumentDTO[]>([])
const loading = ref(true)

onMounted(async () => {
  try {
    drafts.value = await listOrphans()
  } catch {
    showToast('加载草稿失败')
  } finally {
    loading.value = false
  }
})

function openNote(id: string) {
  router.push(`/note/${id}`)
}
</script>

<template>
  <div class="drafts-page">
    <van-cell-group title="未分类的笔记">
      <van-cell
        v-for="d in drafts"
        :key="d.id"
        :title="d.title || '无标题'"
        :label="d.updateTime"
        is-link
        @click="openNote(d.id)"
      />
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
</style>
