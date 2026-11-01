<script setup lang="ts">
/**
 * 移动端首页：层级目录导航 + 文稿列表。
 *
 * 设计取舍（针对旧版可用性问题）：
 * - 旧版把根目录做成横向 tab，二级及更深目录在移动端完全无法触达；
 *   现在改为「下钻 + 面包屑」，任意层级都能进出。
 * - 旧版必须先进某个目录才能新建文稿；现在右下角一键新建，
 *   在根视图新建直接落到草稿箱，不再先弹名称输入框。
 * - 目录与文稿分成两个明确的区块（目录在前、文稿在后），
 *   避免旧版“全部文稿”里目录/文稿混杂难以扫读。
 */
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { showToast, showConfirmDialog } from 'vant'
import type { FolderNode } from '../../src/types/folder'
import type { Document } from '../../src/types/document'
import { useWorkspaceStore } from '../../src/stores/workspace'
import { useLockStore } from '../../src/stores/lock'
import FolderPicker from '../components/FolderPicker.vue'
import MIcon from '../components/MIcon.vue'

const router = useRouter()
const store = useWorkspaceStore()
const lockStore = useLockStore()

const currentFolderId = ref<string | null>(null)
const path = ref<FolderNode[]>([])
const refreshing = ref(false)
const loading = ref(true)
const creating = ref(false)

const currentNode = computed<FolderNode | null>(() =>
  currentFolderId.value ? store.findFolderById(store.folders, currentFolderId.value) : null,
)
const childFolders = computed<FolderNode[]>(() =>
  currentNode.value ? currentNode.value.children : store.folders,
)
const documents = computed<Document[]>(() => store.currentFolderDocuments)
const viewTitle = computed(() => currentNode.value?.name || '全部文稿')
const emptyDescription = computed(() =>
  currentFolderId.value ? '这个目录还没有文稿' : '还没有任何文稿，点右下角开始写',
)

// ------- 加载 -------
async function loadRoot() {
  currentFolderId.value = null
  path.value = []
  await store.browseAllDocuments()
}

async function enterFolder(folder: FolderNode) {
  currentFolderId.value = folder.id
  // 面包屑：把当前路径栈中该目录及其父级保留下来
  const existingIndex = path.value.findIndex((node) => node.id === folder.id)
  path.value = existingIndex >= 0 ? path.value.slice(0, existingIndex + 1) : [...path.value, folder]
  // 压一条同 URL 的历史记录，让 Android 物理返回键先“退一层”而不是直接退出应用。
  window.history.pushState({ wooFolder: folder.id }, '')
  await store.browseFolder(folder.id)
}

/** Android 物理返回键：在目录内部时逐层返回，到根级后交还给系统（退出应用）。 */
function onPopState() {
  if (path.value.length) {
    void goUp()
  }
}

async function goToLevel(index: number) {
  // index = -1 表示回到根
  if (index < 0) {
    await loadRoot()
    return
  }
  const target = path.value[index]
  if (!target) return
  path.value = path.value.slice(0, index + 1)
  currentFolderId.value = target.id
  await store.browseFolder(target.id)
}

async function goUp() {
  await goToLevel(path.value.length - 2)
}

async function refresh() {
  try {
    await store.bootstrap()
    if (currentFolderId.value) {
      const stillExists = store.findFolderById(store.folders, currentFolderId.value)
      if (!stillExists) {
        await loadRoot()
        return
      }
      // 目录树刷新后节点对象已变，重建面包屑
      path.value = buildPath(store.folders, currentFolderId.value) || []
      await store.browseFolder(currentFolderId.value)
    } else {
      await store.browseAllDocuments()
    }
  } catch {
    showToast('刷新失败')
  } finally {
    refreshing.value = false
  }
}

function buildPath(nodes: FolderNode[], targetId: string, trail: FolderNode[] = []): FolderNode[] | null {
  for (const node of nodes) {
    const next = [...trail, node]
    if (node.id === targetId) return next
    const found = buildPath(node.children, targetId, next)
    if (found) return found
  }
  return null
}

onMounted(async () => {
  window.addEventListener('popstate', onPopState)
  try {
    await store.bootstrap()
  } catch {
    showToast('加载目录失败')
  }
  // 保留上次浏览的目录（桌面端与移动端共享 woo:lastView）
  const lastView = localStorage.getItem('woo:lastView')
  if (lastView && !lastView.startsWith('__')) {
    const node = store.findFolderById(store.folders, lastView)
    if (node) {
      currentFolderId.value = node.id
      path.value = buildPath(store.folders, node.id) || []
      await store.browseFolder(node.id)
      loading.value = false
      return
    }
  }
  await loadRoot()
  loading.value = false
})

