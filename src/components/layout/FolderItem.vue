<template>
    <div class="folder-item">
        <div 
            class="sidebar-item"
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
                @context-menu="forwardContextMenu"
                @folder-action="forwardFolderAction"
            />
        </div>
    </div>
</template>

<script setup lang="ts">
import { ref, nextTick } from 'vue'
import IconFolderOpen from '../icons/IconFolderOpen.vue'
import IconFolderClosed from '../icons/IconFolderClosed.vue'
import IconFile from '../icons/IconFile.vue'
import type { FolderNode, ContextMenuPosition } from '../../types/folder'

interface Props {
    folder: FolderNode
    depth: number
}

interface Emits {
    (e: 'context-menu', data: { folder: FolderNode, position: ContextMenuPosition }): void
    (e: 'folder-action', data: { action: string, folder: FolderNode }): void
    (e: 'rename', data: { folder: FolderNode, newName: string }): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

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
    // 点击展开/折叠
    if (props.folder.children.length > 0 && !isEditing.value) {
        emit('folder-action', {
            action: 'toggle',
            folder: props.folder
        })
    }
}

const handleDoubleClick = () => {
    // 双击开始编辑
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

const forwardContextMenu = (data: { folder: FolderNode, position: ContextMenuPosition }) => {
    emit('context-menu', data)
}

const forwardFolderAction = (data: { action: string, folder: FolderNode }) => {
    emit('folder-action', data)
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
    color: #333;
    display: flex;
    align-items: center;
    gap: 10px;
    user-select: none;
    height: 36px;
    transition: all 0.2s;
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

.folder-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: none;
}

.folder-name-input {
    flex: 1;
    border: 1px solid #409eff;
    border-radius: 3px;
    padding: 2px 6px;
    font-size: 13px;
    color: #333;
    outline: none;
    background-color: #fff;
}

.folder-children {
    width: 100%;
}
</style>
