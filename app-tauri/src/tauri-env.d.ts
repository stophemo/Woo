/**
 * Tauri v2 API declarations for Woo frontend
 */
import type { invoke as TauriInvoke } from '@tauri-apps/api/core'

// Tauri's invoke returns Promise<T> directly (no { ok, data } wrapper)
// We keep the window.woo interface for backward compat during migration
interface WooAPI {
  invoke: (channel: string, ...args: any[]) => Promise<any>
}

declare global {
  interface Window {
    woo: WooAPI
    electronAPI?: Record<string, (...args: any[]) => any>
  }
}

export {}
