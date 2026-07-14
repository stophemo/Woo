<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { showToast, showConfirmDialog } from 'vant'
import { useWorkspaceStore } from '../../src/stores/workspace'

const router = useRouter()
const store = useWorkspaceStore()

// 0 = 全部；>=1 对应 store.folders[activeTab-1]（移动端只做根目录，扁平 tab）
const activeTab = ref(0)
const refreshing = ref(false)
const showCreateDoc = ref(false)
const newTitle = ref('')

// 文件夹管理
const showFolderSheet = ref(false)
const showNameDialog = ref(false)
const nameMode = ref<'newRoot' | 'rename'>('newRoot')
const nameInput = ref('')

const topFolders = computed(() => store.folders)
const documents = computed(() => store.currentFolderDocuments)
const currentRealFolderId = computed<string | null>(() => {
  const f = activeTab.value >= 1 ? topFolders.value[activeTab.value - 1] : null
  return f ? f.id : null
})
const currentFolderName = computed(() => {
  const f = activeTab.value >= 1 ? topFolders.value[activeTab.value - 1] : null
  return f ? f.name : ''
})

const folderActions = computed(() => {
  const acts: { name: string; key: string; color?: string }[] = [
    { name: '新建文件夹', key: 'newRoot' },
  ]
  if (currentRealFolderId.value) {
    acts.push({ name: `重命名「${currentFolderName.value}」`, key: 'rename' })
    acts.push({ name: `删除「${currentFolderName.value}」`, key: 'delete', color: '#ee0a24' })
  }
  return acts
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
  showCreateDoc.value = false
  if (doc) {
    showToast('创建成功')
    router.push(`/note/${doc.id}`)
  } else {
    showToast('创建失败')
  }
}

async function onDeleteDoc(d: { id: string; title?: string }) {
  try {
    await showConfirmDialog({ title: '删除笔记', message: `确定删除「${d.title || '无标题'}」？` })
    await store.deleteDocument(d.id)
    showToast('已移入回收站')
  } catch { /* 取消 */ }
}

// ------- 文件夹管理 -------
function currentFolderNode() {
  const id = currentRealFolderId.value
  return id ? store.findFolderById(store.folders, id) : null
}

function onFolderAction(action: { key: string }) {
  showFolderSheet.value = false
  if (action.key === 'newRoot') {
    nameMode.value = 'newRoot'
    nameInput.value = ''
    showNameDialog.value = true
  } else if (action.key === 'rename') {
    nameMode.value = 'rename'
    nameInput.value = currentFolderName.value
    showNameDialog.value = true
  } else if (action.key === 'delete') {
    void deleteCurrentFolder()
  }
}

async function submitName() {
  const name = nameInput.value.trim()
  if (!name) return
  if (nameMode.value === 'newRoot') {
    await store.createRootFolder() // 建“新建目录”并置 editingFolderId
    const node = store.findFolderById(store.folders, store.editingFolderId as string)
    if (node) await store.renameFolder(node, name)
    store.clearEditingFolder()
    showToast('已创建')
    // 切到新建的文件夹
    const idx = topFolders.value.findIndex((f) => f.id === node?.id)
    if (idx >= 0) await onTabChange(idx + 1)
  } else if (nameMode.value === 'rename') {
    const node = currentFolderNode()
    if (node) await store.renameFolder(node, name)
    showToast('已重命名')
  }
}

async function deleteCurrentFolder() {
  const node = currentFolderNode()
  if (!node) return
  try {
    await showConfirmDialog({
      title: '删除文件夹',
      message: `确定删除「${node.name}」？其中的笔记将移入回收站。`,
    })
    await store.deleteFolder(node)
    activeTab.value = 0
    await store.openAllDocuments()
    showToast('已删除')
  } catch { /* 取消 */ }
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
    <van-nav-bar title="笔记">
      <template #right>
        <van-icon name="search" class="nav-icon" @click="router.push('/search')" />
        <van-icon name="more-o" class="nav-icon" @click="showFolderSheet = true" />
      </template>
    </van-nav-bar>

    <!-- 文件夹横向 tab -->
    <van-tabs v-model:active="activeTab" sticky @change="onTabChange">
      <van-tab title="全部" />
      <van-tab v-for="f in topFolders" :key="f.id" :title="f.name" />
    </van-tabs>

    <!-- 真实目录内可新建笔记 -->
    <van-button
      v-if="currentRealFolderId"
      type="primary"
      size="small"
      plain
      icon="plus"
      class="new-btn"
      @click="showCreateDoc = true"
    >
      新建笔记
    </van-button>

    <!-- 文档列表（滑动删除） -->
    <div class="doc-list">
      <van-pull-refresh v-model="refreshing" @refresh="onRefresh">
        <van-swipe-cell v-for="d in documents" :key="d.id">
          <van-cell @click="openNote(d.id)">
            <template #title>
              <span class="doc-title">{{ d.title || '无标题' }}</span>
            </template>
            <template #label>
              <span class="doc-preview">{{ store.getDocumentPreview(d) }}</span>
              <span class="doc-time">{{ formatTime(d.updatedAt) }}</span>
            </template>
          </van-cell>
          <template #right>
            <van-button square type="danger" text="删除" class="swipe-del" @click="onDeleteDoc(d)" />
          </template>
        </van-swipe-cell>

        <template v-if="!documents.length && !store.loading">
          <van-empty description="暂无笔记">
            <van-button
              v-if="!topFolders.length"
              round
              type="primary"
              icon="plus"
              @click="onFolderAction({ key: 'newRoot' })"
            >
              新建文件夹
            </van-button>
          </van-empty>
        </template>
      </van-pull-refresh>
    </div>

    <!-- 文件夹管理 action sheet -->
    <van-action-sheet
      v-model:show="showFolderSheet"
      :actions="folderActions"
      cancel-text="取消"
      close-on-click-action
      @select="onFolderAction"
    />

    <!-- 文件夹命名对话框 -->
    <van-dialog
      v-model:show="showNameDialog"
      :title="nameMode === 'newRoot' ? '新建文件夹' : '重命名文件夹'"
      show-cancel-button
      @confirm="submitName"
    >
      <van-field v-model="nameInput" placeholder="文件夹名称" autofocus maxlength="50" clearable />
    </van-dialog>

    <!-- 新建文档对话框 -->
    <van-dialog
      v-model:show="showCreateDoc"
      title="新建笔记"
      show-cancel-button
      @confirm="createDocument"
    >
      <van-field v-model="newTitle" placeholder="输入笔记标题" autofocus maxlength="200" clearable />
    </van-dialog>
  </div>
</template>

<style scoped>
.nav-icon {
  font-size: 20px;
  margin-left: 16px;
}
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
.swipe-del {
  height: 100%;
}
</style>
