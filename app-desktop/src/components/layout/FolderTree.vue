<template>
    <div class="folder-tree">
        <FolderItem
            v-for="folder in folders"
            :key="folder.id"
            :folder="folder"
            :depth="0"
            :selected-folder-id="selectedFolderId"
            @context-menu="forwardContextMenu"
            @folder-select="forwardFolderSelect"
            @rename="forwardRename"
        />
    </div>
</template>

<script setup lang="ts">
import FolderItem from './FolderItem.vue'
import type { FolderNode, ContextMenuPosition } from '../../types/folder'

interface Props {
    folders: FolderNode[]
    selectedFolderId: string | null
}

interface Emits {
    (e: 'context-menu', data: { folder: FolderNode, position: ContextMenuPosition }): void
    (e: 'folder-select', folder: FolderNode): void
    (e: 'rename', data: { folder: FolderNode, newName: string }): void
}

defineProps<Props>()
const emit = defineEmits<Emits>()

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
</style>
