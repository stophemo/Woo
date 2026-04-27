<template>
    <div class="folder-tree">
        <FolderItem
            v-for="folder in folders"
            :key="folder.id"
            :folder="folder"
            :depth="0"
            @context-menu="forwardContextMenu"
            @folder-action="forwardFolderAction"
            @rename="forwardRename"
        />
    </div>
</template>

<script setup lang="ts">
import FolderItem from './FolderItem.vue'
import type { FolderNode, ContextMenuPosition } from '../../types/folder'

interface Props {
    folders: FolderNode[]
}

interface Emits {
    (e: 'context-menu', data: { folder: FolderNode, position: ContextMenuPosition }): void
    (e: 'folder-action', data: { action: string, folder: FolderNode }): void
    (e: 'rename', data: { folder: FolderNode, newName: string }): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

const forwardContextMenu = (data: { folder: FolderNode, position: ContextMenuPosition }) => {
    emit('context-menu', data)
}

const forwardFolderAction = (data: { action: string, folder: FolderNode }) => {
    emit('folder-action', data)
}

const forwardRename = (data: { folder: FolderNode, newName: string }) => {
    emit('rename', data)
}
</script>

<style scoped>
.folder-tree {
    width: 100%;
}
</style>
