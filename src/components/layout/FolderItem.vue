<template>
    <div class="folder-item">
        <div
            class="sidebar-item"
            :class="{
                'selected': selectedFolderId === folder.id,
                'dragging': isDragging,
                'drag-over-above': dragOverState === 'above',
                'drag-over-below': dragOverState === 'below',
                'drag-over-inside': dragOverState === 'inside'
            }"
            :style="{ paddingLeft: `${depth * 16 + 10}px` }"
            :data-folder-id="folder.id"
            @contextmenu.stop="handleContextMenu"
            @click="handleClick"
            @dblclick="handleDoubleClick"
            @pointerdown="onPointerDown"
            @touchstart.passive="handleTouchStart"
            @touchend.passive="handleTouchEnd"
            @touchmove.passive="handleTouchMove"
            @touchcancel.passive="handleTouchEnd"
        >
            <span
                class="folder-color-dot"
                :class="{ expanded: folder.isExpanded }"
                :style="{ '--folder-dot-color': folderDotColor }"
                aria-hidden="true"
            ></span>
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
                    v-for="(child, childIndex) in folder.children"
                    :key="child.id"
                    :folder="child"
                    :depth="depth + 1"
                    :color-index="colorIndex + childIndex + 1"
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
import { computed, ref, nextTick, onBeforeUnmount, watch } from 'vue'
import IconLock from '../icons/IconLock.vue'
import IconGrip from '../icons/IconGrip.vue'
import { useWorkspaceStore } from '../../stores/workspace'
import {
    folderDragState,
    clearFolderDragState,
    clearFolderDropTarget,
    debugLogDrag,
    setFolderDropTarget,
} from './folderDragState'
import type { FolderNode, FolderDropZone, ContextMenuPosition } from '../../types/folder'

