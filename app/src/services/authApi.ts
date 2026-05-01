/**
 * 认证相关接口
 */
import api from './api'

export interface LoginPayload {
  username: string
  password: string
}

export interface LoginResult {
  accessToken: string
  tokenType: string
  expiresIn: number
  userId: string
  username: string
  nickname: string
}

export interface RegisterPayload {
  username: string
  password: string
  nickname?: string
  email?: string
}

export interface UserInfo {
  userId: string
  username: string
  nickname: string
  email?: string
  avatar?: string
}

export function login(payload: LoginPayload): Promise<LoginResult> {
  return api.post('/api/auth/login', payload)
}

export function register(payload: RegisterPayload): Promise<void> {
  return api.post('/api/auth/register', payload)
}

export function me(): Promise<UserInfo> {
  return api.get('/api/auth/me')
}
