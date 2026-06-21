<template>
    <aside class="left-sidebar" :class="{ 'collapsed': !isOpen }" @contextmenu="handleContextMenu">
        <div class="sidebar-section" @contextmenu.stop.prevent>
            <div class="sidebar-item" @click="handleNewDocument">
                <IconNewDocument />
                <span>新建文档</span>
            </div>
            <div class="sidebar-item" v-if="!isSearchMode" @click="enableSearch">
                <IconSearch />
                <span>搜索</span>
            </div>
            <div class="sidebar-item search-item" v-else>
                <IconSearch />
                <input 
                    ref="searchInputRef"
                    v-model="searchKeyword" 
                    type="text" 
                    class="search-input-inline"
                    placeholder="搜索标题和内容..."
                    @input="handleSearchInput"
                    @blur="handleSearchBlur"
                    @keyup.escape="disableSearch"
                />
            </div>
            <div class="sidebar-item" @click="handleOpenAll">
                <IconFile />
                <span>全部</span>
            </div>
            <div class="sidebar-item" @click="handleOpenDraftBox">
                <IconDraft />
                <span>草稿箱</span>
            </div>
            <div class="sidebar-item" @click="handleOpenTrashBox">
                <IconTrash />
                <span>废纸篓</span>
            </div>
        </div>
        <div class="divider"></div>
        <div class="sidebar-section">
            <FolderTree 
                :folders="store.folders" 
                :selected-folder-id="store.selectedFolderId"
                @context-menu="handleFolderContextMenu"
                @folder-select="handleFolderSelect"
                @rename="handleRename"
            />
        </div>
        
        <!-- 右键菜单 -->
        <ContextMenu
            v-if="showContextMenu"
            :position="contextMenuPosition"
            :items="contextMenuItems"
            @select="handleMenuSelect"
            @close="closeContextMenu"
        />

        <!-- 加锁密码弹窗 -->
        <LockPasswordDialog
            v-if="showLockDialog"
            :mode="lockDialogMode"
            @confirm="handleLockConfirm"
            @cancel="handleLockCancel"
        />


    </aside>
</template>

<script setup lang="ts">
import { ref, nextTick, watch } from 'vue'
import FolderTree from './FolderTree.vue'
import ContextMenu from '../ui/ContextMenu.vue'
import IconNewDocument from '../icons/IconNewDocument.vue'
import IconSearch from '../icons/IconSearch.vue'
import IconFile from '../icons/IconFile.vue'
import IconDraft from '../icons/IconDraft.vue'
import IconTrash from '../icons/IconTrash.vue'
import { useWorkspaceStore } from '../../stores/workspace'
import { log } from '../../services/logger'
import { useLockStore } from '../../stores/lock'
import LockPasswordDialog from './LockPasswordDialog.vue'
import type { FolderNode, ContextMenuPosition, ContextMenuItem } from '../../types/folder'

interface Props {
    isOpen: boolean
}

defineProps<Props>()

const store = useWorkspaceStore()
const lockStore = useLockStore()

// 加锁弹窗状态
const showLockDialog = ref(false)
const lockDialogMode = ref<'set' | 'verify'>('verify')
const pendingLockTarget = ref<FolderNode | null>(null)

// 搜索相关状态
const isSearchMode = ref(false)
const searchKeyword = ref('')
const searchInputRef = ref<HTMLInputElement | null>(null)
let searchDebounceTimer: number | null = null

// 启用搜索模式
const enableSearch = async () => {
    isSearchMode.value = true
    searchKeyword.value = ''
    // 取消文件夹选中，进入全局搜索模式
    store.selectedFolderId = null
    store.selectedDocumentId = null
    // 清空当前文档
    if (store.currentDocument) {
        // 通过重置选中文档ID来清空currentDocument
        store.selectedDocumentId = null
    }
    
    await nextTick()
    searchInputRef.value?.focus()
}

// 禁用搜索模式
const disableSearch = () => {
    isSearchMode.value = false
    searchKeyword.value = ''
    if (searchDebounceTimer) {
        clearTimeout(searchDebounceTimer)
        searchDebounceTimer = null
    }
    // 清空搜索结果（通过调用openSearch传入空字符串）
    store.openSearch('')
}

