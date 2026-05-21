/**
 * 目录相关接口（本地版，走 IPC）
 */
import { invoke } from './api'

export interface FolderTreeNodeDTO {
  id: string
  parentId: string | null
  name: string
  sortOrder: number
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
  return invoke<void>('folder:rename', folderId, name)
}

export function remove(folderId: string): Promise<void> {
  return invoke<void>('folder:remove', folderId)
}
