import { check, type DownloadEvent, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'

const UPDATE_CHECK_TIMEOUT_MS = 15_000
const LOCAL_UPDATE_PROXY = 'http://127.0.0.1:7892'

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

function errorMessage(error: unknown): string {
  if (error instanceof Error && error.message) return error.message
  if (typeof error === 'string' && error.trim()) return error.trim()
  return '未知错误'
}

export async function clearPendingAppUpdate(): Promise<void> {
  const update = pendingUpdate
  pendingUpdate = null
  await update?.close().catch(() => {})
}

export async function checkForAppUpdate(): Promise<AppUpdateInfo | null> {
  await clearPendingAppUpdate()

  let update: Update | null
  try {
    update = await check({
      timeout: UPDATE_CHECK_TIMEOUT_MS,
      proxy: LOCAL_UPDATE_PROXY,
    })
  } catch (proxyError: unknown) {
    try {
      update = await check({ timeout: UPDATE_CHECK_TIMEOUT_MS })
    } catch (directError: unknown) {
      throw new Error(
        `无法连接更新服务器，已尝试本地代理 ${LOCAL_UPDATE_PROXY} 和直连。` +
        `代理：${errorMessage(proxyError)}；直连：${errorMessage(directError)}`
      )
    }
  }
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
