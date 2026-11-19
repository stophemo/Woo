export type McpClientId = 'claude-code' | 'codex' | 'hermes' | 'pi' | 'generic'

export interface McpClientOption {
  id: McpClientId
  label: string
  location: string
  build: (url: string, token: string) => string
}

export const mcpClientOptions: McpClientOption[] = [
  {
    id: 'claude-code',
    label: 'Claude Code',
    location: '在终端执行',
    build: (url, token) =>
      `claude mcp add --transport http woo ${url} --header "Authorization: Bearer ${token}"`,
  },
  {
    id: 'codex',
    label: 'Codex',
    location: '~/.codex/config.toml',
    build: (url, token) =>
      `[mcp_servers.woo]\nurl = "${url}"\nhttp_headers = { Authorization = "Bearer ${token}" }`,
  },
  {
    id: 'hermes',
    label: 'Hermes',
    location: '~/.hermes/config.yaml',
    build: (url, token) =>
      `mcp_servers:\n  woo:\n    url: ${url}\n    headers:\n      Authorization: Bearer ${token}\n    enabled: true`,
  },
  {
    id: 'pi',
    label: 'Pi',
    location: '~/.pi/agent/mcp.json 或项目 .pi/mcp.json',
    build: (url, token) =>
      JSON.stringify(
        {
          mcpServers: {
            woo: {
              type: 'http',
              url,
              headers: { Authorization: `Bearer ${token}` },
            },
          },
        },
        null,
        2,
      ),
  },
  {
    id: 'generic',
    label: '通用',
    location: '任意支持 Streamable HTTP 的 MCP 客户端',
    build: (url, token) => `URL: ${url}\nAuthorization: Bearer ${token}`,
  },
]

export function buildMcpSnippet(id: McpClientId, url: string, token: string): string {
  return mcpClientOptions.find(option => option.id === id)?.build(url, token) ?? ''
}

export function mcpClientLocation(id: McpClientId): string {
  return mcpClientOptions.find(option => option.id === id)?.location ?? ''
}
