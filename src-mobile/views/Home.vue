<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { showToast, showConfirmDialog } from 'vant'
import { useWorkspaceStore } from '../../src/stores/workspace'
import { useLockStore } from '../../src/stores/lock'

const router = useRouter()
const store = useWorkspaceStore()
const lockStore = useLockStore()

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
const currentViewName = computed(() => currentFolderName.value || '全部文稿')

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

// ------- 加锁/解锁 -------
const showUnlock = ref(false)
const unlockPwd = ref('')
let unlockResolve: ((v: string | null) => void) | null = null

function promptPassword(): Promise<string | null> {
  unlockPwd.value = ''
  showUnlock.value = true
  return new Promise((r) => { unlockResolve = r })
}
function unlockConfirm() {
  showUnlock.value = false
  unlockResolve?.(unlockPwd.value)
  unlockResolve = null
}
function unlockCancel() {
  showUnlock.value = false
  unlockResolve?.(null)
  unlockResolve = null
}

async function onToggleLock(d: { id: string; isLocked?: boolean }) {
  if (d.isLocked) {
    const pw = await promptPassword()
    if (pw === null) return
    const ok = await lockStore.verify(pw)
    if (!ok) { showToast('密码错误'); return }
    await lockStore.unlockDocument(d.id)
    showToast('已解锁')
  } else {
    if (!lockStore.hasPassword) {
      showToast('请先在设置里设置密码锁')
      return
    }
    await lockStore.lockDocument(d.id)
    showToast('已加锁')
  }
  await onTabChange(activeTab.value)
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

const FOLDER_DOT_COLORS = ['#4597b7', '#3da57a', '#ec7047', '#d3a32c', '#9b78c6', '#6f8fa4']

function documentDotColor(folderId: string) {
  let hash = 0
  for (const char of folderId) {
    hash = ((hash << 5) - hash + char.charCodeAt(0)) | 0
  }
  return FOLDER_DOT_COLORS[Math.abs(hash) % FOLDER_DOT_COLORS.length]
}
</script>

<template>
  <div class="home-page">
    <header class="mobile-page-header">
      <div class="mobile-page-header__main">
        <h1 class="mobile-page-header__title">文稿</h1>
        <p class="mobile-page-header__meta">{{ currentViewName }} · {{ documents.length }} 篇</p>
      </div>
      <div class="mobile-page-header__actions">
        <button
          type="button"
          class="mobile-icon-button"
          title="搜索"
          aria-label="搜索"
          @click="router.push('/search')"
        >
          <van-icon name="search" />
        </button>
        <button
          v-if="currentRealFolderId"
          type="button"
          class="mobile-icon-button is-primary"
          title="新建文稿"
          aria-label="新建文稿"
          @click="showCreateDoc = true"
        >
          <van-icon name="plus" />
        </button>
        <button
          type="button"
          class="mobile-icon-button"
          title="管理文件夹"
          aria-label="管理文件夹"
          @click="showFolderSheet = true"
        >
          <van-icon name="more-o" />
        </button>
      </div>
    </header>

    <!-- 文件夹横向 tab -->
    <div class="folder-strip">
      <van-tabs v-model:active="activeTab" shrink @change="onTabChange">
        <van-tab title="全部" />
        <van-tab v-for="f in topFolders" :key="f.id" :title="f.name" />
      </van-tabs>
    </div>

    <!-- 文档列表（滑动删除） -->
    <div class="doc-list">
      <van-pull-refresh v-model="refreshing" @refresh="onRefresh">
        <van-swipe-cell v-for="d in documents" :key="d.id" class="doc-swipe-cell">
          <van-cell class="doc-cell" clickable @click="openNote(d.id)">
            <template #title>
              <div class="doc-heading">
                <span class="doc-dot" :style="{ backgroundColor: documentDotColor(d.folderId) }" aria-hidden="true"></span>
                <span class="doc-title">{{ d.isLocked ? '已锁定' : (d.title || '无标题') }}</span>
                <van-icon v-if="d.isLocked" name="lock" class="lock-ic" />
              </div>
            </template>
            <template #label>
              <span v-if="!d.isLocked" class="doc-preview">{{ store.getDocumentPreview(d) || '暂无正文' }}</span>
              <span class="doc-meta">
                <span v-if="d.folderName" class="doc-folder">{{ d.folderName }}</span>
                <time class="doc-time">{{ formatTime(d.updatedAt) }}</time>
              </span>
            </template>
          </van-cell>
          <template #right>
            <van-button square type="warning" :text="d.isLocked ? '解锁' : '加锁'" class="swipe-lock" @click="onToggleLock(d)" />
            <van-button square type="danger" text="删除" class="swipe-del" @click="onDeleteDoc(d)" />
          </template>
        </van-swipe-cell>

        <template v-if="!documents.length && !store.loading">
          <van-empty description="暂无笔记">
            <van-button
              v-if="!topFolders.length"
              type="primary"
              icon="plus"
              class="empty-create-button"
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

    <!-- 解锁密码对话框 -->
    <van-dialog
      v-model:show="showUnlock"
      title="解锁笔记"
      show-cancel-button
      @confirm="unlockConfirm"
      @cancel="unlockCancel"
    >
      <van-field v-model="unlockPwd" type="password" placeholder="输入密码锁密码" autofocus />
    </van-dialog>
  </div>
</template>

<style scoped>
.home-page {
  min-height: 100vh;
  min-height: 100dvh;
  background: var(--van-background);
}

.folder-strip {
  position: sticky;
  top: 0;
  z-index: 4;
  overflow: hidden;
  padding: 0 8px;
  border-bottom: 1px solid var(--van-border-color);
  background: color-mix(in srgb, var(--van-background) 94%, transparent);
  backdrop-filter: blur(14px);
}

.folder-strip :deep(.van-tabs__nav) {
  gap: 4px;
  padding: 0 8px;
}

.folder-strip :deep(.van-tab) {
  min-width: auto;
  padding: 0 11px;
}

.doc-list {
  padding: 10px 12px 20px;
}

.doc-swipe-cell {
  margin-bottom: 8px;
  overflow: hidden;
  border: 1px solid var(--van-border-color);
  border-radius: 7px;
  background: var(--van-background-2);
}

.doc-swipe-cell :deep(.van-swipe-cell__wrapper) {
  min-height: 108px;
}

.doc-cell {
  min-height: 108px;
  padding: 14px 15px 13px;
}

.doc-cell::after {
  display: none;
}

.doc-heading {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 8px;
}

.doc-dot {
  width: 7px;
  height: 7px;
  flex: 0 0 7px;
  border-radius: 50%;
  box-shadow: 0 0 0 3px color-mix(in srgb, currentColor 8%, transparent);
}

.doc-title {
  overflow: hidden;
  color: var(--van-text-color);
  font-size: 15px;
  font-weight: 650;
  line-height: 1.4;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.doc-preview {
  display: -webkit-box;
  overflow: hidden;
  margin: 7px 0 0 15px;
  color: var(--van-text-color-2);
  font-size: 12px;
  line-height: 1.5;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 1;
}

.doc-meta {
  display: flex;
  min-height: 17px;
  align-items: center;
  gap: 8px;
  margin: 9px 0 0 15px;
}

.doc-folder {
  overflow: hidden;
  max-width: 54%;
  padding: 1px 5px;
  border-radius: 3px;
  background: color-mix(in srgb, var(--van-primary-color) 10%, transparent);
  color: var(--van-primary-color);
  font-size: 10px;
  line-height: 1.5;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.doc-time {
  margin-left: auto;
  color: var(--van-text-color-3, var(--van-text-color-2));
  font-size: 10px;
  font-variant-numeric: tabular-nums;
}

.swipe-del {
  height: 100%;
}

.swipe-lock {
  height: 100%;
}

.lock-ic {
  flex-shrink: 0;
  margin-left: auto;
  color: #d3a32c;
  font-size: 14px;
}

.empty-create-button {
  border-radius: 7px;
}
</style>
