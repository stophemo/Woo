/**
 * 同步状态 Store。
 * 监听主进程通过 preload 推送的 sync:status-update 事件。
 */
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '../services/api'

export type SyncBackend = 'webdav' | 'lan' | 'off'

export const useSyncStore = defineStore('sync', () => {
  const isSyncing = ref(false)
  const lastSyncTime = ref<string | null>(null)
  const pendingChanges = ref(0)
  const backend = ref<SyncBackend>('off')
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
   * 监听主进程推送的同步状态，并接管「什么时候该补一轮同步」：
   * - 本地保存/目录变更：1.5 秒防抖后触发，连续输入不会每键同步；
   * - 回到前台（手机切回、桌面窗口重新可见/聚焦）：立即补一轮，
   *   这样另一台设备的最新修改不必等兜底轮询；
   * - 自动触发之间至少间隔 6 秒（局域网）/ 15 秒（WebDAV），
   *   避免短时间反复请求网盘配额。
   */
  let autoTimer: ReturnType<typeof setTimeout> | null = null
  let lastAutoTriggerAt = 0
  const AUTO_SYNC_DEBOUNCE_MS = 1500
  const AUTO_SYNC_MIN_GAP_LAN_MS = 6000
  const AUTO_SYNC_MIN_GAP_WEBDAV_MS = 15000

  function listen() {
    if (listening) return
    listening = true
    window.addEventListener('sync-status', ((event: CustomEvent) => {
      const status = event.detail
      isSyncing.value = status.isSyncing ?? false
      lastSyncTime.value = status.lastSyncTime ?? null
      pendingChanges.value = status.pendingChanges ?? 0
      backend.value = status.backend ?? 'off'
    }) as EventListener)

    window.addEventListener('woo:local-change', () => scheduleSync())
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState !== 'visible') return
      void refreshStatus().then(() => scheduleSync(0))
    })
    window.addEventListener('focus', () => {
      if (backend.value !== 'off') scheduleSync(0)
    })
  }

  /**
   * 安排一轮自动同步。0 表示尽快，但仍受最小间隔约束。
   */
  function scheduleSync(delay = AUTO_SYNC_DEBOUNCE_MS) {
    if (backend.value === 'off') return
    if (autoTimer) clearTimeout(autoTimer)
    const minGap = backend.value === 'lan' ? AUTO_SYNC_MIN_GAP_LAN_MS : AUTO_SYNC_MIN_GAP_WEBDAV_MS
    const sinceLast = Date.now() - lastAutoTriggerAt
    const wait = Math.max(delay, minGap - sinceLast)
    autoTimer = setTimeout(() => {
      autoTimer = null
      if (backend.value === 'off') return
      if (isSyncing.value) {
        scheduleSync(3000)
        return
      }
      lastAutoTriggerAt = Date.now()
      void triggerSync().then((ok) => {
        // 极小概率与兜底轮询撞车：等这一轮结束后再补一次。
        if (!ok && errorMsg.value.includes('正在进行')) scheduleSync(3000)
      })
    }, wait)
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
        // 手动同步和启动同步都走统一的数据刷新事件。
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
        backend.value = status.backend ?? 'off'
      }
    } catch { /* ignore */ }
  }

  return {
    isSyncing,
    lastSyncTime,
    lastSyncLabel,
    pendingChanges,
    backend,
    errorMsg,
    listen,
    scheduleSync,
    triggerSync,
    refreshStatus
  }
})
