/**
 * 认证 Store。
 * 通过 IPC 调用主进程的 Supabase Auth 服务。
 *
 * Session 策略：
 *   - 正常登录后持久化到 localStorage
 *   - 应用启动时先恢复本地缓存和后端已持久化的 session，不等待网络
 *   - 首屏加载完成后再由调用方执行 Supabase 校验与同步
 *   - 30 天内免登录；超过期限后清除本地 session
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
const SESSION_DAYS = 30
const SESSION_MAX_AGE_MS = SESSION_DAYS * 24 * 60 * 60 * 1000

interface SessionData {
  userId: string
  email: string
  username: string
  loginTime: number // Date.now()
}

function loadSession(): SessionData | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return null
    const parsed = JSON.parse(raw) as Partial<SessionData>
    if (
      typeof parsed.userId !== 'string' || !parsed.userId ||
      typeof parsed.email !== 'string' ||
      typeof parsed.username !== 'string' ||
      typeof parsed.loginTime !== 'number' || !Number.isFinite(parsed.loginTime)
    ) {
      return null
    }
    return parsed as SessionData
  } catch {
    return null
  }
}

function persistSession(userId: string, email: string, username: string, loginTime = Date.now()) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ userId, email, username, loginTime }))
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
  return elapsed >= 0 && elapsed < SESSION_MAX_AGE_MS
}

export const useAuthStore = defineStore('auth', () => {
  const user = ref<AuthUser | null>(null)
  const loading = ref(false)
  const errorMsg = ref<string>('')
  const initializing = ref(true)

  const isLoggedIn = computed(() => user.value !== null && !!user.value.id)

  function normalizeUsername(u: any): string | undefined {
    const candidate = typeof u?.username === 'string' ? u.username.trim() : ''
    const userId = typeof u?.id === 'string' ? u.id.trim() : ''
    const looksLikeUuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(candidate)
    if (candidate && candidate !== userId && !looksLikeUuid) return candidate
    const email = typeof u?.email === 'string' ? u.email.trim() : ''
    return email ? email.split('@')[0] : undefined
  }

  function mapUser(u: any): AuthUser {
    const username = normalizeUsername(u)
    return {
      id: u.id,
      email: u.email,
      username,
      nickname: username || '用户',
      avatarUrl: undefined
    }
  }

  function persistRestoredUser(restoredUser: AuthUser) {
    const cached = loadSession()
    const loginTime = cached?.userId === restoredUser.id && isSessionValid(cached)
      ? cached.loginTime
      : Date.now()
    persistSession(
      restoredUser.id,
      restoredUser.email || '',
      restoredUser.username || '',
      loginTime
    )
  }

  /**
   * 启动时尝试恢复 session。
   *
   * 路径：
   *   1. localStorage 立即恢复登录展示
   *   2. 后端内存 session 确认账号并保证用户数据库已激活
   *   3. 不在此处访问网络，避免阻塞本地工作区
   */
  async function bootstrap() {
    initializing.value = true
    const cached = loadSession()
    if (!cached || !isSessionValid(cached)) {
      clearSession()
      user.value = null
      try {
        await invoke('auth:clearLocalSession')
      } catch { /* ignore */ }
      initializing.value = false
      return
    }

    user.value = mapUser({ id: cached.userId, email: cached.email, username: cached.username })

    try {
      // 仅读取 Tauri 启动时已恢复的本地 session，不访问 Supabase。
      const restoredUser = await invoke<any>('auth:getUser')
      if (restoredUser?.id === cached.userId) {
        const mappedUser = mapUser(restoredUser)
        user.value = mappedUser
        persistRestoredUser(mappedUser)
      } else {
        clearSession()
        user.value = null
        await invoke('auth:clearLocalSession').catch(() => {})
      }
    } catch {
      // IPC 暂不可用时保留仍在 30 天期限内的前端缓存。
    } finally {
      initializing.value = false
    }
  }

  /** 首屏加载完成后校验/刷新 Supabase session。离线时后端会保留本地 session。 */
  async function refreshSession(): Promise<boolean> {
    const expectedUserId = user.value?.id
    if (!expectedUserId) return false
    try {
      const session = await invoke<any>('auth:getSession')
      // 用户可能在后台请求期间主动登出或切换账号，旧请求不能覆盖新状态。
      if (user.value?.id !== expectedUserId) return isLoggedIn.value
      if (!session?.user) {
        clearSession()
        user.value = null
        return false
      }
      const mappedUser = mapUser(session.user)
      user.value = mappedUser
      persistRestoredUser(mappedUser)
      return true
    } catch {
      return isLoggedIn.value
    }
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
    refreshSession,
    login,
    signup,
    logout
  }
})