onBeforeUnmount(() => {
  window.removeEventListener('popstate', onPopState)
})

// ------- 新建 -------
async function createNote() {
  if (creating.value) return
  creating.value = true
  try {
    if (currentFolderId.value) {
      const doc = await store.createDocument(currentFolderId.value)
      if (doc) {
        router.push(`/note/${doc.id}`)
        return
      }
      showToast(store.error || '创建失败')
      return
    }
    // 根视图没有归属目录 → 建草稿，稍后可在编辑器里归入目录
    store.createNewDocument()
    const id = store.selectedDocumentId
    if (id) {
      router.push(`/note/${id}`)
    } else {
      showToast('创建失败')
    }
  } finally {
    creating.value = false
  }
}

function openNote(id: string) {
  router.push(`/note/${id}`)
}

// ------- 文稿操作 -------
const showDocActions = ref(false)
const activeDoc = ref<Document | null>(null)
const showFolderPicker = ref(false)

function openDocMenu(doc: Document) {
  activeDoc.value = doc
  showDocActions.value = true
}

async function onDocAction(action: { key: string }) {
  const doc = activeDoc.value
  showDocActions.value = false
  if (!doc) return

  if (action.key === 'lock') {
    if (doc.isLocked) {
      const password = await promptPassword()
      if (password === null) return
      const ok = await lockStore.verify(password)
      if (!ok) {
        showToast('密码错误')
        return
      }
      await lockStore.unlockDocument(doc.id)
      showToast('已解锁')
    } else {
      if (!lockStore.hasPassword) {
        showToast('请先在「我的 → 密码锁」中设置密码')
        return
      }
      await lockStore.lockDocument(doc.id)
      showToast('已加锁')
    }
    await refreshCurrent()
    return
  }

  if (action.key === 'move') {
    showFolderPicker.value = true
    return
  }

  if (action.key === 'delete') {
    try {
      await showConfirmDialog({
        title: '删除文稿',
        message: `「${doc.title || '无标题'}」将移入回收站`,
      })
      await store.deleteDocument(doc.id)
      showToast('已移入回收站')
      await refreshCurrent()
    } catch { /* 取消 */ }
  }
}

async function onFolderPicked(folderId: string) {
  const doc = activeDoc.value
  if (!doc) return
  const moved = await store.moveDocument(doc.id, folderId)
  if (!moved) {
    showToast(store.error || '移动失败')
    return
  }
  showToast('已移动')
  await refreshCurrent()
}

async function refreshCurrent() {
  if (currentFolderId.value) {
    await store.browseFolder(currentFolderId.value)
  } else {
    await store.browseAllDocuments()
  }
}

function docActions(doc: Document): { name: string; key: string; color?: string }[] {
  return [
    { name: '移动到目录', key: 'move' },
    { name: doc.isLocked ? '解除锁定' : '加锁保护', key: 'lock' },
    { name: '移入回收站', key: 'delete', color: '#ee0a24' },
  ]
}

// ------- 目录操作 -------
const showFolderSheet = ref(false)
const activeFolder = ref<FolderNode | null>(null)
const showNameDialog = ref(false)
const nameInput = ref('')
const nameTarget = ref<'root' | 'child' | 'rename'>('root')

function openFolderMenu(folder: FolderNode) {
  activeFolder.value = folder
  showFolderSheet.value = true
}

async function onFolderAction(action: { key: string }) {
  const folder = activeFolder.value
  showFolderSheet.value = false
  if (!folder) return

  if (action.key === 'enter') {
    await enterFolder(folder)
  } else if (action.key === 'rename') {
    nameTarget.value = 'rename'
    nameInput.value = folder.name
    showNameDialog.value = true
  } else if (action.key === 'child') {
    nameTarget.value = 'child'
    nameInput.value = ''
    showNameDialog.value = true
  } else if (action.key === 'delete') {
    try {
      await showConfirmDialog({
        title: '删除目录',
        message: `「${folder.name}」中的文稿会一起移入回收站`,
      })
      await store.deleteFolder(folder)
      if (path.value.some((node) => node.id === folder.id)) {
        await loadRoot()
      } else {
        await refreshCurrent()
      }
      showToast('已删除')
    } catch { /* 取消 */ }
  }
}

