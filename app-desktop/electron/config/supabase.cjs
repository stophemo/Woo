/**
 * Supabase 客户端单例。
 *
 * 配置优先级（降序）：
 *   1. 环境变量 SUPABASE_URL / SUPABASE_ANON_KEY
 *   2. 项目根目录 .env 文件
 *   3. 下方编译时占位符（打包后使用）
 *
 * anon key 是公开的（由 RLS 策略保护数据安全），可安全嵌入客户端。
 */
const { createClient } = require('@supabase/supabase-js')
const path = require('path')
const fs = require('fs')
const WebSocket = require('ws')

// 从项目根目录 .env 加载（多端共享配置值）
try {
  const envPath = path.resolve(__dirname, '../../../.env')
  const envContent = fs.readFileSync(envPath, 'utf-8')
  for (const line of envContent.split('\n')) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith('#')) continue
    const eqIdx = trimmed.indexOf('=')
    if (eqIdx === -1) continue
    const key = trimmed.slice(0, eqIdx).trim()
    const value = trimmed.slice(eqIdx + 1).trim()
    if (key && value && !process.env[key]) {
      process.env[key] = value
    }
  }
} catch (e) {
  // .env 不存在时不报错（生产环境或 CI 使用环境变量或占位符）
  if (e.code !== 'ENOENT') {
    console.warn('[Supabase] 加载 .env 失败:', e.message)
  }
}

// 最终取值：环境变量 > .env > 编译时占位符
const SUPABASE_URL = process.env.SUPABASE_URL || 'https://urgpunxxupufmygmutxa.supabase.co'
const SUPABASE_ANON_KEY = process.env.SUPABASE_ANON_KEY || 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InVyZ3B1bnh4dXB1Zm15Z211dHhhIiwicm9sZSI6ImFub24iLCJpYXQiOjE3Nzk1MTA1OTYsImV4cCI6MjA5NTA4NjU5Nn0.Ezs2uFwgrxkwCpLVfdByFC8m5PNjjP92i26Sg0YX-RI'

let client = null

function getClient() {
  if (!SUPABASE_URL || !SUPABASE_ANON_KEY) {
    console.warn('[Supabase] URL 或 Key 未配置，请先在 config/supabase.cjs 中设置')
    return null
  }
  if (client) return client
  client = createClient(SUPABASE_URL, SUPABASE_ANON_KEY, {
    auth: {
      persistSession: true,
      autoRefreshToken: true,
      detectSessionInUrl: false // Electron 环境下关闭 URL 检测
    },
    realtime: {
      transport: WebSocket
    }
  })
  return client
}

module.exports = { getClient }