// 搜索输入失焦处理：输入栏失焦即关闭搜索模式
const handleSearchBlur = () => {
    if (!searchKeyword.value.trim()) {
        // 输入框为空时，直接退出搜索模式，不触发搜索
        isSearchMode.value = false
        return
    }
    // 延时关闭，避免点击输入栏内其他元素时误触
    setTimeout(() => {
        if (isSearchMode.value) {
            disableSearch()
        }
    }, 200)
}

// 处理搜索输入（带防抖）
const handleSearchInput = () => {
    if (searchDebounceTimer) {
        clearTimeout(searchDebounceTimer)
    }
    
    searchDebounceTimer = window.setTimeout(async () => {
        const keyword = searchKeyword.value.trim()
        if (!keyword) {
            // 空关键词，清空结果
            await store.openSearch('')
            return
        }
        
        log.app.info('[LeftSidebar] searching with:', keyword)
        await store.openSearch(keyword)
        log.app.info('[LeftSidebar] search completed')
    }, 300) // 300ms 防抖
}

// 监听搜索模式关闭，清理定时器
watch(isSearchMode, (newVal) => {
    if (!newVal && searchDebounceTimer) {
        clearTimeout(searchDebounceTimer)
        searchDebounceTimer = null
    }
})

// 右键菜单状态
const showContextMenu = ref(false)
const contextMenuPosition = ref<ContextMenuPosition>({ x: 0, y: 0 })
const contextMenuItems = ref<ContextMenuItem[]>([])
const selectedFolder = ref<FolderNode | null>(null)
const contextMenuTarget = ref<'folder' | 'empty'>('empty')

// 处理目录选中（点击目录时触发）
const handleFolderSelect = (folder: FolderNode) => {
    store.selectFolder(folder.id)
    store.toggleFolder(folder)
}

// 处理空白区域右键点击
const handleContextMenu = (event: MouseEvent) => {
    event.preventDefault()
    event.stopPropagation()
    
    contextMenuTarget.value = 'empty'
    selectedFolder.value = null
    
    contextMenuPosition.value = { x: event.clientX, y: event.clientY }
    contextMenuItems.value = [
        { label: '新建文件夹', action: 'createRoot' }
    ]
    showContextMenu.value = true
}

// 处理文件夹右键点击（由子组件触发）
const handleFolderContextMenu = (data: { folder: FolderNode, position: ContextMenuPosition }) => {
    contextMenuTarget.value = 'folder'
    selectedFolder.value = data.folder

    contextMenuPosition.value = data.position
    contextMenuItems.value = data.folder.isLocked
      ? [{ label: '解锁', action: 'toggleLock' }]
      : [
          { label: '创建同级目录', action: 'createSibling' },
          { label: '创建子目录', action: 'createChild' },
          { label: '重命名', action: 'rename' },
          { label: '加锁', action: 'toggleLock' },
          { label: '删除当前目录', action: 'delete', disabled: false }
        ]
    showContextMenu.value = true
}

// 处理菜单项选择
const handleMenuSelect = (action: string) => {
  log.app.info('[LeftSidebar] 菜单选择:', action, '选中的目录:', selectedFolder.value)
  switch (action) {
    case 'createRoot':
      log.app.info('[LeftSidebar] 调用 createRootFolder')
      store.createRootFolder().then(() => {
        log.app.info('[LeftSidebar] createRootFolder 完成, 当前文件夹数:', store.folders.length)
      }).catch((e: any) => {
        log.app.error('[LeftSidebar] createRootFolder 失败:', e)
      })
      break
    case 'createSibling':
      if (selectedFolder.value) store.createSiblingFolder(selectedFolder.value)
      break
    case 'createChild':
      if (selectedFolder.value) store.createChildFolder(selectedFolder.value)
      break
    case 'rename':
      if (selectedFolder.value) store.editingFolderId = selectedFolder.value.id
      break
    case 'delete':
      if (selectedFolder.value) store.deleteFolder(selectedFolder.value)
      break
    case 'toggleLock':
      if (selectedFolder.value) {
        handleLockAction(selectedFolder.value)
      }
      break
  }
  closeContextMenu()
}

