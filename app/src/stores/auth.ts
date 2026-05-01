/**
 * 登录态 Store：管理 Token 与当前用户信息。
 */
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import * as authApi from '../services/authApi'
import { getStoredToken, setStoredToken, clearStoredToken } from '../services/api'

export const useAuthStore = defineStore('auth', () => {
  const token = ref<string>(getStoredToken())
  const user = ref<authApi.UserInfo | null>(null)
  const loading = ref(false)
  const errorMsg = ref<string>('')

  const isLoggedIn = computed(() => !!token.value)

  async function login(username: string, password: string): Promise<boolean> {
    loading.value = true
    errorMsg.value = ''
    try {
      const result = await authApi.login({ username, password })
      token.value = result.accessToken
      setStoredToken(result.accessToken)
      user.value = {
        userId: result.userId,
        username: result.username,
        nickname: result.nickname
      }
      return true
    } catch (e: any) {
      errorMsg.value = e?.message || '登录失败'
      return false
    } finally {
      loading.value = false
    }
  }

  /**
   * 根据 Token 拉取当前用户信息，用于启动时校验登录态
   */
  async function fetchMe(): Promise<boolean> {
    if (!token.value) return false
    try {
      const info = await authApi.me()
      user.value = info
      return true
    } catch {
      logout()
      return false
    }
  }

  function logout() {
    token.value = ''
    user.value = null
    clearStoredToken()
  }

  return {
    token,
    user,
    loading,
    errorMsg,
    isLoggedIn,
    login,
    fetchMe,
    logout
  }
})
