import { listen } from '@tauri-apps/api/event'
import { invoke } from './api'

const MOBILE_UPDATE_PROGRESS_EVENT = 'mobile-update-download-progress'

export interface MobileUpdateInfo {
  currentVersion: string
  version: string
  notes?: string
  pubDate?: string
  downloadUrl: string
}

export interface MobileUpdateProgress {
  downloadedBytes: number
  totalBytes?: number | null
  downloadFinished: boolean
}

export interface MobileUpdateDownload {
  apkPath: string
  cached: boolean
}

export interface MobileInstallResult {
  status: 'installStarted' | 'permissionRequired'
}

export function checkForMobileAppUpdate(): Promise<MobileUpdateInfo | null> {
  return invoke<MobileUpdateInfo | null>('app:check-mobile-update')
}

export async function downloadMobileAppUpdate(
  update: MobileUpdateInfo,
  onProgress: (progress: MobileUpdateProgress) => void,
): Promise<MobileUpdateDownload> {
  const unlisten = await listen<MobileUpdateProgress>(MOBILE_UPDATE_PROGRESS_EVENT, (event) => {
    onProgress(event.payload)
  })
  try {
    return await invoke<MobileUpdateDownload>('app:download-mobile-update', {
      version: update.version,
      downloadUrl: update.downloadUrl,
    })
  } finally {
    unlisten()
  }
}

export function installMobileAppUpdate(apkPath: string): Promise<MobileInstallResult> {
  return invoke<MobileInstallResult>('app:install-mobile-update', { apkPath })
}
