export interface FolderNode {
  id: string
  name: string
  children: FolderNode[]
  parentId: string | null
  isExpanded?: boolean
}

export interface ContextMenuPosition {
  x: number
  y: number
}

export interface ContextMenuItem {
  label: string
  action: string
  disabled?: boolean
}
