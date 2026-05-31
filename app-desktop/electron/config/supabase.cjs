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
  if (e.code !== 'ENOENT') {
    console.warn('[Supabase] 加载 .env 失败:', e.message)
  }
}

const SUPABASE_URL = process.env.SUPABASE_URL || 'https://urgpunxxupufmygmutxa.supabase.co'
const SUPABASE_ANON_KEY = process.env.SUPABASE_ANON_KEY || 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InVyZ3B1bnh4dXB1Zm15Z211dHhhIiwicm9sZSI6ImFub24iLCJpYXQiOjE3Nzk1MTA1OTYsImV4cCI6MjA5NTA4NjU5Nn0.Ezs2uFwgrxkwCpLVfdByFC8m5PNjjP92i26Sg0YX-RI'

/* ================================================================
 * 自定义 Storage 适配器
 *
 * @supabase/supabase-js 默认使用浏览器 localStorage 持久化 token，
 * 但在 Electron 主进程（Node.js）中 localStorage 不可用。
 * 此适配器将 token 持久化到 userData 目录的 JSON 文件，
 * 使得应用重启后 session 可恢复，无需重新登录。
 * ================================================================ */
function createFileStorage() {
  let storagePath = null

  function getStoragePath() {
    if (storagePath) return storagePath
    // 延迟获取 app path — 确保在 app.whenReady() 之后调用
    try {
      const { app } = require('electron')
      const userData = app.getPath('userData')
      storagePath = path.join(userData, 'supabase-auth.json')
    } catch (e) {
      // fallback: 写入项目目录
      storagePath = path.resolve(__dirname, '../../../.supabase-auth-cache.json')
    }
    return storagePath
  }

  function readAll() {
    try {
      const p = getStoragePath()
      if (!fs.existsSync(p)) return {}
      return JSON.parse(fs.readFileSync(p, 'utf-8'))
    } catch { return {} }
  }

  function writeAll(data) {
    try {
      const p = getStoragePath()
      fs.writeFileSync(p, JSON.stringify(data, null, 2), 'utf-8')
    } catch (e) {
      console.warn('[Supabase] 写入 auth 缓存失败:', e.message)
    }
  }

  return {
    getItem(key) {
      return readAll()[key] || null
    },
    setItem(key, value) {
      const data = readAll()
      data[key] = value
      writeAll(data)
    },
    removeItem(key) {
      const data = readAll()
      delete data[key]
      writeAll(data)
    }
  }
}

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
      detectSessionInUrl: false,
      storage: createFileStorage()
    },
    realtime: {
      transport: WebSocket
    }
  })
  return client
}

module.exports = { getClient }
