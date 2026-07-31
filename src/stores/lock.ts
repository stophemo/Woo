import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as lockApi from '../services/lockApi'

export const useLockStore = defineStore('lock', () => {
  const hasPassword = ref(false)
  const passwordMode = ref<string | null>(null)
  const verifying = ref(false)

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
      // 旧版本没有同步锁设置；一次成功验证可安全确认本机哈希可信并补种云端。
      if (ok) await lockApi.cloudPushSettings().catch(() => {})
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
    await lockApi.cloudPushSettings().catch(() => {})
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
