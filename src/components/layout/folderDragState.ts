/**
 * 目录拖拽期间跨 FolderItem 实例共享的模块级状态。
 *
 * 注意：不能放在 <script setup> 顶层——那里每个组件实例都有独立的 setup 局部变量，
 * 拖拽源与放置目标是两个不同实例，目标实例读不到源实例写入的字段。
 */
import { reactive } from 'vue'
import { log } from '../../services/logger'
import type { FolderDropZone } from '../../types/folder'

export const folderDragState = reactive({
  dragFolderId: null as string | null,
  dragTargetId: null as string | null,
  dropZone: null as FolderDropZone | null,
})

export function setFolderDropTarget(targetId: string, zone: FolderDropZone) {
  folderDragState.dragTargetId = targetId
  folderDragState.dropZone = zone
}

export function clearFolderDropTarget() {
  folderDragState.dragTargetId = null
  folderDragState.dropZone = null
}

export function clearFolderDragState() {
  folderDragState.dragFolderId = null
  clearFolderDropTarget()
}

/** 仅在显式启用 debug 日志时记录拖拽事件。 */
export function debugLogDrag(evt: string, dragId: string | null, targetId: string | null, zone: string | null) {
  log.app.debug(`[folder-drag] ${evt} drag=${dragId} target=${targetId} zone=${zone}`)
}
