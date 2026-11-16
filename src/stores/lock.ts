import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as lockApi from '../services/lockApi'

export const useLockStore = defineStore('lock', () => {
  const hasPassword = ref(false)
  const passwordMode = ref<string | null>(null)
  const verifying = ref(false)

  async function bootstrap() {
    try {
      // 锁密码只保存在本机；WebDAV 同步不搬运密码哈希。
      const status = await lockApi.getStatus()
      hasPassword.value = status.hasPassword
      passwordMode.value = status.mode
    } catch {
      hasPassword.value = false
      passwordMode.value = null
    }
  }

  async function verify(password: string): Promise<boolean> {
    verifying.value = true
    try {
      return await lockApi.verifyPassword(password)
    } catch {
      return false
    } finally {
      verifying.value = false
    }
  }

  async function setPassword(password: string): Promise<void> {
    await lockApi.setPassword(password)
    hasPassword.value = true
    passwordMode.value = 'custom'
  }

  async function lockFolder(folderId: string): Promise<void> {
    await lockApi.lockFolder(folderId)
  }

  async function unlockFolder(folderId: string): Promise<void> {
    await lockApi.unlockFolder(folderId)
  }

  async function lockDocument(documentId: string): Promise<void> {
    await lockApi.lockDocument(documentId)
  }

  async function unlockDocument(documentId: string): Promise<void> {
    await lockApi.unlockDocument(documentId)
  }

  return {
    hasPassword,
    passwordMode,
    verifying,
    bootstrap,
    verify,
    setPassword,
    lockFolder,
    unlockFolder,
    lockDocument,
    unlockDocument
  }
})
