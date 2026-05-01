/**
 * API 客户端：统一封装 axios 实例，处理鉴权、响应格式和错误。
 *
 * 响应约定：后端统一包装为 { code, message, data }，
 * 拦截器会自动解包并在失败时抛错。
 */
import axios, { AxiosError, AxiosInstance, AxiosResponse } from 'axios'

export interface ApiEnvelope<T = unknown> {
  code: number
  message: string
  data: T
}

// 可通过 Vite 环境变量配置，默认指向本地网关
const BASE_URL = (import.meta.env.VITE_API_BASE_URL as string | undefined) || 'http://localhost:8080'

const TOKEN_KEY = 'nonego-auth-token'

export function getStoredToken(): string {
  return localStorage.getItem(TOKEN_KEY) || ''
}

export function setStoredToken(token: string) {
  if (token) localStorage.setItem(TOKEN_KEY, token)
  else localStorage.removeItem(TOKEN_KEY)
}

export function clearStoredToken() {
  localStorage.removeItem(TOKEN_KEY)
}

const instance: AxiosInstance = axios.create({
  baseURL: BASE_URL,
  timeout: 15000,
  headers: {
    'Content-Type': 'application/json'
  }
})

// 请求拦截：注入 Token
instance.interceptors.request.use(config => {
  const token = getStoredToken()
  if (token) {
    config.headers = config.headers ?? {}
    config.headers.Authorization = `Bearer ${token}`
  }
  return config
})

// 响应拦截：解包数据 / 统一错误
instance.interceptors.response.use(
  (resp: AxiosResponse<ApiEnvelope>) => {
    const body = resp.data
    if (body && typeof body === 'object' && 'code' in body) {
      if (body.code === 200) {
        return body.data as any
      }
      // 业务错误
      const err: any = new Error(body.message || '请求失败')
      err.code = body.code
      err.response = resp
      throw err
    }
    return resp.data as any
  },
  (error: AxiosError<ApiEnvelope>) => {
    // HTTP 401：Token 失效，清空本地登录态
    if (error.response?.status === 401) {
      clearStoredToken()
      // 通知登录态失效
      window.dispatchEvent(new CustomEvent('auth:unauthorized'))
    }
    const body = error.response?.data
    const message =
      (body && typeof body === 'object' && 'message' in body && (body as any).message) ||
      error.message ||
      '网络请求失败'
    const err: any = new Error(message)
    err.status = error.response?.status
    err.code = (body as any)?.code
    throw err
  }
)

export default instance
