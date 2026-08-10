export interface FolderNode {
  id: string
  name: string
  children: FolderNode[]
  parentId: string | null
  isExpanded?: boolean
  isLocked?: boolean
}

export interface ContextMenuPosition {
  x: number
  y: number
}

export interface ContextMenuItem {
  label: string
  action: string
  disabled?: boolean
  divider?: boolean
  shortcut?: string
}

/** 目录拖拽放置区域：above/below 为同层排序，inside 为放入下级 */
export type FolderDropZone = 'above' | 'below' | 'inside'
