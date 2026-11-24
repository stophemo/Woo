/**
 * WebDAV 同步空间接口：账号、空间、六位码配对。
 */
import { invoke } from './api'

export interface WebDavAccountView {
  configured: boolean
  active: boolean
  endpoint: string
  username: string
  hasPassword: boolean
}

export interface WebDavIdentityView {
  configured: boolean
  active: boolean
  vaultId: string
  deviceId: string
  deviceName: string
  platform: string
  spaceFingerprint: string
}

export interface WebdavSpaceConfigView {
  account: WebDavAccountView
  identity: WebDavIdentityView
  spaceConfigured: boolean
  active: boolean
  lastSyncTime: string | null
  pendingChanges: number
  notice: string | null
  legacyNotice: string | null
}

export interface PairingStartView {
  pairingId: string
  shortCode: string
  expiresAt: number
  verificationCode: string | null
}

export interface PairingPeerView {
  deviceId: string
  name: string
  platform: string
}

export interface PairingStatusView {
  pairingId: string
  status: 'waiting' | 'claimed' | 'resolved' | 'ready' | 'approved' | 'expired' | string
  expiresAt: number
  verificationCode: string | null
  peer: PairingPeerView | null
  vaultId: string | null
  spaceFingerprint: string | null
  message: string | null
}

export interface JoinConfirmView {
  configured: boolean
  vaultId: string
  deviceId: string
  deviceName: string
  platform: string
  spaceFingerprint: string
}

export interface SpaceCreatedView {
  vaultId: string
  deviceId: string
  deviceName: string
  spaceFingerprint: string
}

export interface SyncResultView {
  pushedCount: number
  pulledCount: number
  conflictCount: number
  cleanupCount: number
  tombstoneCount: number
  syncTime: string
}

export function getConfig(): Promise<WebdavSpaceConfigView> {
  return invoke<WebdavSpaceConfigView>('webdavSpace:getConfig')
}

export function connectAccount(
  endpoint: string,
  username: string,
  password?: string,
): Promise<WebDavAccountView> {
  return invoke<WebDavAccountView>('webdavSpace:connectAccount', {
    endpoint,
    username,
    password: password || null,
  })
}

export function createSpace(deviceName: string): Promise<SpaceCreatedView> {
  return invoke<SpaceCreatedView>('webdavSpace:create', { deviceName })
}

export function leaveSpace(): Promise<void> {
  return invoke<void>('webdavSpace:leave')
}

export function syncNow(): Promise<SyncResultView> {
  return invoke<SyncResultView>('webdavSpace:syncNow')
}

export function startOffer(): Promise<PairingStartView> {
  return invoke<PairingStartView>('webdavSpace:startOffer')
}

export function startJoin(deviceName: string): Promise<PairingStartView> {
  return invoke<PairingStartView>('webdavSpace:startJoin', { deviceName })
}

export function findOffer(code: string, deviceName: string): Promise<PairingStatusView> {
  return invoke<PairingStatusView>('webdavSpace:findOffer', { code, deviceName })
}

export function findJoin(code: string): Promise<PairingStatusView> {
  return invoke<PairingStatusView>('webdavSpace:findJoin', { code })
}

export function offerStatus(pairingId: string): Promise<PairingStatusView> {
  return invoke<PairingStatusView>('webdavSpace:offerStatus', { pairingId })
}

export function joinStatus(pairingId: string): Promise<PairingStatusView> {
  return invoke<PairingStatusView>('webdavSpace:joinStatus', { pairingId })
}

export function approveOffer(pairingId: string): Promise<PairingStatusView> {
  return invoke<PairingStatusView>('webdavSpace:approveOffer', { pairingId })
}

export function approveJoin(pairingId: string): Promise<PairingStatusView> {
  return invoke<PairingStatusView>('webdavSpace:approveJoin', { pairingId })
}

export function confirmJoin(pairingId: string): Promise<JoinConfirmView> {
  return invoke<JoinConfirmView>('webdavSpace:confirmJoin', { pairingId })
}

export function cancelPairing(): Promise<void> {
  return invoke<void>('webdavSpace:cancelPairing')
}

export default {
  getConfig,
  connectAccount,
  createSpace,
  leaveSpace,
  syncNow,
  startOffer,
  startJoin,
  findOffer,
  findJoin,
  offerStatus,
  joinStatus,
  approveOffer,
  approveJoin,
  confirmJoin,
  cancelPairing,
}
