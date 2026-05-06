/**
 * 本地模式认证 Store。
 * 本地应用无登录注册，直接标记为已登录状态。
 */
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

export const useAuthStore = defineStore('auth', () => {
  // 本地版：直接设为已登录，无需 token
  const isLoggedIn = computed(() => true)
  const user = ref<{
    nickname: string
    avatar?: string
    username?: string
    userId?: string
    email?: string
  } | null>({
    nickname: '本地用户'
  })
  const loading = ref(false)
  const errorMsg = ref<string>('')

  // 本地版：不需要登录/登出/fetchMe
  async function login(_username?: string, _password?: string): Promise<boolean> {
    return true
  }

  async function fetchMe(): Promise<boolean> {
    return true
  }

  function logout() {
    // 本地版不支持登出
  }

  return {
    user,
    loading,
    errorMsg,
    isLoggedIn,
    login,
    fetchMe,
    logout
  }
})
