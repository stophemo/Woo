/**
 * 文稿相关接口（本地版，走 IPC）
 */
import { invoke } from './api'

export interface DocumentDTO {
  id: string
  userId: string
  folderId: string
  title: string
  content?: string
  branchName?: string
  sortOrder: number
  createTime: string
  updateTime: string
}

export interface CreateDocumentPayload {
  title: string
  folderId: string
}

export function listByFolder(folderId: string): Promise<DocumentDTO[]> {
  return invoke<DocumentDTO[]>('document:listByFolder', folderId)
}

export function getById(documentId: string): Promise<DocumentDTO> {
  return invoke<DocumentDTO>('document:get', documentId)
}

export function create(payload: CreateDocumentPayload): Promise<DocumentDTO> {
  return invoke<DocumentDTO>('document:create', payload)
}

export function rename(documentId: string, title: string): Promise<void> {
  return invoke<void>('document:rename', documentId, title)
}

export function updateContent(documentId: string, content: string): Promise<void> {
  return invoke<void>('document:updateContent', documentId, content)
}

export function remove(documentId: string): Promise<void> {
  return invoke<void>('document:remove', documentId)
}
