import { invoke } from './api'

export interface McpView {
  enabled: boolean
  running: boolean
  active: boolean
  port: number
  url: string | null
  token: string
  allowWrites: boolean
  boundUser: string | null
  tokenRotatedAt: string | null
  lastError: string | null
}

export function getMcpConfig(): Promise<McpView> {
  return invoke<McpView>('mcp:getConfig')
}

export function setMcpEnabled(enabled: boolean): Promise<McpView> {
  return invoke<McpView>('mcp:setEnabled', { enabled })
}

export function setMcpAllowWrites(allowWrites: boolean): Promise<McpView> {
  return invoke<McpView>('mcp:setAllowWrites', { allowWrites })
}

export function setMcpPort(port: number): Promise<McpView> {
  return invoke<McpView>('mcp:setPort', { port })
}

export function regenerateMcpToken(): Promise<McpView> {
  return invoke<McpView>('mcp:regenerateToken')
}

export function setMcpEditorPresence(
  documentId: string | null,
  dirty: boolean,
): Promise<void> {
  return invoke<void>('mcp:setEditorPresence', { documentId, dirty })
}
