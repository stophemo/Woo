<template>
    <aside class="left-sidebar" :class="{ 'collapsed': !isOpen }">
        <div class="sidebar-section">
            <div class="sidebar-item">
                <IconNewDocument />
                <span>新建文档</span>
            </div>
            <div class="sidebar-item">
                <IconSearch />
                <span>搜索</span>
            </div>
            <div class="sidebar-item">
                <IconDraft />
                <span>草稿箱</span>
            </div>
            <div class="sidebar-item">
                <IconTrash />
                <span>废纸篓</span>
            </div>
        </div>
        <div class="divider"></div>
        <div class="sidebar-section" @contextmenu="handleContextMenu">
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
    </aside>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import FolderTree from './FolderTree.vue'
import ContextMenu from '../ui/ContextMenu.vue'
import IconNewDocument from '../icons/IconNewDocument.vue'
import IconSearch from '../icons/IconSearch.vue'
import IconDraft from '../icons/IconDraft.vue'
import IconTrash from '../icons/IconTrash.vue'
import { useWorkspaceStore } from '../../stores/workspace'
import type { FolderNode, ContextMenuPosition, ContextMenuItem } from '../../types/folder'

interface Props {
    isOpen: boolean
}

defineProps<Props>()

const store = useWorkspaceStore()

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
        { label: '新建目录', action: 'createRoot' }
    ]
    showContextMenu.value = true
}

// 处理文件夹右键点击（由子组件触发）
const handleFolderContextMenu = (data: { folder: FolderNode, position: ContextMenuPosition }) => {
    contextMenuTarget.value = 'folder'
    selectedFolder.value = data.folder
    
    contextMenuPosition.value = data.position
    contextMenuItems.value = [
        { label: '创建同级目录', action: 'createSibling' },
        { label: '创建子目录', action: 'createChild' },
        { label: '删除当前目录', action: 'delete', disabled: false }
    ]
    showContextMenu.value = true
}

// 处理菜单项选择
const handleMenuSelect = (action: string) => {
    switch (action) {
        case 'createRoot':
            store.createRootFolder()
            break
        case 'createSibling':
            if (selectedFolder.value) store.createSiblingFolder(selectedFolder.value)
            break
        case 'createChild':
            if (selectedFolder.value) store.createChildFolder(selectedFolder.value)
            break
        case 'delete':
            if (selectedFolder.value) store.deleteFolder(selectedFolder.value)
            break
    }
    closeContextMenu()
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

.divider {
    height: 1px;
    background-color: var(--border-primary);
    margin: 8px 0;
}

</style>
