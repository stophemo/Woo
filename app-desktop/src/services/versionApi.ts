/**
 * 文稿版本接口（本地版，走 IPC）
 */
import { invoke } from './api'

export interface DocumentVersionSummary {
  id: string
  documentId: string
  versionNo: number
  title: string
  changeType: 'auto' | 'manual' | 'restore'
  operatorId: string | null
  createTime: string
  preview: string
}

export interface DocumentVersionDetail extends DocumentVersionSummary {
  content: string
  contentHash: string
}

export function listVersions(documentId: string): Promise<DocumentVersionSummary[]> {
  return invoke<DocumentVersionSummary[]>('version:list', documentId)
}

export function getVersion(documentId: string, versionNo: number): Promise<DocumentVersionDetail> {
  return invoke<DocumentVersionDetail>('version:get', documentId, versionNo)
}

export function saveManualVersion(documentId: string): Promise<void> {
  return invoke<void>('version:saveManual', documentId)
}

export function commitVersion(documentId: string, changeType: 'auto' | 'manual' = 'auto'): Promise<void> {
  return invoke<void>('version:commit', documentId, changeType)
}

export function restoreVersion(documentId: string, versionNo: number): Promise<DocumentVersionDetail> {
  return invoke<DocumentVersionDetail>('version:restore', documentId, versionNo)
}
