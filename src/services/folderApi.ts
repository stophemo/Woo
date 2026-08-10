/**
 * 目录相关接口（本地版，走 IPC）
 */
import { invoke } from './api'

export interface FolderTreeNodeDTO {
  id: string
  parentId: string | null
  name: string
  sortOrder: number
  isLocked: boolean
  children: FolderTreeNodeDTO[] | null
}

export interface CreateFolderPayload {
  name: string
  parentId: string | null
}

export function getTree(): Promise<FolderTreeNodeDTO[]> {
  return invoke<FolderTreeNodeDTO[]>('folder:tree')
}

export function create(payload: CreateFolderPayload): Promise<string> {
  return invoke<string>('folder:create', payload)
}

export function rename(folderId: string, name: string): Promise<void> {
  return invoke<void>('folder:rename', { folderId, name })
}

export function remove(folderId: string): Promise<void> {
  return invoke<void>('folder:remove', { folderId })
}

export interface ReorderItem {
  id: string
  sortOrder: number
}

/** 批量更新同级目录排序 */
export function reorderFolders(parentId: string | null, items: ReorderItem[]): Promise<void> {
  return invoke<void>('folder:reorder', { parentId, items })
}

/**
 * 把目录移动到新父级（跨层或同层重排）并放到指定位置。
 * @param newParentId null 表示移到根层级
 * @param position 同级内的插入位置索引；省略表示追加到末尾
 */
export function moveFolder(
  folderId: string,
  newParentId: string | null,
  position?: number,
): Promise<void> {
  return invoke<void>('folder:move', {
    folderId,
    newParentId,
    position: position ?? null,
  })
}
