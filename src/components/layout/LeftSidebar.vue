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
                :folders="folders" 
                @context-menu="handleFolderContextMenu"
                @folder-action="handleFolderAction"
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
import { ref, computed } from 'vue'
import FolderTree from './FolderTree.vue'
import ContextMenu from '../ui/ContextMenu.vue'
import IconNewDocument from '../icons/IconNewDocument.vue'
import IconSearch from '../icons/IconSearch.vue'
import IconDraft from '../icons/IconDraft.vue'
import IconTrash from '../icons/IconTrash.vue'
import type { FolderNode, ContextMenuPosition, ContextMenuItem } from '../../types/folder'

interface Props {
    isOpen: boolean
}

defineProps<Props>()

// 初始目录数据
const folders = ref<FolderNode[]>([
    {
        id: '1',
        name: '随想',
        children: [],
        parentId: null,
        isExpanded: false
    },
    {
        id: '2',
        name: '随记',
        children: [],
        parentId: null,
        isExpanded: false
    },
    {
        id: '3',
        name: '随摘',
        children: [],
        parentId: null,
        isExpanded: false
    },
    {
        id: '4',
        name: '备忘',
        children: [],
        parentId: null,
        isExpanded: false
    },
    {
        id: '5',
        name: '工作',
        children: [],
        parentId: null,
        isExpanded: false
    }
])

// 右键菜单状态
const showContextMenu = ref(false)
const contextMenuPosition = ref<ContextMenuPosition>({ x: 0, y: 0 })
const contextMenuItems = ref<ContextMenuItem[]>([])
const selectedFolder = ref<FolderNode | null>(null)
const contextMenuTarget = ref<'folder' | 'empty'>('empty')

// 处理空白区域右键点击
const handleContextMenu = (event: MouseEvent) => {
    event.preventDefault()
    event.stopPropagation()
    
    // 检查是否点击在文件夹项上（由FolderTree组件处理）
    // 如果到达这里，说明点击在空白区域
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
            createRootFolder()
            break
        case 'createSibling':
            createSiblingFolder()
            break
        case 'createChild':
            createChildFolder()
            break
        case 'delete':
            deleteFolder()
            break
    }
    closeContextMenu()
}

// 创建根级目录
const createRootFolder = () => {
    const newFolder: FolderNode = {
        id: Date.now().toString(),
        name: '新建目录',
        children: [],
        parentId: null,
        isExpanded: false
    }
    folders.value.push(newFolder)
}

// 创建同级目录
const createSiblingFolder = () => {
    if (!selectedFolder.value) return
    
    const sibling: FolderNode = {
        id: Date.now().toString(),
        name: '新建目录',
        children: [],
        parentId: selectedFolder.value.parentId,
        isExpanded: false
    }
    
    // 找到父级并添加子项
    if (selectedFolder.value.parentId === null) {
        // 根级目录的同级
        folders.value.push(sibling)
    } else {
        // 在父目录的children中添加
        addFolderToParent(folders.value, selectedFolder.value.parentId, sibling)
    }
}

// 创建子目录
const createChildFolder = () => {
    if (!selectedFolder.value) return
    
    const child: FolderNode = {
        id: Date.now().toString(),
        name: '新建子目录',
        children: [],
        parentId: selectedFolder.value.id,
        isExpanded: false
    }
    
    selectedFolder.value.children.push(child)
    selectedFolder.value.isExpanded = true
}

// 删除目录
const deleteFolder = () => {
    if (!selectedFolder.value) return
    
    if (selectedFolder.value.parentId === null) {
        // 根级目录，直接从根数组中删除
        const index = folders.value.findIndex(f => f.id === selectedFolder.value!.id)
        if (index !== -1) {
            folders.value.splice(index, 1)
        }
    } else {
        // 非根级目录，从父目录的children中删除
        removeFolderFromParent(folders.value, selectedFolder.value.id)
    }
    
    selectedFolder.value = null
}

// 辅助函数：向指定父ID添加文件夹
const addFolderToParent = (nodes: FolderNode[], parentId: string, folder: FolderNode): boolean => {
    for (const node of nodes) {
        if (node.id === parentId) {
            node.children.push(folder)
            return true
        }
        if (node.children.length > 0) {
            if (addFolderToParent(node.children, parentId, folder)) {
                return true
            }
        }
    }
    return false
}

// 辅助函数：从指定父ID下删除文件夹
const removeFolderFromParent = (nodes: FolderNode[], folderId: string): boolean => {
    for (const node of nodes) {
        const index = node.children.findIndex(f => f.id === folderId)
        if (index !== -1) {
            node.children.splice(index, 1)
            return true
        }
        if (node.children.length > 0) {
            if (removeFolderFromParent(node.children, folderId)) {
                return true
            }
        }
    }
    return false
}

// 关闭右键菜单
const closeContextMenu = () => {
    showContextMenu.value = false
    selectedFolder.value = null
}

// 处理文件夹操作（如展开/折叠）
const handleFolderAction = (data: { action: string, folder: FolderNode }) => {
    if (data.action === 'toggle') {
        data.folder.isExpanded = !data.folder.isExpanded
    }
}

// 处理重命名
const handleRename = (data: { folder: FolderNode, newName: string }) => {
    data.folder.name = data.newName
}
</script>

<style scoped>
.left-sidebar {
    width: 220px;
    background-color: #f8f8f8;
    border-right: 1px solid #e0e0e0;
    padding: 12px 8px;
    overflow-y: auto;
    overflow-x: hidden;
    transition: width 0.3s, padding 0.3s, opacity 0.3s;
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
    color: #333;
    display: flex;
    align-items: center;
    gap: 10px;
    transition: all 0.2s;
    height: 36px;
}

.sidebar-item:hover {
    background-color: #e8e8e8;
}

.sidebar-item:active {
    background-color: #d8d8d8;
}

.sidebar-item :deep(svg) {
    width: 16px;
    height: 16px;
    color: #666;
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
    background-color: #e0e0e0;
    margin: 8px 0;
}

</style>
