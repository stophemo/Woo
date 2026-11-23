import { invoke } from './api'

export interface VaultSyncConfigView {
  enabled: boolean
  active: boolean
  configured: boolean
  endpoint: string
  vaultId: string
  deviceId: string
  deviceName: string
}

export interface PairingCodeView {
  pairingId: string
  shortCode: string
  expiresAt: number
  serverTime: number
}

export interface ClaimedDeviceView {
  deviceId: string
  name: string
  platform: string
}

export interface PairingStatusView {
  pairingId: string
  status: string
  expiresAt: number
  claimedDevice: ClaimedDeviceView | null
  verificationCode: string | null
}

export interface JoinStartView {
  pairingId: string
  deviceId: string
  expiresAt: number
  verificationCode: string
}

export interface JoinPollView {
  status: string
  configured: boolean
  vaultId: string | null
  message: string | null
}

export interface VaultDeviceView {
  id: string
  name: string
  platform: string
  createdAt: number
  lastSeenAt: number | null
  isCurrent: boolean
}

export const vaultSyncApi = {
  getConfig: () => invoke<VaultSyncConfigView>('vaultSync:getConfig'),
  test: (endpoint: string) => invoke<string>('vaultSync:test', { endpoint }),
  createVault: (endpoint: string, inviteCode: string, deviceName: string) =>
    invoke<VaultSyncConfigView>('vaultSync:createVault', { endpoint, inviteCode, deviceName }),
  startPairing: () => invoke<PairingCodeView>('vaultSync:startPairing'),
  pairingStatus: (pairingId: string) =>
    invoke<PairingStatusView>('vaultSync:pairingStatus', { pairingId }),
  confirmPairing: (pairingId: string) =>
    invoke<void>('vaultSync:confirmPairing', { pairingId }),
  beginJoin: (endpoint: string, code: string, deviceName: string) =>
    invoke<JoinStartView>('vaultSync:beginJoin', { endpoint, code, deviceName }),
  pollJoin: (pairingId: string) =>
    invoke<JoinPollView>('vaultSync:pollJoin', { pairingId }),
  leave: () => invoke<void>('vaultSync:leave'),
  seedLocal: () => invoke<number>('vaultSync:seedLocal'),
  listDevices: () => invoke<VaultDeviceView[]>('vaultSync:listDevices'),
  revokeDevice: (deviceId: string) =>
    invoke<void>('vaultSync:revokeDevice', { deviceId })
}

export default vaultSyncApi
