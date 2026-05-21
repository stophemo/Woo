/**
 * 认证相关接口（本地版占位）。
 * 本地应用无登录注册，导出空函数保持其他模块不报错。
 */

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

// 本地版：直接返回成功的假数据（实际不会被调用）
export function login(_payload: LoginPayload): Promise<LoginResult> {
  return Promise.reject(new Error('本地版不支持登录'))
}

export function register(_payload: RegisterPayload): Promise<void> {
  return Promise.reject(new Error('本地版不支持注册'))
}

export function me(): Promise<UserInfo> {
  return Promise.reject(new Error('本地版不支持获取用户信息'))
}
