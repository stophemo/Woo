<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { showToast } from 'vant'
import { useWorkspaceStore } from '../../src/stores/workspace'

const router = useRouter()
const store = useWorkspaceStore()

// 0 = 全部；>=1 对应 store.folders[activeTab-1]
const activeTab = ref(0)
const refreshing = ref(false)
const showCreate = ref(false)
const newTitle = ref('')

const topFolders = computed(() => store.folders)
const documents = computed(() => store.currentFolderDocuments)
// 当前是否处于真实目录（草稿/全部视图不可直接新建到目录）
const currentRealFolderId = computed<string | null>(() => {
  const f = activeTab.value >= 1 ? topFolders.value[activeTab.value - 1] : null
  return f ? f.id : null
})

onMounted(async () => {
  try {
    await store.bootstrap()
  } catch {
    showToast('加载文件夹失败')
  }
  await store.openAllDocuments()
})

async function onTabChange(index: number) {
  activeTab.value = index
  if (index === 0) {
    await store.openAllDocuments()
  } else {
    const folder = topFolders.value[index - 1]
    if (folder) await store.selectFolder(folder.id)
  }
}

async function onRefresh() {
  try {
    await onTabChange(activeTab.value)
  } finally {
    refreshing.value = false
  }
}

function openNote(id: string) {
  router.push(`/note/${id}`)
}

async function createDocument() {
  const fid = currentRealFolderId.value
  if (!newTitle.value.trim() || !fid) return
  const doc = await store.createDocument(fid, newTitle.value.trim())
  newTitle.value = ''
  showCreate.value = false
  if (doc) {
    showToast('创建成功')
    router.push(`/note/${doc.id}`)
  } else {
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
</script>

<template>
  <div class="home-page">
    <!-- 顶部搜索栏（搜索为子项目 3） -->
    <van-search
      placeholder="搜索笔记..."
      readonly
      @click="showToast('搜索功能开发中')"
    />

    <!-- 文件夹横向滚动 -->
    <van-tabs
      v-model:active="activeTab"
      sticky
      offset-top="54"
      @change="onTabChange"
    >
      <van-tab title="全部" />
      <van-tab v-for="f in topFolders" :key="f.id" :title="f.name" />
    </van-tabs>

    <!-- 新建文档按钮 -->
    <van-button
      v-if="currentRealFolderId"
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
      <van-pull-refresh v-model="refreshing" @refresh="onRefresh">
        <van-cell
          v-for="d in documents"
          :key="d.id"
          @click="openNote(d.id)"
        >
          <template #title>
            <span class="doc-title">{{ d.title || '无标题' }}</span>
          </template>
          <template #label>
            <span class="doc-preview">{{ store.getDocumentPreview(d) }}</span>
            <span class="doc-time">{{ formatTime(d.updatedAt) }}</span>
          </template>
        </van-cell>

        <van-empty v-if="!documents.length && !store.loading" description="暂无笔记" />
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
