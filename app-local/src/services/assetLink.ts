export interface AssetLinkSettings {
  provider: string
  baseUrl: string
  pathPrefix: string
}

const STORAGE_KEY = 'asset-link-settings'

const DEFAULT_SETTINGS: AssetLinkSettings = {
  provider: 'custom',
  baseUrl: '',
  pathPrefix: ''
}

export function getAssetLinkSettings(): AssetLinkSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return { ...DEFAULT_SETTINGS }
    const parsed = JSON.parse(raw) as Partial<AssetLinkSettings>
    return {
      provider: parsed.provider || DEFAULT_SETTINGS.provider,
      baseUrl: parsed.baseUrl || DEFAULT_SETTINGS.baseUrl,
      pathPrefix: parsed.pathPrefix || DEFAULT_SETTINGS.pathPrefix
    }
  } catch {
    return { ...DEFAULT_SETTINGS }
  }
}

export function saveAssetLinkSettings(settings: AssetLinkSettings): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(settings))
}

export function buildAssetUrl(pathOrUrl: string): string {
  const input = pathOrUrl.trim()
  if (!input) return ''
  if (/^https?:\/\//i.test(input)) return input

  const { baseUrl, pathPrefix } = getAssetLinkSettings()
  if (!baseUrl) return input

  const left = baseUrl.replace(/\/+$/, '')
  const prefix = pathPrefix.trim().replace(/^\/+|\/+$/g, '')
  const right = input.replace(/^\/+/, '')

  if (!prefix) {
    return `${left}/${right}`
  }

  return `${left}/${prefix}/${right}`
}
