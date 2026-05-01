/**
 * 文稿相关接口
 */
import api from './api'

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
  return api.get('/api/documents', { params: { folderId } })
}

export function getById(documentId: string): Promise<DocumentDTO> {
  return api.get(`/api/documents/${documentId}`)
}

export function create(payload: CreateDocumentPayload): Promise<DocumentDTO> {
  return api.post('/api/documents', payload)
}

export function rename(documentId: string, title: string): Promise<void> {
  return api.put(`/api/documents/${documentId}/rename`, null, { params: { title } })
}

export function updateContent(documentId: string, content: string): Promise<void> {
  return api.put(`/api/documents/${documentId}/content`, { content })
}

export function remove(documentId: string): Promise<void> {
  return api.delete(`/api/documents/${documentId}`)
}
