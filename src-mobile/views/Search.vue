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
    <div class="search-field-wrap">
      <van-search
        v-model="keyword"
        placeholder="搜索标题或正文…"
        show-action
        autofocus
        @update:model-value="onInput"
        @search="runSearch"
        @cancel="router.back()"
      />
    </div>

    <div v-if="searched && store.currentFolderDocuments.length" class="search-results">
      <p class="result-count">找到 {{ store.currentFolderDocuments.length }} 篇文稿</p>
      <van-cell
        v-for="d in store.currentFolderDocuments"
        :key="d.id"
        class="result-cell"
        clickable
        @click="openNote(d.id)"
      >
        <template #title>
          <div class="result-heading">
            <span class="result-dot" aria-hidden="true"></span>
            <span>{{ d.title || '无标题' }}</span>
          </div>
        </template>
        <template #label>
          <span class="result-preview">{{ stripHtml(d.content || '') || '暂无正文' }}</span>
        </template>
      </van-cell>
    </div>

    <van-empty
      v-if="searched && !store.currentFolderDocuments.length"
      description="没有匹配的笔记"
    />
    <div v-if="!searched" class="hint">
      <van-icon name="search" />
      <span>输入关键词开始搜索</span>
    </div>
  </div>
</template>

<style scoped>
.search-page {
  min-height: 100vh;
  min-height: 100dvh;
  background: var(--van-background);
}

.search-field-wrap {
  padding: 10px 12px;
  border-bottom: 1px solid var(--van-border-color);
  background: var(--van-background-2);
}

.search-field-wrap :deep(.van-search) {
  padding: 0;
}

.search-field-wrap :deep(.van-search__content) {
  border: 1px solid var(--van-border-color);
  border-radius: 7px;
  background: color-mix(in srgb, var(--van-background) 75%, var(--van-background-2));
}

.search-results {
  padding: 0 12px 18px;
}

.result-count {
  margin: 0;
  padding: 15px 3px 9px;
  color: var(--van-text-color-2);
  font-size: 11px;
}

.result-cell {
  margin-bottom: 8px;
  padding: 14px 15px;
  border: 1px solid var(--van-border-color);
  border-radius: 7px;
}

.result-cell::after {
  display: none;
}

.result-heading {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 8px;
  color: var(--van-text-color);
  font-size: 15px;
  font-weight: 650;
}

.result-heading span:last-child {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.result-dot {
  width: 7px;
  height: 7px;
  flex: 0 0 7px;
  border-radius: 50%;
  background: var(--van-primary-color);
}

.result-preview {
  display: block;
  overflow: hidden;
  margin: 7px 0 0 15px;
  color: var(--van-text-color-2);
  font-size: 12px;
  line-height: 1.5;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.hint {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  text-align: center;
  color: var(--van-text-color-2);
  margin-top: 72px;
  font-size: 13px;
}

.hint .van-icon {
  font-size: 28px;
  opacity: 0.55;
}
</style>
