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

</script>

<template>
  <div class="search-page">
    <van-nav-bar title="搜索" left-arrow @click-left="router.back()" />

    <div class="search-field">
      <van-search
        v-model="keyword"
        placeholder="搜索标题或正文"
        autofocus
        @update:model-value="onInput"
        @search="runSearch"
      />
    </div>

    <template v-if="searched">
      <p class="hint">找到 {{ store.currentFolderDocuments.length }} 篇</p>
      <ul v-if="store.currentFolderDocuments.length" class="w-group">
        <li v-for="d in store.currentFolderDocuments" :key="d.id">
          <button type="button" class="w-row" @click="openNote(d.id)">
            <span class="w-row__icon"><van-icon name="search" /></span>
            <span class="w-row__main">
              <span class="w-row__title">{{ d.title || '无标题' }}</span>
              <span class="w-row__sub">{{ store.getDocumentPreview(d) || '暂无正文' }}</span>
            </span>
            <van-icon name="arrow" class="w-row__chev" />
          </button>
        </li>
      </ul>
      <van-empty v-else description="没有匹配的笔记" image-size="72" />
    </template>

    <p v-else class="hint">输入关键词开始搜索</p>
  </div>
</template>

<style scoped>
.search-page {
  min-height: 100vh;
  min-height: 100dvh;
  background: var(--w-paper);
}

.search-field {
  padding: 10px 8px 2px;
}

.hint {
  margin: 14px 20px 8px;
  color: var(--w-ink-3);
  font-size: 11.5px;
}

.w-group {
  margin: 0 12px calc(28px + env(safe-area-inset-bottom, 0px));
}
</style>
