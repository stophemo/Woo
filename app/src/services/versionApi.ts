/**
 * 文稿版本接口
 */
import api from './api'

export interface DocumentVersionSummary {
  id: string
  documentId: string
  versionNo: number
  title: string
  changeType: 'auto' | 'manual' | 'restore'
  operatorId: string | null
  createTime: string
  /** 内容预览（纯文本） */
  preview: string
}

export interface DocumentVersionDetail extends DocumentVersionSummary {
  content: string
  contentHash: string
  userId: string
}

/** 获取版本列表（不含正文） */
export function listVersions(documentId: string): Promise<DocumentVersionSummary[]> {
  return api.get(`/api/documents/${documentId}/versions`)
}

/** 获取某版本的完整内容 */
export function getVersion(documentId: string, versionNo: number): Promise<DocumentVersionDetail> {
  return api.get(`/api/documents/${documentId}/versions/${versionNo}`)
}

/** 将当前文稿手动保存为一个新版本 */
export function saveManualVersion(documentId: string): Promise<void> {
  return api.post(`/api/documents/${documentId}/versions`)
}

/**
 * 前端触发的自动版本提交：失焦 / 变更量达阈值 / 停顿 3s。
 * 调用前请确保当前文稿正文已 flush 到后端。
 */
export function commitVersion(documentId: string, changeType: 'auto' | 'manual' = 'auto'): Promise<void> {
  return api.post(`/api/documents/${documentId}/versions/commit?changeType=${changeType}`)
}

/** 回滚到指定版本，返回新生成的 restore 版本 */
export function restoreVersion(documentId: string, versionNo: number): Promise<DocumentVersionDetail> {
  return api.post(`/api/documents/${documentId}/versions/${versionNo}/restore`)
}
