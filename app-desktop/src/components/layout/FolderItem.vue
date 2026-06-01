<template>
    <div class="folder-item">
        <div
            class="sidebar-item"
            :class="{
                'selected': selectedFolderId === folder.id,
                'dragging': isDragging,
                'drag-over-above': dragOverState === 'above',
                'drag-over-below': dragOverState === 'below'
            }"
            :style="{ paddingLeft: `${depth * 16 + 8}px` }"
            :draggable="!folder.isLocked"
            @contextmenu.stop="handleContextMenu"
            @click="handleClick"
            @dblclick="handleDoubleClick"
            @dragstart="onDragStart"
            @dragover="onDragOver"
            @dragleave="onDragLeave"
            @drop="onDrop"
            @dragend="onDragEnd"
        >
            <Transition name="icon-swap" mode="out-in">
                <IconFolderOpen v-if="folder.isExpanded" key="open" />
                <IconFolderClosed v-else key="closed" />
            </Transition>
            <IconLock v-if="folder.isLocked" class="lock-icon" />
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
            <span class="grip-area">
                <IconGrip />
            </span>
        </div>
        
        <!-- 递归渲染子目录（带 Apple 风格展开/收起动画） -->
        <Transition name="folder-children">
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
        </Transition>
    </div>
</template>

<script setup lang="ts">
import { ref, nextTick, watch } from 'vue'
import IconFolderOpen from '../icons/IconFolderOpen.vue'
import IconFolderClosed from '../icons/IconFolderClosed.vue'
import IconLock from '../icons/IconLock.vue'
import IconGrip from '../icons/IconGrip.vue'
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

/* ========== 拖拽排序 ========== */
// 模块级状态（拖拽期间共享）
let _dragFolderId: string | null = null
let _dragParentId: string | null | undefined = undefined

const dragOverState = ref<'above' | 'below' | null>(null)
const isDragging = ref(false)

function onDragStart(event: DragEvent) {
    if (props.folder.isLocked) { event.preventDefault(); return }
    _dragFolderId = props.folder.id
    _dragParentId = props.folder.parentId
    isDragging.value = true
    event.dataTransfer!.effectAllowed = 'move'
    event.dataTransfer!.setData('text/plain', props.folder.id)
}

function onDragOver(event: DragEvent) {
    if (!_dragFolderId || _dragFolderId === props.folder.id) return
    if (_dragParentId !== props.folder.parentId) return // 非同层
    event.preventDefault()
    const el = event.currentTarget as HTMLElement
    const rect = el.getBoundingClientRect()
    dragOverState.value = event.clientY < rect.top + rect.height / 2 ? 'above' : 'below'
}

function onDragLeave(event: DragEvent) {
    const el = event.currentTarget as HTMLElement
    if (!el.contains(event.relatedTarget as Node)) {
        dragOverState.value = null
    }
}

async function onDrop(event: DragEvent) {
    event.preventDefault()
    dragOverState.value = null
    if (!_dragFolderId || _dragFolderId === props.folder.id) return
    if (_dragParentId !== props.folder.parentId) return

    // 获取同级数组
    const sibs: any[] = props.folder.parentId === null
        ? (workspaceStore as any).folders
        : ((p: any) => p ? p.children : [])(workspaceStore.findFolderById((workspaceStore as any).folders, props.folder.parentId))

    if (!sibs || sibs.length === 0) return

    const dragIdx = sibs.findIndex((f: any) => f.id === _dragFolderId)
    const dropIdx = sibs.findIndex((f: any) => f.id === props.folder.id)
    if (dragIdx === -1 || dropIdx === -1) return

    // 快照原始状态（用于回滚）
    const originalSnapshot = sibs.slice()

    // 从数组中移除拖拽项
    const [moved] = sibs.splice(dragIdx, 1)
    // 重新计算目标位置（移除后索引可能偏移）
    const newDropIdx = sibs.findIndex((f: any) => f.id === props.folder.id)
    if (newDropIdx === -1) return
    const insertIdx = dragOverState.value === 'above' ? newDropIdx : newDropIdx + 1
    sibs.splice(insertIdx, 0, moved)

    // 持久化
    try {
        await workspaceStore.reorderFolderSiblings(props.folder.parentId, sibs.map((f: any) => f.id))
    } catch {
        // 回滚
        sibs.splice(0, sibs.length, ...originalSnapshot)
    }
}

