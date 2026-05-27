import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as lockApi from '../services/lockApi'

export const useLockStore = defineStore('lock', () => {
  const hasPassword = ref(false)
  const passwordMode = ref<string | null>(null)
  const verifying = ref(false)
  // 当前会话中已验证过的密码缓存（避免每次操作都输密码）
  const sessionVerified = ref(false)

  async function bootstrap() {
    try {
      // 尝试从云端拉取锁密码（仅已登录用户有效）
      await lockApi.cloudPullSettings().catch(() => {})
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
      const ok = await lockApi.verifyPassword(password)
      if (ok) {
        sessionVerified.value = true
      }
      return ok
    } catch {
      return false
    } finally {
      verifying.value = false
    }
  }

  async function setPassword(password: string): Promise<void> {
    await lockApi.setPassword(password)
    // 已登录用户同步锁密码到云端
    await lockApi.cloudPushSettings(password).catch(() => {})
    hasPassword.value = true
    passwordMode.value = 'custom'
    sessionVerified.value = true
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
    sessionVerified,
    bootstrap,
    verify,
    setPassword,
    lockFolder,
    unlockFolder,
    lockDocument,
    unlockDocument
  }
})
