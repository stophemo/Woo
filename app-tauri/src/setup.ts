/**
 * Tauri v2 environment setup.
 * Bridges Electron-style APIs (window.electronAPI) using Tauri APIs.
 * This allows the existing Vue components to work without modification.
 */

import { getCurrentWindow } from '@tauri-apps/api/window'
import { open } from '@tauri-apps/plugin-shell'
import { invoke } from './services/api'

let menuActionHandler: ((event: Event) => void) | null = null

export function setupTauriBridge() {
  const appWindow = getCurrentWindow()

  window.electronAPI = {
    // Window controls
    minimize: () => appWindow.minimize(),
    maximize: () => appWindow.toggleMaximize(),
    close: () => appWindow.close(),
    setFullscreen: (fullscreen: boolean) => appWindow.setFullscreen(fullscreen),

    // External links
    openExternalLink: (url: string) => open(url),

    // App version
    getAppVersion: () => invoke<string>('app:getVersion'),

    // Menu actions (Tauri emits menu events via Event system)
    onMenuAction: (callback: (action: string) => void) => {
      menuActionHandler = (event: Event) => {
        callback((event as CustomEvent<string>).detail)
      }
      window.addEventListener('tauri-menu-action', menuActionHandler as EventListener)
    },
    removeMenuActionListener: () => {
      if (menuActionHandler) {
        window.removeEventListener('tauri-menu-action', menuActionHandler as EventListener)
        menuActionHandler = null
      }
    },

    // Update check
    checkForUpdates: () => invoke<{ hasUpdate: boolean; version?: string; downloadUrl?: string; error?: string }>('update:check'),
  }
}
