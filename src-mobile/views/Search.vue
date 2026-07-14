<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useWorkspaceStore } from '../../src/stores/workspace'

const router = useRouter()
const store = useWorkspaceStore()

const keyword = ref('')
const searched = ref(false)
let timer: ReturnType<typeof setTimeout> | null = null

function onInput() {
  if (timer) clearTimeout(timer)
  timer = setTimeout(runSearch, 300)
}

async function runSearch() {
  const q = keyword.value.trim()
  if (!q) {
    searched.value = false
    return
  }
  await store.openSearch(q)
  searched.value = true
}

function openNote(id: string) {
  router.push(`/note/${id}`)
}

function stripHtml(html: string) {
  return html.replace(/<[^>]+>/g, '').slice(0, 60)
}
</script>

<template>
  <div class="search-page">
    <van-nav-bar title="搜索" left-text="返回" left-arrow @click-left="router.back()" />
    <van-search
      v-model="keyword"
      placeholder="搜索标题或正文…"
      show-action
      autofocus
      @update:model-value="onInput"
      @search="runSearch"
      @cancel="router.back()"
    />

    <van-cell
      v-for="d in store.currentFolderDocuments"
      :key="d.id"
      :title="d.title || '无标题'"
      :label="stripHtml(d.content || '')"
      is-link
      @click="openNote(d.id)"
    />

    <van-empty
      v-if="searched && !store.currentFolderDocuments.length"
      description="没有匹配的笔记"
    />
    <div v-if="!searched" class="hint">输入关键词开始搜索</div>
  </div>
</template>

<style scoped>
.hint {
  text-align: center;
  color: #969799;
  margin-top: 60px;
  font-size: 14px;
}
</style>
