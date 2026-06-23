<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { showToast } from 'vant'
import { getTree } from '../../src/services/folderApi'
import { listByFolder, listAll, create as createDoc } from '../../src/services/documentApi'
import type { FolderTreeNodeDTO } from '../../src/services/folderApi'
import type { DocumentDTO } from '../../src/services/documentApi'

const router = useRouter()
const folders = ref<FolderTreeNodeDTO[]>([])
const documents = ref<DocumentDTO[]>([])
const loading = ref(false)
const currentFolderId = ref<string | null>(null)
const showCreate = ref(false)
const newTitle = ref('')

onMounted(async () => {
  await loadFolders()
  await loadAllDocs()
})

async function loadFolders() {
  try {
    folders.value = await getTree()
  } catch {
    showToast('加载文件夹失败')
  }
}

async function loadAllDocs() {
  loading.value = true
  try {
    documents.value = await listAll()
  } catch {
    // ignore
  } finally {
    loading.value = false
  }
}

async function onFolderClick(folderId: string) {
  currentFolderId.value = folderId
  loading.value = true
  try {
    documents.value = await listByFolder(folderId)
  } catch {
    showToast('加载文档失败')
  } finally {
    loading.value = false
  }
}

function showAll() {
  currentFolderId.value = null
  loadAllDocs()
}

function openNote(id: string) {
  router.push(`/note/${id}`)
}

async function createDocument() {
  if (!newTitle.value.trim() || !currentFolderId.value) return
  try {
    await createDoc({ title: newTitle.value.trim(), folderId: currentFolderId.value })
    newTitle.value = ''
    showCreate.value = false
    await onFolderClick(currentFolderId.value)
    showToast('创建成功')
  } catch {
    showToast('创建失败')
  }
}

function formatTime(ts: string) {
  const d = new Date(ts)
  const now = new Date()
  const diff = now.getTime() - d.getTime()
  if (diff < 86400000 && d.getDate() === now.getDate()) {
    return d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
  }
  return d.toLocaleDateString('zh-CN')
}

function stripHtml(html: string) {
  return html.replace(/<[^>]+>/g, '').slice(0, 100)
}
</script>

<template>
  <div class="home-page">
    <!-- 顶部搜索栏 -->
    <van-search
      placeholder="搜索笔记..."
      readonly
      @click="showToast('搜索功能开发中')"
    />

    <!-- 文件夹横向滚动 -->
    <van-tabs
      v-if="folders.length"
      sticky
      offset-top="54"
      @change="(i: number) => folders[i] ? onFolderClick(folders[i].id) : showAll()"
    >
      <van-tab title="全部" @click="showAll" />
      <van-tab v-for="f in folders" :key="f.id" :title="f.name" />
    </van-tabs>

    <!-- 新建文档按钮 -->
    <van-button
      v-if="currentFolderId"
      type="primary"
      size="small"
      plain
      icon="plus"
      class="new-btn"
      @click="showCreate = true"
    >
      新建笔记
    </van-button>

    <!-- 文档列表 -->
    <div class="doc-list">
      <van-pull-refresh v-model="loading" @refresh="showAll">
        <van-cell
          v-for="d in documents"
          :key="d.id"
          @click="openNote(d.id)"
        >
          <template #title>
            <span class="doc-title">{{ d.title || '无标题' }}</span>
          </template>
          <template #label>
            <span class="doc-preview">{{ stripHtml(d.content ?? '') }}</span>
            <span class="doc-time">{{ formatTime(d.updateTime) }}</span>
          </template>
        </van-cell>

        <van-empty v-if="!documents.length && !loading" description="暂无笔记" />
      </van-pull-refresh>
    </div>

    <!-- 新建文档弹窗 -->
    <van-dialog
      v-model:show="showCreate"
      title="新建笔记"
      show-cancel-button
      @confirm="createDocument"
    >
      <van-field
        v-model="newTitle"
        placeholder="输入笔记标题"
        autofocus
        maxlength="200"
        clearable
      />
    </van-dialog>
  </div>
</template>

<style scoped>
.new-btn {
  margin: 8px 16px;
}
.doc-list {
  padding-bottom: 50px;
}
.doc-title {
  font-weight: 500;
}
.doc-preview {
  color: #999;
  font-size: 12px;
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.doc-time {
  color: #ccc;
  font-size: 11px;
}
</style>
