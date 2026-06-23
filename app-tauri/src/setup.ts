/**
 * Tauri v2 environment setup.
 * Bridges Electron-style APIs (window.electronAPI) using Tauri APIs.
 * This allows the existing Vue components to work without modification.
 */

import { getCurrentWindow } from '@tauri-apps/api/window'
import { open } from '@tauri-apps/plugin-shell'
import { listen } from '@tauri-apps/api/event'
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

  // Bridge Tauri backend events to DOM CustomEvents for existing store listeners.
  // This allows the sync store, workspace store, and App.vue to receive sync updates
  // without modification (they listen for 'sync-status', 'sync-data-changed', etc.).
  listen<unknown>('sync-status', (event) => {
    window.dispatchEvent(new CustomEvent('sync-status', { detail: event.payload }))
  })
  listen<unknown>('sync-data-changed', (event) => {
    window.dispatchEvent(new CustomEvent('sync-data-changed', { detail: event.payload }))
  })
  listen<unknown>('sync:toast', (event) => {
    window.dispatchEvent(new CustomEvent('sync:toast', { detail: event.payload }))
  })
}
