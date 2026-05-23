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
  folderName?: string
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

export function listAll(): Promise<DocumentDTO[]> {
  return invoke<DocumentDTO[]>('document:listAll')
}

export function listTrash(): Promise<DocumentDTO[]> {
  return invoke<DocumentDTO[]>('document:listTrash')
}

export function listOrphans(): Promise<DocumentDTO[]> {
  return invoke<DocumentDTO[]>('document:listOrphans')
}

export function search(keyword: string): Promise<DocumentDTO[]> {
  return invoke<DocumentDTO[]>('document:search', keyword)
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

export function restore(documentId: string): Promise<void> {
  return invoke<void>('document:restore', documentId)
}

export function hardDelete(documentId: string): Promise<void> {
  return invoke<void>('document:hardDelete', documentId)
}

export function emptyTrash(): Promise<void> {
  return invoke<void>('document:emptyTrash')
}
