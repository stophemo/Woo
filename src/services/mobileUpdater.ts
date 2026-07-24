import { open } from '@tauri-apps/plugin-shell'
import { invoke } from './api'

export interface MobileUpdateInfo {
  currentVersion: string
  version: string
  notes?: string
  pubDate?: string
  downloadUrl: string
}

export function checkForMobileAppUpdate(): Promise<MobileUpdateInfo | null> {
  return invoke<MobileUpdateInfo | null>('app:check-mobile-update')
}

export function openMobileUpdateDownload(downloadUrl: string): Promise<void> {
  return open(downloadUrl)
}
