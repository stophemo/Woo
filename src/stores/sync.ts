/**
 * 同步状态 Store。
 * 监听主进程通过 preload 推送的 sync:status-update 事件。
 */
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '../services/api'

export const useSyncStore = defineStore('sync', () => {
  const isSyncing = ref(false)
  const lastSyncTime = ref<string | null>(null)
  const pendingChanges = ref(0)
  const errorMsg = ref<string>('')
  let listening = false

  // 格式化的上次同步时间
  const lastSyncLabel = computed(() => {
    if (!lastSyncTime.value) return '尚未同步'
    const d = new Date(lastSyncTime.value)
    const now = new Date()
    const diff = now.getTime() - d.getTime()

    if (diff < 60000) return '刚刚'
    if (diff < 3600000) return `${Math.floor(diff / 60000)} 分钟前`
    if (diff < 86400000) return `${Math.floor(diff / 3600000)} 小时前`
    return d.toLocaleDateString('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })
  })

  /**
   * 监听主进程推送的同步状态
   */
  function listen() {
    if (listening) return
    listening = true
    window.addEventListener('sync-status', ((event: CustomEvent) => {
      const status = event.detail
      isSyncing.value = status.isSyncing ?? false
      lastSyncTime.value = status.lastSyncTime ?? null
      pendingChanges.value = status.pendingChanges ?? 0
    }) as EventListener)
  }

  /**
   * 手动触发同步
   */
  async function triggerSync(): Promise<boolean> {
    if (isSyncing.value) return false
    isSyncing.value = true
    errorMsg.value = ''
    try {
      const result = await invoke<any>('sync:trigger')
      if (result) {
        lastSyncTime.value = result.syncTime || new Date().toISOString()
        pendingChanges.value = 0
        // 手动同步和登录后同步也走统一的数据刷新事件。
        window.dispatchEvent(new CustomEvent('sync-data-changed', { detail: result }))
        return true
      }
      return false
    } catch (err: any) {
      errorMsg.value = err?.message || '同步失败'
      return false
    } finally {
      isSyncing.value = false
    }
  }

  /**
   * 获取当前同步状态（一次性查询）
   */
  async function refreshStatus() {
    try {
      const status = await invoke<any>('sync:status')
      if (status) {
        isSyncing.value = status.isSyncing ?? false
        lastSyncTime.value = status.lastSyncTime ?? null
        pendingChanges.value = status.pendingChanges ?? 0
      }
    } catch { /* ignore */ }
  }

  return {
    isSyncing,
    lastSyncTime,
    lastSyncLabel,
    pendingChanges,
    errorMsg,
    listen,
    triggerSync,
    refreshStatus
  }
})
