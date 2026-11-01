/**
 * 认证 Store。
 * 通过 IPC 调用主进程的 Supabase Auth 服务。
 *
 * Session 策略（重要）：
 *   - **Rust 侧的 `supabase-auth.json` 是唯一事实源**。前端 localStorage 只是
 *     一份用于首屏展示的缓存，任何情况下都不得反向清除后端会话。
 *   - 启动时直接向后端要当前用户，拿到就进入已登录状态，不等待网络。
 *   - 只有后端明确返回“无会话”或服务端撤销了 refresh token 时才登出。
 *   - 不再使用“30 天免登录”本地硬过期：那会让仍在有效期内的 Supabase 会话
 *     被本地规则强行废弃，并且旧实现还会顺手清掉后端会话文件。
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

interface SessionCache {
  userId: string
  email: string
  username: string
  loginTime: number // Date.now()，仅用于展示
}

function loadCache(): SessionCache | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return null
    const parsed = JSON.parse(raw) as Partial<SessionCache>
    if (typeof parsed.userId !== 'string' || !parsed.userId) return null
    return {
      userId: parsed.userId,
      email: typeof parsed.email === 'string' ? parsed.email : '',
      username: typeof parsed.username === 'string' ? parsed.username : '',
      loginTime: typeof parsed.loginTime === 'number' && Number.isFinite(parsed.loginTime)
        ? parsed.loginTime
        : Date.now(),
    }
  } catch {
    return null
  }
}

function persistCache(userId: string, email: string, username: string, loginTime = Date.now()) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ userId, email, username, loginTime }))
  } catch { /* ignore */ }
}

function clearCache() {
  try {
    localStorage.removeItem(STORAGE_KEY)
  } catch { /* ignore */ }
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
      avatarUrl: undefined,
    }
  }

  function adoptUser(mappedUser: AuthUser) {
    user.value = mappedUser
    persistCache(mappedUser.id, mappedUser.email || '', mappedUser.username || '')
  }

  /**
   * 启动时恢复登录态。
   *
   * 顺序：后端持久化会话 → 前端展示缓存。
   * 后端没有会话时只清理前端缓存，**不**调用 auth:clearLocalSession，
   * 避免 IPC 瞬时异常把仍然有效的登录态删掉。
   */
  async function bootstrap() {
    initializing.value = true
    try {
      const restoredUser = await invoke<any>('auth:getUser')
      if (restoredUser?.id) {
        adoptUser(mapUser(restoredUser))
        return
      }
      if (loadCache()) clearCache()
      user.value = null
    } catch {
      // IPC 尚不可用（例如启动竞态）：用缓存维持界面状态，后续 refreshSession 校正。
      const cached = loadCache()
      user.value = cached
        ? mapUser({ id: cached.userId, email: cached.email, username: cached.username })
        : null
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
        clearCache()
        user.value = null
        return false
      }
      adoptUser(mapUser(session.user))
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
        errorMsg.value = '登录失败：服务器未返回用户信息'
        return false
      }
      adoptUser(mapUser(result.user))
      return true
    } catch (err: any) {
      errorMsg.value = err?.message || '登录失败'
      return false
    } finally {
      loading.value = false
    }
  }

  async function signup(
    email: string,
    username: string,
    password: string,
  ): Promise<{ ok: boolean; message?: string }> {
    loading.value = true
    errorMsg.value = ''
    try {
      const result = await invoke<any>('auth:signUp', { email, username, password })
      if (result?.user) {
        adoptUser(mapUser(result.user))
        return { ok: true }
      }
      // 未返回会话 → 项目开启了邮箱确认，账号已创建。
      return { ok: true, message: '注册成功，请前往邮箱点击确认链接后再登录' }
    } catch (err: any) {
      errorMsg.value = err?.message || '注册失败'
      return { ok: false, message: errorMsg.value }
    } finally {
      loading.value = false
    }
  }

  async function logout() {
    loading.value = true
    // 先清除本地展示缓存，防止进程在异步操作中被终止时 session 残留。
    user.value = null
    clearCache()
    try {
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
    logout,
  }
})
