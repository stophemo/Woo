/**
 * 目录相关接口
 */
import api from './api'

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
  return api.get('/api/folders')
}

export function create(payload: CreateFolderPayload): Promise<string> {
  return api.post('/api/folders', payload)
}

export function rename(folderId: string, name: string): Promise<void> {
  return api.put(`/api/folders/${folderId}/rename`, null, { params: { name } })
}

export function remove(folderId: string): Promise<void> {
  return api.delete(`/api/folders/${folderId}`)
}
