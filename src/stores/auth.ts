/**
 * 认证 Store。
 * 通过 IPC 调用主进程的 Supabase Auth 服务。
 *
 * Session 策略：
 *   - 正常登录后持久化到 localStorage
 *   - 应用启动时优先尝试 Supabase getSession 恢复
 *   - 若 Supabase 恢复失败但距离上次登录 ≤7 天，降级使用缓存凭据
 *   - 超过 7 天未登录才要求重新登录
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

const STORAGE_KEY = 'woo:auth'
const SESSION_DAYS = 7

interface SessionData {
  userId: string
  email: string
  username: string
  loginTime: number // Date.now()
}

function loadSession(): SessionData | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    return raw ? JSON.parse(raw) : null
  } catch {
    return null
  }
}

function persistSession(userId: string, email: string, username: string) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ userId, email, username, loginTime: Date.now() }))
  } catch { /* ignore */ }
}

function clearSession() {
  try {
    localStorage.removeItem(STORAGE_KEY)
  } catch { /* ignore */ }
}

/** 检查缓存的 session 是否在有效天数内 */
function isSessionValid(cached: SessionData): boolean {
  const elapsed = Date.now() - cached.loginTime
  return elapsed < SESSION_DAYS * 24 * 60 * 60 * 1000
}

export const useAuthStore = defineStore('auth', () => {
  const user = ref<AuthUser | null>(null)
  const loading = ref(false)
  const errorMsg = ref<string>('')
  const initializing = ref(true)

  const isLoggedIn = computed(() => user.value !== null && !!user.value.id)

  function mapUser(u: any): AuthUser {
    return {
      id: u.id,
      email: u.email,
      username: u.username,
      nickname: u.username || u.email?.split('@')[0] || '用户',
      avatarUrl: undefined
    }
  }

  /**
   * 启动时尝试恢复 session。
   *
   * 路径：
   *   1. Supabase getSession（主路径）→ 成功则恢复
   *   2. localStorage 缓存（7天内降级）→ 失败则显示登录
   */
  async function bootstrap() {
    initializing.value = true

    try {
      // 路径 A：Supabase session 恢复
      const session = await invoke<any>('auth:getSession')
      if (session?.user) {
        user.value = mapUser(session.user)
        persistSession(session.user.id, session.user.email || '', session.user.username || '')
        initializing.value = false
        return
      }
    } catch {
      // 网络错误 / Supabase 不可达，走降级
    }

    // 路径 B：7 天内缓存降级
    const cached = loadSession()
    if (cached && isSessionValid(cached)) {
      user.value = { id: cached.userId, email: cached.email, username: cached.username }
      initializing.value = false
      return
    }

    // 缓存过期或无缓存
    clearSession()
    user.value = null
    initializing.value = false
  }

  async function login(identifier: string, password: string): Promise<boolean> {
    loading.value = true
    errorMsg.value = ''
    try {
      const result = await invoke<any>('auth:signIn', { email: identifier, password })
      if (!result?.user) {
        errorMsg.value = '登录失败'
        return false
      }
      user.value = mapUser(result.user)
      persistSession(result.user.id, result.user.email || '', result.user.username || '')
      return true
    } catch (err: any) {
      errorMsg.value = err?.message || '登录失败'
      return false
    } finally {
      loading.value = false
    }
  }

  async function signup(email: string, username: string, password: string): Promise<{ ok: boolean; message?: string }> {
    loading.value = true
    errorMsg.value = ''
    try {
      const result = await invoke<any>('auth:signUp', { email, username, password })
      if (result?.user) {
        user.value = mapUser(result.user)
        persistSession(result.user.id, result.user.email || '', result.user.username || '')
        return { ok: true }
      }
      return { ok: true, message: '注册成功，请查看邮箱完成确认' }
    } catch (err: any) {
      errorMsg.value = err?.message || '注册失败'
      return { ok: false, message: errorMsg.value }
    } finally {
      loading.value = false
    }
  }

  async function logout() {
    loading.value = true
    // 先清除本地缓存，防止进程在异步操作中被终止时 session 残留
    user.value = null
    clearSession()
    try {
      const { useSyncStore } = await import('./sync')
      const syncStore = useSyncStore()
      await syncStore.triggerSync()
      await invoke('auth:signOut')
    } catch { /* ignore */ }
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
