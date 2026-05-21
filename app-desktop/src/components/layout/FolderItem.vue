<template>
    <div class="folder-item">
        <div 
            class="sidebar-item"
            :class="{ 'selected': selectedFolderId === folder.id }"
            :style="{ paddingLeft: `${depth * 16 + 8}px` }"
            @contextmenu.stop="handleContextMenu"
            @click="handleClick"
            @dblclick="handleDoubleClick"
        >
            <IconFolderOpen v-if="folder.isExpanded" />
            <IconFolderClosed v-else />
            <input 
                v-if="isEditing"
                ref="inputRef"
                v-model="editName"
                class="folder-name-input"
                @blur="finishEdit"
                @keydown.enter="finishEdit"
                @keydown.escape="cancelEdit"
            />
            <span v-else class="folder-name">{{ folder.name }}</span>
        </div>
        
        <!-- 递归渲染子目录 -->
        <div v-if="folder.isExpanded && folder.children.length > 0" class="folder-children">
            <FolderItem
                v-for="child in folder.children"
                :key="child.id"
                :folder="child"
                :depth="depth + 1"
                :selected-folder-id="selectedFolderId"
                @context-menu="forwardContextMenu"
                @folder-select="forwardFolderSelect"
                @rename="forwardRename"
            />
        </div>
    </div>
</template>

<script setup lang="ts">
import { ref, nextTick, watch } from 'vue'
import IconFolderOpen from '../icons/IconFolderOpen.vue'
import IconFolderClosed from '../icons/IconFolderClosed.vue'
import { useWorkspaceStore } from '../../stores/workspace'
import type { FolderNode, ContextMenuPosition } from '../../types/folder'

interface Props {
    folder: FolderNode
    depth: number
    selectedFolderId: string | null
}

interface Emits {
    (e: 'context-menu', data: { folder: FolderNode, position: ContextMenuPosition }): void
    (e: 'folder-select', folder: FolderNode): void
    (e: 'rename', data: { folder: FolderNode, newName: string }): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

const workspaceStore = useWorkspaceStore()

const isEditing = ref(false)
const editName = ref('')
const inputRef = ref<HTMLInputElement | null>(null)

const handleContextMenu = (event: MouseEvent) => {
    event.preventDefault()
    event.stopPropagation()
    
    emit('context-menu', {
        folder: props.folder,
        position: { x: event.clientX, y: event.clientY }
    })
}

const handleClick = () => {
    if (!isEditing.value) {
        emit('folder-select', props.folder)
    }
}

const handleDoubleClick = () => {
    startEdit()
}

const startEdit = () => {
    isEditing.value = true
    editName.value = props.folder.name
    nextTick(() => {
        if (inputRef.value) {
            inputRef.value.focus()
            inputRef.value.select()
        }
    })
}

const finishEdit = () => {
    if (isEditing.value && editName.value.trim()) {
        emit('rename', {
            folder: props.folder,
            newName: editName.value.trim()
        })
    }
    isEditing.value = false
}

const cancelEdit = () => {
    isEditing.value = false
}

// 新建目录时由 store 设置 editingFolderId，命中自己则自动进入重命名状态
watch(
    () => workspaceStore.editingFolderId,
    (id) => {
        if (id && id === props.folder.id) {
            startEdit()
            workspaceStore.clearEditingFolder()
        }
    },
    { immediate: true }
)

const forwardContextMenu = (data: { folder: FolderNode, position: ContextMenuPosition }) => {
    emit('context-menu', data)
}

const forwardFolderSelect = (folder: FolderNode) => {
    emit('folder-select', folder)
}

const forwardRename = (data: { folder: FolderNode, newName: string }) => {
    emit('rename', data)
}
</script>

<style scoped>
.folder-item {
    width: 100%;
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
    user-select: none;
    height: 36px;
    transition: all 0.2s;
}

.sidebar-item:hover {
    background-color: var(--bg-hover);
}

.sidebar-item:active {
    background-color: var(--bg-active);
}

.sidebar-item.selected {
    background-color: var(--bg-selected);
    color: var(--text-on-selected);
}

.sidebar-item.selected:hover {
    background-color: var(--bg-selected-hover);
}

.sidebar-item :deep(svg) {
    width: 16px;
    height: 16px;
    color: var(--text-secondary);
    flex-shrink: 0;
}

.sidebar-item.selected :deep(svg) {
    color: var(--text-on-selected);
}

.folder-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: none;
}

.folder-name-input {
    flex: 1;
    border: 1px solid var(--accent);
    border-radius: 3px;
    padding: 2px 6px;
    font-size: 13px;
    color: var(--text-primary);
    outline: none;
    background-color: var(--bg-elevated);
}

.folder-children {
    width: 100%;
}
</style>
