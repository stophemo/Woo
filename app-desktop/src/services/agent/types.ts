/**
 * Agent 类型定义（轻量 MCP 风格）
 *
 * Tool 定义兼容 MCP JSON Schema 格式，
 * Agent 支持 OpenAI / DeepSeek / Gemini 的 function calling。
 */

/** Tool 参数 JSON Schema */
export interface ToolParameter {
  type: 'object' | 'string' | 'number' | 'boolean' | 'array'
  description?: string
  properties?: Record<string, ToolParameter>
  required?: string[]
  items?: ToolParameter
}

/** Tool 定义 */
export interface Tool {
  name: string
  description: string
  parameters: ToolParameter
  handler: (args: Record<string, unknown>) => Promise<string>
}

/** Agent 支持的供应商 */
export type AgentProvider = 'deepseek' | 'gemini' | 'openai-compatible'

/** Agent 配置 */
export interface AgentConfig {
  provider: AgentProvider
  apiKey: string
  baseUrl?: string
  model: string
  systemPrompt?: string
}

/** 标准消息格式 */
export interface AgentMessage {
  role: 'system' | 'user' | 'assistant' | 'tool'
  content: string
  name?: string
  tool_call_id?: string
  tool_calls?: ToolCall[]
}

/** API 返回的 Tool Call */
export interface ToolCall {
  id: string
  type: 'function'
  function: {
    name: string
    arguments: string
  }
}

/** Streaming 回调 */
export interface StreamCallbacks {
  onChunk?: (text: string) => void
  onToolCall?: (toolName: string, args: string) => void
  onToolResult?: (toolName: string, result: string) => void
  onError?: (error: string) => void
}

/** 上下文提供者 */
export interface ContextProvider {
  /** 获取当前文档内容 */
  getCurrentDoc: () => Promise<string | null>
  /** 搜索知识库（语义搜索） */
  searchKb: (query: string, limit?: number) => Promise<string>
  /** 按标题搜索笔记文档，返回格式化列表（含标题和修改时间） */
  searchDocuments: (query: string) => Promise<string>
  /** 根据文档标题获取文档详细信息（含内容、修改时间等） */
  getDocumentDetail: (title: string) => Promise<string | null>

  // ── CRUD 工具所需的上下文方法 ──

  /** 通过名称查找目录，返回目录 ID 或 null */
  findFolderByName: (name: string) => Promise<string | null>
  /** 通过标题查找笔记，返回 { id, title } 或 null */
  findNoteByTitle: (title: string) => Promise<{ id: string; title: string } | null>
  /** 创建笔记，返回笔记 ID 或 null */
  createNote: (title: string, folderId?: string) => Promise<string | null>
  /** 更新笔记内容，返回是否成功 */
  updateNote: (noteId: string, content: string) => Promise<boolean>
  /** 软删除笔记，返回是否成功 */
  deleteNote: (noteId: string) => Promise<boolean>
  /** 创建目录，返回目录 ID 或 null */
  createFolder: (name: string, parentId?: string) => Promise<string | null>
  /** 重命名目录，返回是否成功 */
  renameFolder: (folderId: string, newName: string) => Promise<boolean>
  /** 删除目录（级联软删除笔记），返回是否成功 */
  deleteFolder: (folderId: string) => Promise<boolean>
  /** 列出所有目录，返回格式化字符串 */
  listFolders: () => Promise<string>
  /** 列出最近笔记，返回格式化字符串 */
  listRecentNotes: (limit?: number) => Promise<string>
}