function folderActionsMenu(): { name: string; key: string; color?: string }[] {
  return [
    { name: '进入目录', key: 'enter' },
    { name: '新建子目录', key: 'child' },
    { name: '重命名', key: 'rename' },
    { name: '删除目录', key: 'delete', color: '#ee0a24' },
  ]
}

function openCreateFolder() {
  nameTarget.value = currentFolderId.value ? 'child' : 'root'
  nameInput.value = ''
  showNameDialog.value = true
}

async function submitFolderName() {
  const name = nameInput.value.trim()
  if (!name) return

  if (nameTarget.value === 'rename' && activeFolder.value) {
    await store.renameFolder(activeFolder.value, name)
    showToast('已重命名')
    await refresh()
    return
  }

  if (nameTarget.value === 'child' && currentNode.value) {
    await store.createChildFolder(currentNode.value)
  } else {
    await store.createRootFolder()
  }
  const createdId = store.editingFolderId
  const created = createdId ? store.findFolderById(store.folders, createdId) : null
  if (created) {
    await store.renameFolder(created, name)
  }
  store.clearEditingFolder()
  showToast('已创建')
  await refresh()
}

function validateFolderName(action: string): boolean {
  if (action === 'confirm' && !nameInput.value.trim()) {
    showToast('请输入名称')
    return false
  }
  return true
}

// ------- 解锁 -------
const showUnlock = ref(false)
const unlockPwd = ref('')
let unlockResolve: ((value: string | null) => void) | null = null

