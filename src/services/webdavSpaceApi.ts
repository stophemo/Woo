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
  status: 'waiting' | 'claimed' | 'resolved' | 'ready' | 'approved' | 'expired' | 'configured' | string
  configured: boolean
  expiresAt: number
  verificationCode: string | null
  peer: PairingPeerView | null
  vaultId: string | null
  spaceFingerprint: string | null
  message: string | null
}

export interface OfferPreviewView {
  pairingId: string
  shortCode: string
  expiresAt: number
  peer: PairingPeerView
  vaultId: string
  spaceFingerprint: string
}

export interface JoinLocalStateView {
  hasIdentity: boolean
  vaultId: string | null
  spaceFingerprint: string | null
  lastSyncTime: string | null
  documentCount: number
  folderCount: number
  versionCount: number
  trashCount: number
  hasLocalData: boolean
}

export interface WebDavDeviceView {
  deviceId: string
  name: string
  platform: string
  lastSeenAt: number
  appVersion: string
  isCurrent: boolean
}

export interface SpaceCreatedView {
  vaultId: string
  deviceId: string
  deviceName: string
  spaceFingerprint: string
}

export interface ImportConfigView {
  endpoint: string
  username: string
  password: string | null
  vaultId: string | null
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

/** 生成“配置卡”：新设备粘贴后自动填好账号。 */
export function exportConfig(includePassword: boolean): Promise<string> {
  return invoke<string>('webdavSpace:exportConfig', { includePassword })
}

/** 解析配置卡，不保存、不发请求。 */
export function parseConfig(payload: string): Promise<ImportConfigView> {
  return invoke<ImportConfigView>('webdavSpace:parseConfig', { payload })
}

export function getJoinState(): Promise<JoinLocalStateView> {
  return invoke<JoinLocalStateView>('webdavSpace:joinState')
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

export function startJoin(deviceName: string, adoptPeer: boolean): Promise<PairingStartView> {
  return invoke<PairingStartView>('webdavSpace:startJoin', { deviceName, adoptPeer })
}

export function previewOffer(code: string): Promise<OfferPreviewView> {
  return invoke<OfferPreviewView>('webdavSpace:previewOffer', { code })
}

export function listDevices(): Promise<WebDavDeviceView[]> {
  return invoke<WebDavDeviceView[]>('webdavSpace:listDevices')
}

export function findOffer(
  code: string,
  deviceName: string,
  adoptPeer: boolean,
): Promise<PairingStatusView> {
  return invoke<PairingStatusView>('webdavSpace:findOffer', { code, deviceName, adoptPeer })
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

export function cancelPairing(): Promise<void> {
  return invoke<void>('webdavSpace:cancelPairing')
}

export default {
  getConfig,
  connectAccount,
  exportConfig,
  parseConfig,
  joinState: getJoinState,
  createSpace,
  leaveSpace,
  syncNow,
  startOffer,
  startJoin,
  previewOffer,
  listDevices,
  findOffer,
  findJoin,
  offerStatus,
  joinStatus,
  approveOffer,
  approveJoin,
  cancelPairing,
}
