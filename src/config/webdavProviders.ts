/**
 * WebDAV 服务商预设：只做两件事——预填常见地址、给出对应的密码说明。
 * 地址始终可编辑；用户改成别的值时自动落到「自定义」。
 */
export interface WebdavProviderPreset {
  id: string
  label: string
  /** 可直接预填的完整地址；为空表示需要用户按模板填写 */
  endpoint: string
  endpointPlaceholder: string
  passwordHint: string
}

export const WEBDAV_PROVIDERS: WebdavProviderPreset[] = [
  {
    id: 'jianguoyun',
    label: '坚果云',
    endpoint: 'https://dav.jianguoyun.com/dav/',
    endpointPlaceholder: 'https://dav.jianguoyun.com/dav/',
    passwordHint:
      '请在坚果云「账户信息 → 安全选项」中生成「第三方应用密码」，不要使用登录密码。',
  },
  {
    id: 'nextcloud',
    label: 'Nextcloud / ownCloud',
    endpoint: '',
    endpointPlaceholder: 'https://cloud.example.com/remote.php/dav/files/用户名/',
    passwordHint:
      '在「设置 → 安全」中生成应用密码；地址要写到用户目录，例如 /remote.php/dav/files/你的用户名/。',
  },
  {
    id: 'synology',
    label: '群晖 Synology',
    endpoint: '',
    endpointPlaceholder: 'https://nas.example.com:5006/共享文件夹/',
    passwordHint:
      '先在套件中心启用 WebDAV Server；HTTPS 端口通常是 5006，地址写到共享文件夹。',
  },
  {
    id: 'infinicloud',
    label: 'InfiniCLOUD',
    endpoint: '',
    endpointPlaceholder: 'https://服务器地址/用户名/',
    passwordHint: '地址通常是 https://服务器地址/用户名/；请在 InfiniCLOUD 设置中开启 WebDAV。',
  },
  {
    id: 'custom',
    label: '其他 / 自定义',
    endpoint: '',
    endpointPlaceholder: 'https://dav.example.com/dav/',
    passwordHint: '填写完整的 HTTPS WebDAV 地址；部分服务需要单独生成「应用密码」。',
  },
]

const DEFAULT_PROVIDER = WEBDAV_PROVIDERS[0]

export function findProvider(id: string): WebdavProviderPreset {
  return WEBDAV_PROVIDERS.find(provider => provider.id === id) || DEFAULT_PROVIDER
}

/** 根据已保存的地址反推服务商，识别不了就归到「自定义」。 */
export function matchProvider(endpoint: string): WebdavProviderPreset {
  const normalized = endpoint.trim().replace(/\/+$/, '')
  if (!normalized) return DEFAULT_PROVIDER
  const matched = WEBDAV_PROVIDERS.find(
    provider => provider.endpoint && provider.endpoint.replace(/\/+$/, '') === normalized,
  )
  return matched || findProvider('custom')
}