function promptPassword(): Promise<string | null> {
  unlockPwd.value = ''
  showUnlock.value = true
  return new Promise((resolve) => { unlockResolve = resolve })
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

// ------- 展示辅助 -------
function previewText(doc: Document): string {
  if (doc.isLocked) return '已加锁'
  // 摘要统一走 store（内部会按块级元素切行，跳过作为标题的首行）
  return store.getDocumentPreview(doc) || '暂无正文'
}

function formatTime(iso: string): string {
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return ''
  const now = new Date()
  const sameDay = date.toDateString() === now.toDateString()
  if (sameDay) {
    return date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
  }
  const sameYear = date.getFullYear() === now.getFullYear()
  return date.toLocaleDateString('zh-CN', sameYear
    ? { month: 'numeric', day: 'numeric' }
    : { year: 'numeric', month: 'numeric', day: 'numeric' })
}

function subfolderHint(folder: FolderNode): string {
  if (folder.isLocked) return '已加锁'
  // 空目录不显示冗余的“目录”二字，保持行内信息克制
  return folder.children.length ? `${folder.children.length} 个子目录` : ''
}
</script>

<template>
  <div class="home">
    <header class="w-head">
      <div class="w-head__row">
        <button
          v-if="currentFolderId"
          type="button"
          class="w-icon-btn"
          aria-label="返回上级"
          @click="goUp"
        >
          <van-icon name="arrow-left" />
        </button>
        <div class="w-head__titles">
          <span class="w-head__kicker">文稿</span>
          <h1 class="w-head__title">{{ viewTitle }}</h1>
        </div>
        <button type="button" class="w-icon-btn" aria-label="搜索" @click="router.push('/search')">
          <van-icon name="search" />
        </button>
        <button type="button" class="w-icon-btn" aria-label="新建目录" @click="openCreateFolder">
          <van-icon name="plus" />
        </button>
      </div>

      <p class="w-head__meta">
        {{ documents.length }} 篇文稿<template v-if="childFolders.length"> · {{ childFolders.length }} 个目录</template>
      </p>

      <nav v-if="path.length" class="home-crumbs" aria-label="目录层级">
        <button type="button" class="w-crumb" @click="goToLevel(-1)">全部</button>
        <template v-for="(node, index) in path" :key="node.id">
          <span class="w-crumb-sep">/</span>
          <button
            type="button"
            class="w-crumb"
            :class="{ 'is-current': index === path.length - 1 }"
            @click="goToLevel(index)"
          >
            {{ node.name }}
          </button>
        </template>
      </nav>
    </header>

    <van-pull-refresh v-model="refreshing" class="home-body" @refresh="refresh">
      <section v-if="childFolders.length" class="w-section">
        <p class="w-section__label">目录</p>
        <ul class="w-group">
          <li v-for="folder in childFolders" :key="folder.id">
            <button
              type="button"
              class="w-row folder-row"
              @click="enterFolder(folder)"
              @contextmenu.prevent="openFolderMenu(folder)"
            >
              <span class="w-row__icon">
                <van-icon v-if="folder.isLocked" name="closed-eye" />
                <MIcon v-else name="folder" :size="17" />
              </span>
              <span class="w-row__main">
                <span class="w-row__title">{{ folder.name }}</span>
              </span>
              <span v-if="subfolderHint(folder)" class="w-row__side">{{ subfolderHint(folder) }}</span>
              <van-icon name="arrow" class="w-row__chev" />
            </button>
          </li>
        </ul>
      </section>

      <van-loading v-if="loading && !documents.length" class="mobile-loading" />

      <section v-else-if="documents.length" class="w-section">
        <p class="w-section__label">文稿</p>
        <ul class="w-group">
          <li v-for="doc in documents" :key="doc.id">
            <button
              type="button"
              class="w-row doc-row"
              @click="openNote(doc.id)"
              @contextmenu.prevent="openDocMenu(doc)"
            >
              <span class="w-row__icon">
                <van-icon v-if="doc.isLocked" name="closed-eye" />
                <MIcon v-else name="doc" :size="17" />
              </span>
              <span class="w-row__main">
                <span class="w-row__title">{{ doc.title || '无标题' }}</span>
                <span class="w-row__sub">
                  <span v-if="doc.folderName" class="doc-folder">{{ doc.folderName }}</span>
                  {{ previewText(doc) }}
                </span>
              </span>
              <span class="w-row__side">
                <time>{{ formatTime(doc.updatedAt) }}</time>
              </span>
            </button>
            <button
              type="button"
              class="doc-more"
              aria-label="文稿操作"
              @click.stop="openDocMenu(doc)"
            >
              <van-icon name="ellipsis" />
            </button>
          </li>
        </ul>
      </section>

      <van-empty v-else :description="emptyDescription" image-size="72" class="w-empty" />
    </van-pull-refresh>

    <button type="button" class="w-fab" aria-label="新建文稿" :disabled="creating" @click="createNote">
      <van-icon name="plus" />
    </button>

    <FolderPicker
      v-model:show="showFolderPicker"
      :title="activeDoc ? `移动「${activeDoc.title || '无标题'}」到` : '移动到目录'"
      @select="onFolderPicked"
    />

    <van-action-sheet
      v-model:show="showDocActions"
      :actions="activeDoc ? docActions(activeDoc) : []"
      cancel-text="取消"
      close-on-click-action
      @select="onDocAction"
    />

    <van-action-sheet
      v-model:show="showFolderSheet"
      :actions="activeFolder ? folderActionsMenu() : []"
      cancel-text="取消"
      close-on-click-action
      @select="onFolderAction"
    />

    <van-dialog
      v-model:show="showNameDialog"
      :title="nameTarget === 'rename' ? '重命名目录' : '新建目录'"
      show-cancel-button
      :before-close="validateFolderName"
      @confirm="submitFolderName"
    >
      <van-field v-model="nameInput" placeholder="目录名称" autofocus maxlength="50" clearable />
    </van-dialog>

    <van-dialog
      v-model:show="showUnlock"
      title="输入密码"
      show-cancel-button
      @confirm="unlockConfirm"
      @cancel="unlockCancel"
    >
      <van-field v-model="unlockPwd" type="password" placeholder="密码锁密码" autofocus />
    </van-dialog>
  </div>
</template>

<style scoped>
.home {
  min-height: 100vh;
  min-height: 100dvh;
  background: var(--w-paper);
}

.home-crumbs {
  display: flex;
  align-items: center;
  gap: 2px;
  overflow-x: auto;
  margin-top: 4px;
  padding-bottom: 1px;
  scrollbar-width: none;
}

.home-crumbs::-webkit-scrollbar {
  display: none;
}

/* 目录是单行信息，不必占两行高度 */
.folder-row {
  min-height: 54px;
}

.home-body {
  min-height: 60vh;
  padding-bottom: calc(96px + env(safe-area-inset-bottom, 0px));
}

/* 文稿行：右侧留出「···」的位置，避免与时间撞在一起 */
.doc-row {
  padding-right: 40px;
}

.doc-folder {
  color: var(--w-ink-3);
}

.doc-folder::after {
  margin: 0 4px;
  color: var(--w-ink-3);
  content: '·';
}

.doc-more {
  position: absolute;
  top: 50%;
  right: 6px;
  display: inline-flex;
  width: 28px;
  height: 32px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--w-ink-3);
  font-size: 16px;
  transform: translateY(-50%);
}

.doc-more:active {
  background: var(--w-rise);
  color: var(--w-accent);
}
</style>
