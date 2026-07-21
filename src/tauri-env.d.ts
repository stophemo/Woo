/**
 * Tauri v2 API declarations for Woo frontend
 */
// Tauri's invoke returns Promise<T> directly (no { ok, data } wrapper)
// We keep the window.woo interface for backward compat during migration
interface WooAPI {
  invoke: (channel: string, ...args: any[]) => Promise<any>
}

interface ElectronCompatAPI {
  minimize: () => Promise<void>
  maximize: () => Promise<void>
  close: () => Promise<void>
  setFullscreen: (fullscreen: boolean) => Promise<void>
  openExternalLink: (url: string) => Promise<void>
  getAppVersion: () => Promise<string>
  onMenuAction: (callback: (action: string) => void) => void
  removeMenuActionListener: () => void
}

declare global {
  interface Window {
    woo: WooAPI
    electronAPI?: ElectronCompatAPI
  }
}

export {}