function onDragEnd() {
    _dragFolderId = null
    _dragParentId = undefined
    isDragging.value = false
    dragOverState.value = null
}

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
    position: relative;
    padding: 8px 8px;
    margin: 2px 0;
    cursor: grab;
    border-radius: 6px;
    font-size: 13px;
    color: var(--text-primary);
    display: flex;
    align-items: center;
    gap: 10px;
    user-select: none;
    height: 36px;
    transition:
        background-color 0.25s ease,
        color 0.25s ease,
        transform 0.25s cubic-bezier(0.42, 0, 0.28, 1);
}
.sidebar-item:active {
    cursor: grabbing;
}
.sidebar-item.dragging {
    opacity: 0.5;
}
.sidebar-item.dragging:active {
    transform: none;
}

/* 拖拽放置指示线 */
.sidebar-item.drag-over-above::before {
    content: '';
    position: absolute;
    top: -2px;
    left: 24px;
    right: 8px;
    height: 3px;
    background: var(--accent);
    border-radius: 2px;
    pointer-events: none;
    z-index: 1;
}
.sidebar-item.drag-over-below::after {
    content: '';
    position: absolute;
    bottom: -2px;
    left: 24px;
    right: 8px;
    height: 3px;
    background: var(--accent);
    border-radius: 2px;
    pointer-events: none;
    z-index: 1;
}

/* 拖拽手柄：默认隐藏，hover 时显示 */
.grip-area {
    position: absolute;
    right: 4px;
    top: 50%;
    transform: translateY(-50%);
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    opacity: 0;
    transition: opacity 0.2s ease;
    color: var(--text-muted);
    z-index: 2;
}
.sidebar-item:hover .grip-area {
    opacity: 0.4;
}
.sidebar-item .grip-area:hover {
    opacity: 0.8;
    background-color: var(--bg-hover);
}
.sidebar-item.selected:hover .grip-area {
    color: var(--text-on-selected);
}

.sidebar-item:hover {
    background-color: var(--bg-hover);
}

.sidebar-item:active {
    transform: scale(0.975);
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

.lock-icon {
    color: var(--accent, #409eff);
    flex-shrink: 0;
    opacity: 0.85;
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
    overflow: hidden;
}

/* ===== Apple 风格展开/收起动画 =====
 * 使用 max-height + opacity 配合 Apple 缓动曲线，
 * 营造自然的「滑动展开/收起」物理感。
 */
.folder-children-enter-active {
    transition:
        max-height 0.55s cubic-bezier(0.42, 0, 0.28, 1),
        opacity 0.35s ease;
}
.folder-children-leave-active {
    transition:
        max-height 0.45s cubic-bezier(0.42, 0, 0.28, 1),
        opacity 0.25s ease;
}
.folder-children-enter-from,
.folder-children-leave-to {
    max-height: 0;
    opacity: 0;
}
.folder-children-enter-to {
    max-height: 2000px;
    opacity: 1;
}

/* ===== 文件夹图标切换动画（Apple 风格轻量淡入淡出） ===== */
.icon-swap-enter-active {
    transition: opacity 0.25s ease, transform 0.25s ease;
}
.icon-swap-leave-active {
    transition: opacity 0.2s ease, transform 0.2s ease;
}
.icon-swap-enter-from {
    opacity: 0;
    transform: scale(0.85) rotate(-6deg);
}
.icon-swap-leave-to {
    opacity: 0;
    transform: scale(0.85) rotate(6deg);
}
</style>