interface Props {
    folder: FolderNode
    depth: number
    colorIndex: number
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

const FOLDER_DOT_COLORS = [
    '#4f91a8',
    '#d47b5e',
    '#6f9b82',
    '#c39a4d',
    '#897fb0',
    '#748f9f',
    '#b9788c',
    '#4f9996'
]

const folderDotColor = computed(() => (
    FOLDER_DOT_COLORS[props.colorIndex % FOLDER_DOT_COLORS.length]
))

const isEditing = ref(false)
const editName = ref('')
const inputRef = ref<HTMLInputElement | null>(null)

// 移动端长按触发右键菜单
let longPressTimer: ReturnType<typeof setTimeout> | null = null
let touchStartX = 0
let touchStartY = 0
const LONG_PRESS_MS = 600
const LONG_PRESS_MOVE_THRESHOLD = 10

const handleTouchStart = (event: TouchEvent) => {
    const touch = event.touches[0]
    touchStartX = touch.clientX
    touchStartY = touch.clientY
    if (longPressTimer) clearTimeout(longPressTimer)
    longPressTimer = setTimeout(() => {
        longPressTimer = null
        emit('context-menu', {
            folder: props.folder,
            position: { x: touchStartX, y: touchStartY }
        })
    }, LONG_PRESS_MS)
}

const handleTouchMove = (event: TouchEvent) => {
    if (!longPressTimer) return
    const touch = event.touches[0]
    const dx = Math.abs(touch.clientX - touchStartX)
    const dy = Math.abs(touch.clientY - touchStartY)
    if (dx > LONG_PRESS_MOVE_THRESHOLD || dy > LONG_PRESS_MOVE_THRESHOLD) {
        clearTimeout(longPressTimer)
        longPressTimer = null
    }
}

const handleTouchEnd = () => {
    if (longPressTimer) {
        clearTimeout(longPressTimer)
        longPressTimer = null
    }
}

const handleContextMenu = (event: MouseEvent) => {
    event.preventDefault()
    event.stopPropagation()
    
    emit('context-menu', {
        folder: props.folder,
        position: { x: event.clientX, y: event.clientY }
    })
}

const handleClick = () => {
    if (suppressClick) {
        suppressClick = false
        return
    }
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

/* ========== 拖拽排序 / 跨层移动 ========== */
const POINTER_DRAG_THRESHOLD = 6
const dragOverState = computed<FolderDropZone | null>(() => (
    folderDragState.dragTargetId === props.folder.id ? folderDragState.dropZone : null
))
const isDragging = ref(false)
let pointerId: number | null = null
let pointerStartX = 0
let pointerStartY = 0
let pointerDragging = false
let suppressClick = false

function getDropZone(clientY: number, rect: DOMRect): FolderDropZone {
    const ratio = (clientY - rect.top) / rect.height
    if (ratio < 0.28) return 'above'
    if (ratio > 0.72) return 'below'
    return 'inside'
}

function onPointerDown(event: PointerEvent) {
    if (event.pointerType !== 'mouse' || event.button !== 0 || props.folder.isLocked || isEditing.value) return
    if ((event.target as HTMLElement).closest('input')) return

    pointerId = event.pointerId
    pointerStartX = event.clientX
    pointerStartY = event.clientY
    pointerDragging = false
    window.addEventListener('pointermove', onPointerMove, { passive: false })
    window.addEventListener('pointerup', onPointerUp)
    window.addEventListener('pointercancel', onPointerCancel)
}

function onPointerMove(event: PointerEvent) {
    if (event.pointerId !== pointerId) return

    if (!pointerDragging) {
        const distance = Math.hypot(event.clientX - pointerStartX, event.clientY - pointerStartY)
        if (distance < POINTER_DRAG_THRESHOLD) return
        pointerDragging = true
        suppressClick = true
        folderDragState.dragFolderId = props.folder.id
        isDragging.value = true
        document.body.classList.add('folder-pointer-dragging')
        debugLogDrag('start', props.folder.id, props.folder.id, null)
    }

    event.preventDefault()
    const targetRow = document
        .elementFromPoint(event.clientX, event.clientY)
        ?.closest<HTMLElement>('.sidebar-item[data-folder-id]')
    const targetId = targetRow?.dataset.folderId
    if (!targetRow || !targetId || targetId === props.folder.id) {
        clearFolderDropTarget()
        return
    }

    setFolderDropTarget(targetId, getDropZone(event.clientY, targetRow.getBoundingClientRect()))
}

function stopPointerTracking() {
    window.removeEventListener('pointermove', onPointerMove)
    window.removeEventListener('pointerup', onPointerUp)
    window.removeEventListener('pointercancel', onPointerCancel)
    pointerId = null
    pointerDragging = false
    isDragging.value = false
    document.body.classList.remove('folder-pointer-dragging')
}

async function onPointerUp(event: PointerEvent) {
    if (event.pointerId !== pointerId) return
    const wasDragging = pointerDragging
    const dragId = folderDragState.dragFolderId
    const targetId = folderDragState.dragTargetId
    const zone = folderDragState.dropZone
    stopPointerTracking()

    if (!wasDragging) return
    event.preventDefault()
    debugLogDrag('drop', dragId, targetId, zone)
    if (dragId && targetId && zone) {
        await workspaceStore.moveFolderRelative(dragId, targetId, zone)
    }
    clearFolderDragState()
    setTimeout(() => { suppressClick = false }, 0)
}

function onPointerCancel(event: PointerEvent) {
    if (event.pointerId !== pointerId) return
    stopPointerTracking()
    clearFolderDragState()
}

onBeforeUnmount(() => {
    stopPointerTracking()
    if (folderDragState.dragFolderId === props.folder.id) clearFolderDragState()
})

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
    padding: 0 10px;
    margin: 2px 0;
    cursor: grab;
    border-radius: 6px;
    font-size: 13px;
    color: var(--text-secondary);
    display: flex;
    align-items: center;
    gap: 10px;
    user-select: none;
    height: 38px;
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

:global(body.folder-pointer-dragging),
:global(body.folder-pointer-dragging *) {
    cursor: grabbing !important;
    user-select: none !important;
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

/* 拖入下级：整行高亮，示意将变成目标的子目录 */
.sidebar-item.drag-over-inside {
    background-color: color-mix(in srgb, var(--accent) 18%, transparent);
    box-shadow: inset 0 0 0 2px var(--accent);
    border-radius: 6px;
}
.sidebar-item.drag-over-inside .folder-name {
    color: var(--accent);
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
    font-weight: 600;
}

.sidebar-item.selected:hover {
    background-color: var(--bg-selected-hover);
}

.folder-color-dot {
    width: 7px;
    height: 7px;
    flex-shrink: 0;
    border-radius: 2px;
    background-color: var(--folder-dot-color);
    transition: background-color 0.2s ease, box-shadow 0.2s ease;
}

.folder-color-dot.expanded {
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--folder-dot-color) 11%, transparent);
}

.sidebar-item.selected .folder-color-dot {
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--folder-dot-color) 22%, transparent);
}

.sidebar-item :deep(svg) {
    width: 16px;
    height: 16px;
    color: var(--text-secondary);
    stroke-width: 1.7;
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
