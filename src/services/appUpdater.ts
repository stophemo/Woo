import { check, type DownloadEvent, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'

export interface AppUpdateInfo {
  currentVersion: string
  version: string
  date?: string
  body?: string
}

export interface AppUpdateProgress {
  downloadedBytes: number
  totalBytes?: number
  downloadFinished: boolean
}

let pendingUpdate: Update | null = null

export async function clearPendingAppUpdate(): Promise<void> {
  const update = pendingUpdate
  pendingUpdate = null
  await update?.close().catch(() => {})
}

export async function checkForAppUpdate(): Promise<AppUpdateInfo | null> {
  await clearPendingAppUpdate()

  const update = await check({ timeout: 15_000 })
  if (!update) return null

  pendingUpdate = update
  return {
    currentVersion: update.currentVersion,
    version: update.version,
    date: update.date,
    body: update.body,
  }
}

export async function installPendingAppUpdate(
  onProgress: (progress: AppUpdateProgress) => void
): Promise<void> {
  if (!pendingUpdate) throw new Error('没有可安装的更新')

  let downloadedBytes = 0
  let totalBytes: number | undefined
  const handleDownloadEvent = (event: DownloadEvent) => {
    if (event.event === 'Started') {
      totalBytes = event.data.contentLength
    } else if (event.event === 'Progress') {
      downloadedBytes += event.data.chunkLength
    }

    onProgress({
      downloadedBytes,
      totalBytes,
      downloadFinished: event.event === 'Finished',
    })
  }

  const update = pendingUpdate
  await update.downloadAndInstall(handleDownloadEvent, { timeout: 120_000 })
  pendingUpdate = null
}

export async function relaunchAfterUpdate(): Promise<void> {
  await relaunch()
}
