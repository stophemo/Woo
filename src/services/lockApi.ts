import { invoke } from './api'

export interface LockStatus {
  hasPassword: boolean
  mode: string | null
}

export function getStatus(): Promise<LockStatus> {
  return invoke<LockStatus>('lock:status')
}

export function setPassword(password: string): Promise<void> {
  return invoke<void>('lock:setPassword', { password })
}

export function verifyPassword(password: string): Promise<boolean> {
  return invoke<boolean>('lock:verifyPassword', { password })
}

export function lockFolder(folderId: string): Promise<void> {
  return invoke<void>('lock:lockFolder', { folderId })
}

export function unlockFolder(folderId: string): Promise<void> {
  return invoke<void>('lock:unlockFolder', { folderId })
}

export function isFolderLocked(folderId: string): Promise<boolean> {
  return invoke<boolean>('lock:isFolderLocked', { folderId })
}

export function isFolderEffectivelyLocked(folderId: string): Promise<boolean> {
  return invoke<boolean>('lock:isFolderEffectivelyLocked', { folderId })
}

export function lockDocument(documentId: string): Promise<void> {
  return invoke<void>('lock:lockDocument', { documentId })
}

export function unlockDocument(documentId: string): Promise<void> {
  return invoke<void>('lock:unlockDocument', { documentId })
}

export function isDocumentLocked(documentId: string): Promise<boolean> {
  return invoke<boolean>('lock:isDocumentLocked', { documentId })
}

export function cloudPushSettings(password: string): Promise<void> {
  return invoke<void>('lock:cloudPushSettings', { password })
}

export function cloudPullSettings(): Promise<void> {
  return invoke<void>('lock:cloudPullSettings')
}
