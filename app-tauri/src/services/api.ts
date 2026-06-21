/**
 * IPC client adapter for Tauri v2.
 * Maps Electron-style channel names (document:listByFolder) to Tauri camelCase
 * command names (documentListByFolder). Auto-unwraps { ok, data, message }.
 */
import { invoke as tauriInvoke } from '@tauri-apps/api/core'

interface CommandResult<T> {
  ok: boolean
  data?: T
  message?: string
}

/** Convert colon-separated channel name to camelCase matching Rust command names */
function toCamelCase(s: string): string {
  const parts = s.split(/[:-]/)
  const first = parts[0]
  const rest = parts.slice(1).map(p => p.charAt(0).toUpperCase() + p.slice(1)).join('')
  return first + rest
}

/**
 * Call a Tauri command, unwrapping the { ok, data, message } response.
 * Throws on !ok or on Tauri runtime error.
 */
export async function invoke<T = unknown>(channel: string, ...args: any[]): Promise<T> {
  const command = toCamelCase(channel)
  const payload = args.length === 1 && typeof args[0] === 'object' && !Array.isArray(args[0])
    ? args[0]
    : args.length === 0 ? undefined : args

  let raw: any
  try {
    raw = await tauriInvoke(command, payload)
  } catch (err: any) {
    throw new Error(err?.message || String(err) || '操作失败')
  }

  // Handle commands that return primitives (not wrapped)
  if (raw === null || raw === undefined) return undefined as T
  if (typeof raw !== 'object' || raw === null) return raw as T
  if (!('ok' in raw)) return raw as T

  // Unwrap { ok, data, message }
  const result = raw as CommandResult<T>
  if (result.ok) {
    return result.data as T
  }
  throw new Error(result.message || '操作失败')
}

export default { invoke }
