<template>
    <div class="folder-tree">
        <TransitionGroup name="folder-item" tag="div">
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
        </TransitionGroup>
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
.folder-item-enter-active {
  transition:
    opacity 0.35s cubic-bezier(0.25, 0.1, 0.25, 1),
    transform 0.35s cubic-bezier(0.25, 0.1, 0.25, 1);
}
.folder-item-leave-active {
  transition:
    opacity 0.25s cubic-bezier(0.25, 0.1, 0.25, 1),
    transform 0.25s cubic-bezier(0.25, 0.1, 0.25, 1);
  position: absolute;
  left: 0;
  right: 0;
  pointer-events: none;
}
.folder-item-move {
  transition: transform 0.35s cubic-bezier(0.25, 0.1, 0.25, 1);
}
.folder-item-enter-from {
  opacity: 0;
  transform: translateY(-8px);
}
.folder-item-leave-to {
  opacity: 0;
  transform: translateY(8px);
}
</style>
