/**
 * Supabase 认证服务。
 * 封装 @supabase/supabase-js 的 Auth API，供 IPC 层调用。
 *
 * ⚠️ 重要：所有方法返回原始数据，不包装 { ok, data }。
 *   IPC 层的 wrap() 会自动处理成功/错误包装。
 *   出错时直接 throw Error，wrap() 会捕获并返回 { ok: false, message }。
 *
 * 登录方式：
 * - 邮箱登录（输入包含 @）
 * - 用户名登录（先通过 get_email_by_username 查询邮箱）
 */
const { getClient } = require('../config/supabase.cjs')

function requireClient() {
  const client = getClient()
  if (!client) throw new Error('Supabase 未配置')
  return client
}

/**
 * 注册新用户
 * @param {string} email
 * @param {string} username - 用户名，存入 user_metadata
 * @param {string} password
 * @returns {{ user, session }}
 */
async function signUp(email, username, password) {
  const client = requireClient()

  const { data, error } = await client.auth.signUp({
    email,
    password,
    options: {
      data: { username, display_name: username },
      emailRedirectTo: 'https://woo-ten-ruby.vercel.app/auth-success'
    }
  })
  if (error) throw new Error(error.message)

  return { user: data.user, session: data.session }
}

/**
 * 邮箱密码登录（也支持用户名登录，自动解析）
 * @param {string} identifier - 邮箱或用户名
 * @param {string} password
 */
async function signIn(identifier, password) {
  const client = requireClient()

  // 判断输入：含 @ 则是邮箱，否则是用户名
  const email = identifier.includes('@')
    ? identifier
    : await resolveEmailByUsername(identifier)

  const { data, error } = await client.auth.signInWithPassword({ email, password })
  if (error) throw new Error(error.message)

  return { user: data.user, session: data.session }
}

/**
 * 通过用户名查询邮箱（调用 Supabase RPC 函数）
 */
async function resolveEmailByUsername(username) {
  const client = getClient()
  if (!client) throw new Error('Supabase 未配置')

  const { data, error } = await client.rpc('get_email_by_username', { p_username: username })
  if (error) throw new Error('用户不存在')
  if (!data) throw new Error('用户不存在')
  return data
}

/**
 * OAuth 登录（Google / GitHub）
 */
async function signInWithOAuth(provider) {
  const client = requireClient()

  const { data, error } = await client.auth.signInWithOAuth({
    provider,
    options: {
      redirectTo: 'http://localhost:5173'
    }
  })
  if (error) throw new Error(error.message)

  return { url: data.url }
}

/**
 * 登出
 */
async function signOut() {
  const client = requireClient()

  const { error } = await client.auth.signOut()
  if (error) throw new Error(error.message)
}

/**
 * 获取当前登录用户
 */
function getCurrentUser() {
  const client = getClient()
  if (!client) return null

  return client.auth.getUser().then(({ data, error }) => {
    if (error || !data?.user) return null
    return data.user
  })
}

/**
 * 获取当前 session（含 access_token）
 */
function getSession() {
  const client = getClient()
  if (!client) return null

  return client.auth.getSession().then(({ data }) => {
    return data?.session || null
  })
}

/**
 * 监听认证状态变化
 * 返回 unsubscribe 函数
 */
function onAuthStateChange(callback) {
  const client = getClient()
  if (!client) {
    callback('SIGNED_OUT', null)
    return () => {}
  }

  const { data: { subscription } } = client.auth.onAuthStateChange(
    (event, session) => {
      callback(event, session)
    }
  )

  return () => subscription?.unsubscribe()
}

module.exports = {
  signUp,
  signIn,
  signInWithOAuth,
  signOut,
  getCurrentUser,
  getSession,
  onAuthStateChange
}