// ============ 加锁/解锁 ============
function handleLockAction(folder: FolderNode) {
  // 解锁操作始终需要验证密码
  if (folder.isLocked) {
    lockDialogMode.value = 'verify'
    pendingLockTarget.value = folder
    showLockDialog.value = true
    return
  }
  // 加锁操作：若当前会话已验证过密码可跳过
  if (lockStore.sessionVerified) {
    performToggleLock(folder)
    return
  }
  lockDialogMode.value = lockStore.hasPassword ? 'verify' : 'set'
  pendingLockTarget.value = folder
  showLockDialog.value = true
}

async function handleLockConfirm(_password: string) {
  showLockDialog.value = false
  const target = pendingLockTarget.value
  pendingLockTarget.value = null
  if (target) {
    await performToggleLock(target)
  }
}

function handleLockCancel() {
  showLockDialog.value = false
  pendingLockTarget.value = null
}

async function performToggleLock(folder: FolderNode) {
  try {
    if (folder.isLocked) {
      await lockStore.unlockFolder(folder.id)
    } else {
      await lockStore.lockFolder(folder.id)
    }
    await store.syncRefresh()
  } catch (e: any) {
    store.error = e?.message || '操作失败'
  }
}

// 关闭右键菜单
const closeContextMenu = () => {
    showContextMenu.value = false
    selectedFolder.value = null
}

// 处理重命名
const handleRename = (data: { folder: FolderNode, newName: string }) => {
    store.renameFolder(data.folder, data.newName)
}

// 新建文档：有选中真实目录则在该目录下创建，否则归入草稿箱
const handleNewDocument = () => {
    store.createNewDocument()
}

// 打开全部文稿视图
const handleOpenAll = () => {
    void store.openAllDocuments()
}

// 打开草稿箱视图
const handleOpenDraftBox = () => {
    store.openDraftBox()
}

const handleOpenTrashBox = () => {
    void store.openTrashBox()
}
</script>

<style scoped>
.left-sidebar {
    width: 220px;
    background-color: var(--bg-secondary);
    border-right: 1px solid var(--border-primary);
    padding: 12px 8px;
    overflow-y: auto;
    overflow-x: hidden;
    transition: width 0.3s, padding 0.3s, opacity 0.3s, var(--theme-transition);
    display: flex;
    flex-direction: column;
    user-select: none;
}

.left-sidebar.collapsed {
    width: 0;
    padding: 0;
    opacity: 0;
    overflow: hidden;
}

.sidebar-section {
    margin-top: 4px;
    margin-bottom: 4px;
}

.sidebar-item {
    padding: 8px 8px;
    margin: 2px 0;
    cursor: pointer;
    border-radius: 6px;
    font-size: 13px;
    color: var(--text-primary);
    display: flex;
    align-items: center;
    gap: 10px;
    transition: all 0.2s;
    height: 36px;
}

.sidebar-item:hover {
    background-color: var(--bg-hover);
}

.sidebar-item:active {
    background-color: var(--bg-active);
}

.sidebar-item :deep(svg) {
    width: 16px;
    height: 16px;
    color: var(--text-secondary);
    flex-shrink: 0;
}

.sidebar-item span {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

/* 搜索模式下的输入框样式 */
.sidebar-item.search-item {
    padding: 6px 8px;
    gap: 8px;
    cursor: default;
}

.sidebar-item.search-item:hover {
    background-color: var(--bg-secondary);
}

.search-input-inline {
    flex: 1;
    border: none;
    background: transparent;
    color: var(--text-primary);
    font-size: 13px;
    outline: none;
    padding: 0;
    margin: 0;
}

.search-input-inline::placeholder {
    color: var(--text-secondary);
}

.search-input-inline:focus {
    background-color: var(--bg-elevated);
    border-radius: 4px;
    padding: 2px 6px;
    margin: -2px -6px;
}

.search-item:focus-within {
    background-color: var(--bg-hover);
    border-radius: 6px;
}

.divider {
    height: 1px;
    background-color: var(--border-primary);
    margin: 8px 0;
}

</style>
