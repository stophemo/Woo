/**
 * Supabase 客户端单例。
 *
 * 使用说明：
 *   1. 在 Supabase 项目 Settings → API 中获取 Project URL 和 anon key
 *   2. 将下方占位符替换为实际值，或通过环境变量 SUPABASE_URL / SUPABASE_ANON_KEY 传入
 *   3. anon key 是公开的（由 RLS 策略保护数据安全），可安全嵌入客户端
 */
const { createClient } = require('@supabase/supabase-js')
const WebSocket = require('ws')

// 优先读取环境变量，未设置时使用占位符（构建前应替换为真实值）
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
