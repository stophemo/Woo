/**
 * 认证 Store。
 * 通过 IPC 调用主进程的 Supabase Auth 服务。
 * 状态变化由主进程 auth state change 事件 + 主动拉取双重保障。
 */
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '../services/api'

export interface AuthUser {
  id: string
  email?: string
  username?: string
  nickname?: string
  avatarUrl?: string
}

// Session 持久化：Electron 重启后恢复上次登录状态
const STORAGE_KEY = 'woo:auth'

function loadSession(): { userId: string; email: string; username: string } | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    return raw ? JSON.parse(raw) : null
  } catch {
    return null
  }
}

function persistSession(userId: string, email: string, username: string) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ userId, email, username }))
  } catch { /* ignore */ }
}

function clearSession() {
  try {
    localStorage.removeItem(STORAGE_KEY)
  } catch { /* ignore */ }
}

export const useAuthStore = defineStore('auth', () => {
  const user = ref<AuthUser | null>(null)
  const loading = ref(false)
  const errorMsg = ref<string>('')
  const initializing = ref(true) // 应用启动时正在恢复 session

  const isLoggedIn = computed(() => user.value !== null && !!user.value.id)

  // 统一映射 Supabase user 对象 → AuthUser，消除 4 处重复
  function mapUser(u: any): AuthUser {
    return {
      id: u.id,
      email: u.email,
      username: u.user_metadata?.username,
      nickname: u.user_metadata?.full_name || u.user_metadata?.username || u.email?.split('@')[0] || '用户',
      avatarUrl: u.user_metadata?.avatar_url
    }
  }

  /**
   * 启动时检查已有 session
   */
  async function bootstrap() {
    initializing.value = true
    try {
      const session = await invoke<any>('auth:getSession')
      if (session?.user) {
        user.value = mapUser(session.user)
        persistSession(session.user.id, session.user.email || '', session.user.user_metadata?.username || '')
      } else {
        // 尝试从 localStorage 恢复
        const cached = loadSession()
        if (cached) {
          // 有缓存但 session 已过期，标记为未登录
          clearSession()
        }
        user.value = null
      }
    } catch {
      user.value = null
    } finally {
      initializing.value = false
    }
  }

  /**
   * 邮箱/用户名密码登录
   */
  async function login(identifier: string, password: string): Promise<boolean> {
    loading.value = true
    errorMsg.value = ''
    try {
      const result = await invoke<any>('auth:signIn', identifier, password)
      if (!result) {
        errorMsg.value = '登录失败'
        return false
      }
      user.value = mapUser(result.user)
      persistSession(result.user.id, result.user.email || '', result.user.user_metadata?.username || '')
      return true
    } catch (err: any) {
      errorMsg.value = err?.message || '登录失败'
      return false
    } finally {
      loading.value = false
    }
  }

  /**
   * 注册新用户（使用邮箱 + 用户名 + 密码）
   */
  async function signup(email: string, username: string, password: string): Promise<{ ok: boolean; message?: string }> {
    loading.value = true
    errorMsg.value = ''
    try {
      const result = await invoke<any>('auth:signUp', email, username, password)
      if (result?.session) {
        // session 存在 → 注册成功且已自动登录
        user.value = mapUser(result.user)
        persistSession(result.user.id, result.user.email || '', result.user.user_metadata?.username || '')
        return { ok: true }
      }
      // session 为 null → 需要邮箱确认或邮箱已存在
      return { ok: true, message: '注册成功，请查看邮箱完成确认' }
    } catch (err: any) {
      errorMsg.value = err?.message || '注册失败'
      return { ok: false, message: errorMsg.value }
    } finally {
      loading.value = false
    }
  }

  /**
   * 登出
   */
  async function logout() {
    loading.value = true
    try {
      // 登出前先同步一次，避免 60s 空窗期数据未保存
      const { useSyncStore } = await import('./sync')
      const syncStore = useSyncStore()
      await syncStore.triggerSync()

      await invoke('auth:signOut')
    } catch { /* ignore */ }
    user.value = null
    clearSession()
    loading.value = false
  }

  return {
    user,
    loading,
    errorMsg,
    initializing,
    isLoggedIn,
    bootstrap,
    login,
    signup,
    logout
  }
})
