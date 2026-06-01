import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { ChatMessage, ThinkingStep, ModelConfig, ProviderType } from '../types/ai'
import { validateApiKey as validateOpenAIKey, listModels as listOpenAIModels } from '../services/deepseek'
import { validateGeminiKey, listGeminiModels } from '../services/gemini'
import { Agent } from '../services/agent'
import type { AgentProvider } from '../services/agent/types'
import { clearConfirmation } from '../services/agent/confirmation'
import { marked } from 'marked'
import { useWorkspaceStore } from './workspace'

const STORAGE_KEY = 'ai-settings'
const CHAT_STORAGE_KEY = 'ai-chat-messages'
const KB_STORAGE_KEY = 'ai-kb-enabled'

/* ========== 树遍历工具（用于 AI 操作后同步响应式状态） ========== */
interface FolderNodeLike {
  id: string
  name: string
  parentId: string | null
  children: FolderNodeLike[]
  isExpanded: boolean
  isLocked?: boolean
}
function findFolderInTree(nodes: FolderNodeLike[], id: string): FolderNodeLike | null {
  for (const node of nodes) {
    if (node.id === id) return node
    if (node.children.length > 0) {
      const found = findFolderInTree(node.children, id)
      if (found) return found
    }
  }
  return null
}
function removeFolderFromTree(nodes: FolderNodeLike[], id: string): boolean {
  for (let i = nodes.length - 1; i >= 0; i--) {
    if (nodes[i].id === id) {
      nodes.splice(i, 1)
      return true
    }
    if (nodes[i].children.length > 0 && removeFolderFromTree(nodes[i].children, id)) {
      return true
    }
  }
  return false
}

marked.setOptions({ breaks: true, gfm: true })

/**
 * 将 AI 输出的 Markdown 内容转为编辑器兼容的 HTML。
 * 如果内容已含 HTML 标签则跳过转换，避免双重编码。
 */
function mdToHtml(text: string): string {
  if (!text) return ''
  // 如果已包含 HTML 块级标签，视为已转换，直接返回
  if (/<[a-z]+[\s>]/i.test(text)) return text
  try {
    return marked.parse(text) as string
  } catch {
    return text
  }
}

/** IPC invoke 的简单包装 */
async function ipc<T = unknown>(channel: string, ...args: any[]): Promise<T> {
  const w = (window as any).woo
  if (!w || typeof w.invoke !== 'function') throw new Error('IPC 未就绪')
  const res = await w.invoke(channel, ...args)
  if (!res.ok) throw new Error(res.message || '操作失败')
  return res.data as T
}

/* ========== 各供应商的预设模型 ========== */
const DEEPSEEK_MODELS: ModelConfig[] = [
  { id: 'deepseek-v4-flash', name: 'deepseek-v4-flash', provider: 'deepseek', model: 'deepseek-v4-flash' },
  { id: 'deepseek-v4-pro',   name: 'deepseek-v4-pro',   provider: 'deepseek', model: 'deepseek-v4-pro' },
]

const GEMINI_MODELS: ModelConfig[] = [
  { id: 'gemini-2.5-flash',      name: 'Gemini 2.5 Flash',     provider: 'gemini', model: 'gemini-2.5-flash' },
  { id: 'gemini-2.5-flash-lite', name: 'Gemini 2.5 Flash-Lite',provider: 'gemini', model: 'gemini-2.5-flash-lite' },
  { id: 'gemini-2.5-pro',        name: 'Gemini 2.5 Pro',       provider: 'gemini', model: 'gemini-2.5-pro' },
]

const OPENAI_MODELS: ModelConfig[] = [
  { id: '_custom_', name: '自定义模型名', provider: 'openai-compatible', model: '' },
]

export const useAiChatStore = defineStore('aiChat', () => {
  const messages = ref<ChatMessage[]>([])
  const selectedModelId = ref<string>('deepseek-v4-flash')
  const isStreaming = ref(false)
  const error = ref<string | null>(null)
  const abortController = ref<AbortController | null>(null)
  const _apiKeyVersion = ref(0)

  /* ---------- Agent 实例（懒初始化） ---------- */
  let agent: Agent | null = null

  /** 获取或初始化 Agent */
  function getAgent(): Agent {
    if (agent) {
      // 配置可能已变更，更新之
      const cm = currentModel.value
      const actualModel = cm?.id === '_custom_' ? (getCustomModelName() || cm?.model) : cm?.model || 'deepseek-v4-flash'
      agent.updateConfig({
        provider: props_provider.value as AgentProvider,
        apiKey: getApiKey(),
        baseUrl: provider.value === 'gemini' ? undefined : (provider.value === 'deepseek' ? getDeepseekBaseUrl() : getOpenaiBaseUrl()),
        model: actualModel,
      })
      return agent
    }

    const cm = currentModel.value
    const actualModel = cm?.id === '_custom_' ? (getCustomModelName() || cm?.model) : cm?.model || 'deepseek-v4-flash'
    agent = new Agent({
      provider: props_provider.value as AgentProvider,
      apiKey: getApiKey(),
      baseUrl: provider.value === 'gemini' ? undefined : (provider.value === 'deepseek' ? getDeepseekBaseUrl() : getOpenaiBaseUrl()),
      model: actualModel,
    })

    // 设置上下文提供者（工具执行时用到）
    agent.setContextProvider({
      getCurrentDoc: async () => {
        return documentContextCache || null
      },
      searchKb: async (query: string, limit?: number) => {
        return searchKb(query, limit || 6)
      },
      searchDocuments: async (query: string) => {
        try {
          const docs = await ipc<any[]>('document:search', query)
          if (!docs || docs.length === 0) return ''
          return docs.map((d, i) => {
            const time = d.update_time ? new Date(d.update_time).toLocaleString('zh-CN') : '未知'
            return `${i + 1}. 《${d.title}》最后修改: ${time}`
          }).join('\n')
        } catch {
          return ''
        }
      },
      getDocumentDetail: async (title: string) => {
        try {
          // 先通过搜索找到匹配标题的文档
          const docs = await ipc<any[]>('document:search', title)
          const match = docs?.find((d: any) => d.title === title)
          if (!match) {
            // 模糊匹配第一个
            const fuzzy = docs?.[0]
            if (!fuzzy) return null
            const detail = await ipc<any>('document:get', fuzzy.id)
            if (!detail) return null
            const created = detail.create_time ? new Date(detail.create_time).toLocaleString('zh-CN') : '未知'
            const updated = detail.update_time ? new Date(detail.update_time).toLocaleString('zh-CN') : '未知'
            const contentLen = detail.content ? detail.content.replace(/<[^>]+>/g, '').length : 0
            return `标题: ${fuzzy.title}\n创建: ${created}\n最后修改: ${updated}\n字数: ${contentLen}`
          }
          const detail = await ipc<any>('document:get', match.id)
          if (!detail) return null
          const created = detail.create_time ? new Date(detail.create_time).toLocaleString('zh-CN') : '未知'
          const updated = detail.update_time ? new Date(detail.update_time).toLocaleString('zh-CN') : '未知'
          const contentLen = detail.content ? detail.content.replace(/<[^>]+>/g, '').length : 0
          return `标题: ${match.title}\n创建: ${created}\n最后修改: ${updated}\n字数: ${contentLen}\n内容预览: ${(detail.content || '').replace(/<[^>]+>/g, '').slice(0, 500)}`
        } catch {
          return null
        }
      },

      // ── CRUD 工具上下文方法 ──

      findFolderByName: async (name: string) => {
        try {
          const tree = await ipc<any[]>('folder:tree')
          const queue = [...tree]
          while (queue.length > 0) {
            const node = queue.shift()
            if (node.name === name) return node.id
            if (node.children) queue.push(...node.children)
          }
          return null
        } catch { return null }
      },

      findNoteByTitle: async (title: string) => {
        try {
          const docs = await ipc<any[]>('document:search', title)
          // 精确匹配优先
          const exact = docs?.find(d => d.title === title)
          if (exact) return { id: exact.id, title: exact.title }
          // 模糊匹配第一个
          if (docs && docs.length > 0) return { id: docs[0].id, title: docs[0].title }
          return null
        } catch { return null }
      },

      createNote: async (title: string, folderId?: string) => {
        try {
          let targetFolderId = folderId
          if (!targetFolderId) {
            // 没有指定目录时，用第一个顶级目录
            const tree = await ipc<any[]>('folder:tree')
            if (tree && tree.length > 0) {
              targetFolderId = tree[0].id
            }
          }
          if (!targetFolderId) return null
          const doc = await ipc<any>('document:create', { title, folderId: targetFolderId })
          if (doc?.id) {
            // 如果当前视图正好是目标目录，立即更新文稿列表
            const ws = useWorkspaceStore()
            if (ws.selectedFolderId === targetFolderId) {
              ws.folderDocuments.unshift({
                id: doc.id,
                title: doc.title || title,
                content: '',
                folderId: targetFolderId,
                folderName: '',
                createdAt: doc.createTime || doc.create_time || new Date().toISOString(),
                updatedAt: doc.updateTime || doc.update_time || new Date().toISOString(),
              })
            }
          }
          return doc?.id || null
        } catch { return null }
      },

      updateNote: async (noteId: string, content: string) => {
        const ws = useWorkspaceStore()
        try {
          const html = mdToHtml(content)
          // 先切换到目标文档
          if (ws.selectedDocumentId !== noteId) {
            await ws.selectDocument(noteId)
          }
          // 流式写入编辑器 — 模拟肉眼可见的打字效果
          const totalLen = html.length
          // 每批 3~5 字符，总耗时至少 1.5 秒以便用户感知
          const chunkSize = totalLen > 800 ? 5 : 3
          const minDuration = 1500
          const totalChunks = Math.ceil(totalLen / chunkSize)
          const delayMs = Math.max(25, Math.min(80, Math.ceil(minDuration / totalChunks)))
          let pos = 0
          ws.isExternalStreaming = true
          while (pos < html.length) {
            let end = Math.min(pos + chunkSize, html.length)
            // 避免卡在标签中间
            const frag = html.slice(pos, end)
            if (frag.includes('<') && !frag.includes('>')) {
              const rest = html.slice(end)
              const closeBracket = rest.indexOf('>')
              if (closeBracket >= 0 && closeBracket < 100) end += closeBracket + 1
            }
            while (end < html.length && (html.charCodeAt(end) & 0xC0) === 0x80) end++
            ws.updateContentStream(noteId, html.slice(0, end))
            pos = end
            await new Promise(r => setTimeout(r, delayMs))
          }
          // 最终写入后端
          await ipc('document:updateContent', noteId, html)
          return true
        } catch {
          return false
        } finally {
          ws.isExternalStreaming = false
        }
      },

      deleteNote: async (noteId: string) => {
        try {
          await ipc('document:remove', noteId)
          // 立即更新响应式状态，触发 ThumbnailColumn 列表变更
          const ws = useWorkspaceStore()
          const idx = ws.folderDocuments.findIndex((d: any) => d.id === noteId)
          if (idx !== -1) {
            ws.folderDocuments.splice(idx, 1)
          }
          if (ws.selectedDocumentId === noteId) {
            const next = ws.folderDocuments[0]
            if (next) {
              ws.selectedDocumentId = next.id
              ws.selectDocument(next.id)
            } else {
              ws.selectedDocumentId = null
              ws.currentDocumentData = null
            }
          }
          return true
        } catch { return false }
      },

      createFolder: async (name: string, parentId?: string) => {
        try {
          const id = await ipc<string>('folder:create', { name, parentId: parentId || null })
          // 立即更新响应式状态，触发 FolderTree TransitionGroup 缓入动画
          const ws = useWorkspaceStore()
          const newNode = { id, name, parentId: parentId || null, children: [] as any[], isExpanded: false }
          if (parentId) {
            const parent = findFolderInTree(ws.folders as any, parentId)
            if (parent) {
              parent.children.push(newNode as any)
              parent.isExpanded = true // 自动展开父目录，让用户看到新目录
            }
          } else {
            ws.folders.push(newNode as any)
          }
          return id
        } catch { return null }
      },

      renameFolder: async (folderId: string, newName: string) => {
        try {
          await ipc('folder:rename', folderId, newName)
          // 立即更新响应式状态
          const ws = useWorkspaceStore()
          const node = findFolderInTree(ws.folders as any, folderId)
          if (node) node.name = newName
          return true
        } catch { return false }
      },

      deleteFolder: async (folderId: string) => {
        try {
          await ipc('folder:remove', folderId)
          // 立即更新响应式状态，触发 FolderTree TransitionGroup 缓出动画
          const ws = useWorkspaceStore()
          removeFolderFromTree(ws.folders as any, folderId)
          // 如果删除的正是当前选中的目录，清空选择
          if (ws.selectedFolderId === folderId) {
            ws.selectedFolderId = null
            ws.selectedDocumentId = null
            ws.folderDocuments = []
            ws.currentDocumentData = null
          }
          return true
        } catch { return false }
      },

      listFolders: async () => {
        try {
          const tree = await ipc<any[]>('folder:tree')
          if (!tree || tree.length === 0) return '（暂无目录）'
          function formatTree(nodes: any[], indent: number): string {
            return nodes.map(n => {
              const prefix = '  '.repeat(indent)
              const children = n.children?.length ? '\n' + formatTree(n.children, indent + 1) : ''
              return `${prefix}📁 ${n.name}${children}`
            }).join('\n')
          }
          return formatTree(tree, 0)
        } catch { return '获取目录失败' }
      },

      listRecentNotes: async (limit?: number) => {
        try {
          const docs = await ipc<any[]>('document:listAll')
          if (!docs || docs.length === 0) return '（暂无笔记）'
          const sorted = docs.slice(0, limit || 10)
          return sorted.map((d, i) => {
            const time = d.updateTime ? new Date(d.updateTime).toLocaleString('zh-CN') : '未知'
            const folder = d.folderName || '未分类'
            return `${i + 1}. 《${d.title}》 - ${folder} - ${time}`
          }).join('\n')
        } catch { return '获取笔记列表失败' }
      },
    })

    return agent
  }

  /** 缓存当前文档内容，供 Agent 工具 read_current_document 使用 */
  let documentContextCache: string | null = null

  /* ---------- 知识库状态 ---------- */
  const kbEnabled = ref(false)
  const kbDocCount = ref(0)
  const kbChunkCount = ref(0)
  const kbEmbedCount = ref(0)
  const kbBuilding = ref(false)

  function loadKbToggle() {
    try {
      kbEnabled.value = localStorage.getItem(KB_STORAGE_KEY) === 'true'
    } catch { kbEnabled.value = false }
  }

  function setKbEnabled(val: boolean) {
    kbEnabled.value = val
    try { localStorage.setItem(KB_STORAGE_KEY, val ? 'true' : 'false') } catch {}
  }

  async function rebuildKb(): Promise<{ totalDocs: number; totalChunks: number; embedSuccess?: number }> {
    kbBuilding.value = true
    try {
      const result = await ipc<{ totalDocs: number; totalChunks: number; embedSuccess: number }>('kb:rebuild')
      kbDocCount.value = result.totalDocs
      kbChunkCount.value = result.totalChunks
      kbEmbedCount.value = result.embedSuccess ?? 0
      return result
    } finally {
      kbBuilding.value = false
    }
  }

  async function refreshKbStatus() {
    try {
      const st = await ipc<{ docCount: number; chunkCount: number; embedCount: number }>('kb:status')
      kbDocCount.value = st.docCount
      kbChunkCount.value = st.chunkCount
      kbEmbedCount.value = st.embedCount ?? 0
    } catch {}
  }

  /** 搜索知识库，返回相关块内容摘要 */
  async function searchKb(query: string, limit?: number): Promise<string> {
    try {
      const chunks = await ipc<any[]>('kb:search', query, limit || 6)
      if (!chunks || chunks.length === 0) return ''
      return chunks.map((c, i) =>
        `[文档「${c.document_title}」片段 ${i + 1}]:\n${c.content}`
      ).join('\n\n---\n\n')
    } catch {
      return ''
    }
  }

  /** 动态获取的模型列表（覆盖内置预置） */
  const dynamicModels = ref<{ id: string; name: string }[]>([])

  /* ---------- 供应商相关 ---------- */

  /** 当前供应商 */
  const provider = computed<ProviderType>(() => {
    return currentModel.value?.provider || 'deepseek'
  })

  /** 各供应商的模型列表（动态获取 + 预置回退） */
  const availableModels = computed<ModelConfig[]>(() => {
    const p = props_provider.value
    // 如果有动态获取的模型，优先使用
    if (dynamicModels.value.length > 0) {
      const mapped = dynamicModels.value.map(m => ({
        id: m.id,
        name: m.name,
        provider: p,
        model: m.id,
      }))
      // 为 OpenAI 兼容保留自定义选项
      if (p === 'openai-compatible') {
        mapped.push({ id: '_custom_', name: '自定义模型名', provider: 'openai-compatible', model: '' })
      }
      return mapped
    }
    // 回退到预置
    if (p === 'deepseek') return DEEPSEEK_MODELS
    if (p === 'gemini') return GEMINI_MODELS
    return OPENAI_MODELS
  })

  /** 从设置读取的供应商偏好（用于初始化模型列表筛选） */
  const props_provider = ref<ProviderType>('deepseek')

  const currentModel = computed(
    () => availableModels.value.find(m => m.id === selectedModelId.value) || availableModels.value[0]
  )

  /* ---------- 设置读写 ---------- */

  function getSettings(): Record<string, string> {
    try {
      const raw = localStorage.getItem(STORAGE_KEY)
      return raw ? JSON.parse(raw) : {}
    } catch {
      return {}
    }
  }

  function saveSettings(settings: Record<string, string>) {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings))
  }

  /** 获取当前供应商对应的 API Key */
  function getApiKey(): string {
    const s = getSettings()
    const p = props_provider.value
    if (p === 'gemini') return s.geminiApiKey || ''
    if (p === 'openai-compatible') return s.openaiApiKey || ''
    return s.deepseekApiKey || ''
  }

  /** 保存完整设置（由 SettingsDialog 统一调用） */
  function saveFullSettings(opts: {
    provider: ProviderType
    deepseekApiKey?: string
    deepseekBaseUrl?: string
    geminiApiKey?: string
    openaiBaseUrl?: string
    openaiApiKey?: string
    modelId?: string
    customModelName?: string
  }) {
    const s = getSettings()
    s.provider = opts.provider
    if (opts.deepseekApiKey !== undefined) s.deepseekApiKey = opts.deepseekApiKey
    if (opts.deepseekBaseUrl !== undefined) s.deepseekBaseUrl = opts.deepseekBaseUrl
    if (opts.geminiApiKey !== undefined) s.geminiApiKey = opts.geminiApiKey
    if (opts.openaiBaseUrl !== undefined) s.openaiBaseUrl = opts.openaiBaseUrl
    if (opts.openaiApiKey !== undefined) s.openaiApiKey = opts.openaiApiKey
    if (opts.modelId !== undefined) s.selectedModelId = opts.modelId
    if (opts.customModelName !== undefined) s.customModelName = opts.customModelName
    props_provider.value = opts.provider
    if (opts.modelId) selectedModelId.value = opts.modelId
    saveSettings(s)
    _apiKeyVersion.value++
  }

  function loadSettings() {
    const s = getSettings()
    props_provider.value = (s.provider as ProviderType) || 'deepseek'
    if (s.selectedModelId) selectedModelId.value = s.selectedModelId
  }

  function getDeepseekBaseUrl(): string {
    return getSettings().deepseekBaseUrl || 'https://api.deepseek.com'
  }

  function getOpenaiBaseUrl(): string {
    return getSettings().openaiBaseUrl || 'http://localhost:11434'
  }

  function getCustomModelName(): string {
    return getSettings().customModelName || ''
  }

  /* ---------- 动态获取模型 ---------- */

  async function fetchAvailableModels(apiKeyOverride?: string): Promise<{ ok: boolean; message: string; count: number }> {
    const p = props_provider.value
    const key = apiKeyOverride || getApiKey()
    if (!key) return { ok: false, message: '请先输入 API Key', count: 0 }

    try {
      let ids: string[] = []
      if (p === 'gemini') {
        ids = await listGeminiModels(key)
      } else {
        const baseUrl = p === 'deepseek' ? getDeepseekBaseUrl() : getOpenaiBaseUrl()
        ids = await listOpenAIModels(key, baseUrl)
      }

      if (ids.length === 0) return { ok: false, message: '未获取到可用模型', count: 0 }

      dynamicModels.value = ids.map(id => ({ id, name: id }))
      // 自动选中第一个
      if (ids.length > 0) {
        selectedModelId.value = ids[0]
      }
      return { ok: true, message: `获取成功，共 ${ids.length} 个模型`, count: ids.length }
    } catch (err: any) {
      return { ok: false, message: err.message || '获取失败', count: 0 }
    }
  }

  /* ---------- 连接测试 ---------- */

  async function testConnection(apiKeyOverride?: string, baseUrlOverride?: string): Promise<{ ok: boolean; message: string }> {
    const p = props_provider.value
    const key = apiKeyOverride || getApiKey()

    if (p === 'gemini') {
      if (!key) return { ok: false, message: '请输入 Gemini API Key' }
      const valid = await validateGeminiKey(key)
      return valid
        ? { ok: true, message: '验证成功！API Key 有效' }
        : { ok: false, message: 'API Key 无效，请检查后重试' }
    }

    // DeepSeek / OpenAI 兼容
    if (!key) return { ok: false, message: '请输入 API Key' }
    const baseUrl = baseUrlOverride || (p === 'deepseek' ? getDeepseekBaseUrl() : getOpenaiBaseUrl())
    const valid = await validateOpenAIKey(key, baseUrl)
    return valid
      ? { ok: true, message: '验证成功！API Key 有效' }
      : { ok: false, message: 'API Key 无效或无法连接到服务，请检查后重试' }
  }

  /* ---------- 聊天消息 ---------- */

  function saveMessages() {
    localStorage.setItem(CHAT_STORAGE_KEY, JSON.stringify(messages.value))
  }

  function loadMessages() {
    try {
      const raw = localStorage.getItem(CHAT_STORAGE_KEY)
      if (!raw) return
      const parsed = JSON.parse(raw)
      if (!Array.isArray(parsed)) return
      messages.value = parsed
        .filter(msg => msg && typeof msg.id === 'string' && (msg.role === 'user' || msg.role === 'assistant'))
        .map(msg => ({
          id: msg.id,
          role: msg.role,
          content: typeof msg.content === 'string' ? msg.content : '',
          timestamp: typeof msg.timestamp === 'number' ? msg.timestamp : Date.now(),
          isStreaming: false
        }))
    } catch {
      messages.value = []
    }
  }

  function generateId(): string {
    return Date.now().toString(36) + Math.random().toString(36).slice(2, 8)
  }

  /* ---------- 发送消息（Agent 驱动） ---------- */

  async function sendUserMessage(userText: string, documentContext?: string) {
    clearConfirmation() // 清理上一个待处理的确认
    const apiKey = getApiKey()
    if (!apiKey) {
      const labels: Record<ProviderType, string> = {
        deepseek: 'DeepSeek',
        gemini: 'Gemini',
        'openai-compatible': 'OpenAI 兼容'
      }
      error.value = `请先在设置中配置 ${labels[props_provider.value] || ''} API Key`
      return
    }

    const model = currentModel.value
    if (!model) { error.value = '未选择模型'; return }

    error.value = null

    // 缓存文档内容供 Agent 工具使用
    documentContextCache = documentContext || null

    const userMsg: ChatMessage = {
      id: generateId(),
      role: 'user',
      content: userText,
      timestamp: Date.now()
    }
    messages.value.push(userMsg)
    saveMessages()

    const assistantMsg: ChatMessage = {
      id: generateId(),
      role: 'assistant',
      content: '',
      timestamp: Date.now(),
      isStreaming: true,
      thinkingSteps: []
    }
    messages.value.push(assistantMsg)
    saveMessages()

    isStreaming.value = true

    // 构建对话历史（排除刚加的 assistant 消息）
    const history = messages.value
      .filter(m => m.id !== assistantMsg.id)
      .map(m => ({ role: m.role, content: m.content }))

    const controller = new AbortController()
    abortController.value = controller

    try {
      const agt = getAgent()

      const TOOL_ICONS: Record<string, string> = {
        search_kb: '🔍',
        search_documents: '📋',
        get_document_detail: '📄',
        read_current_document: '📖',
        create_note: '✏️',
        update_note: '📝',
        delete_note: '🗑️',
        create_folder: '📁',
        rename_folder: '✏️',
        delete_folder: '🗑️',
        list_folders: '📂',
        list_recent_notes: '📋',
      }
      const TOOL_LABELS: Record<string, string> = {
        search_kb: '搜索知识库',
        search_documents: '搜索文档',
        get_document_detail: '获取文档详情',
        read_current_document: '读取当前文档',
        create_note: '创建笔记',
        update_note: '修改笔记',
        delete_note: '删除笔记',
        create_folder: '创建目录',
        rename_folder: '重命名目录',
        delete_folder: '删除目录',
        list_folders: '列出目录',
        list_recent_notes: '列出最近笔记',
      }

      // 跟踪当前正在运行的 thinking step
      let currentStepId: string | null = null

      function addStep(type: 'tool_call' | 'tool_result', label: string, status: 'running' | 'done' | 'error') {
        const msg = messages.value.find(m => m.id === assistantMsg.id)
        if (!msg) return
        if (!msg.thinkingSteps) msg.thinkingSteps = []
        const id = generateId()
        msg.thinkingSteps.push({ id, type, label, status })
        return id
      }

      function updateStep(stepId: string, updates: Partial<ThinkingStep>) {
        const msg = messages.value.find(m => m.id === assistantMsg.id)
        if (!msg || !msg.thinkingSteps) return
        const step = msg.thinkingSteps.find(s => s.id === stepId)
        if (!step) return
        Object.assign(step, updates)
      }

      await agt.chat(
        userText,
        history,
        {
          onChunk: (chunk: string) => {
            const msg = messages.value.find(m => m.id === assistantMsg.id)
            if (msg) {
              msg.content += chunk
              // 不在此处 saveMessages() — 流式过程中写 localStorage 会严重阻塞 UI
            }
          },
          onToolCall: (toolName, args) => {
            const icon = TOOL_ICONS[toolName] || '🔧'
            const labelBase = TOOL_LABELS[toolName] || toolName
            let detail = ''
            try {
              const parsed = JSON.parse(args)
              if (parsed.query) detail = parsed.query
              else if (parsed.title) detail = `「${parsed.title}」`
              else if (parsed.name) detail = `「${parsed.name}」`
            } catch {}
            const label = `${icon} ${labelBase}${detail ? ' ' + detail : ''}`
            const stepId = addStep('tool_call', label, 'running')
            currentStepId = stepId || null
          },
          onToolResult: (_toolName, result) => {
            // 完成当前步骤
            if (currentStepId) {
              updateStep(currentStepId, { status: result.startsWith('操作已取消') ? 'error' : 'done' })
              currentStepId = null
            }
            // tool_result 步骤只取首行前 50 字
            const firstLine = result.split('\n')[0] || ''
            const summary = firstLine.length > 50 ? firstLine.slice(0, 50) + '…' : firstLine
            if (summary && !summary.includes('服务未就绪') && !summary.includes('未找到')) {
              addStep('tool_result', summary, 'done')
            }
          },
          onError: (errMsg) => {
            console.warn('[Agent] 错误:', errMsg)
          },
        },
        controller.signal
      )
    } catch (err: any) {
      if (err.name !== 'AbortError') {
        error.value = err.message || '请求失败'
        const msg = messages.value.find(m => m.id === assistantMsg.id)
        if (msg && !msg.content.trim()) {
          const idx = messages.value.indexOf(msg)
          if (idx !== -1) messages.value.splice(idx, 1)
          saveMessages()
        }
      }
    } finally {
      isStreaming.value = false
      abortController.value = null
      documentContextCache = null
      const msg = messages.value.find(m => m.id === assistantMsg.id)
      if (msg) {
        msg.isStreaming = false
      }
      saveMessages()
    }
  }

  function cancelGeneration() {
    abortController.value?.abort()
    clearConfirmation()
  }

  function clearChat() {
    messages.value = []
    error.value = null
    saveMessages()
  }

  // 初始化
  loadMessages()
  loadSettings()
  loadKbToggle()
  void refreshKbStatus()

  const hasApiKey = computed(() => {
    void _apiKeyVersion.value
    return !!getApiKey()
  })

  function setModel(modelId: string) {
    if (!availableModels.value.some(m => m.id === modelId)) return
    selectedModelId.value = modelId
    const s = getSettings()
    s.selectedModelId = modelId
    saveSettings(s)
  }

  return {
    messages,
    selectedModelId,
    isStreaming,
    error,
    provider,
    availableModels,
    currentModel,
    hasApiKey,
    getApiKey,
    setModel,
    saveFullSettings,
    testConnection,
    getDeepseekBaseUrl,
    getOpenaiBaseUrl,
    getCustomModelName,
    dynamicModels,
    fetchAvailableModels,
    sendUserMessage,
    cancelGeneration,
    clearChat,

    // 知识库
    kbEnabled,
    kbDocCount,
    kbChunkCount,
    kbEmbedCount,
    kbBuilding,
    setKbEnabled,
    rebuildKb,
    refreshKbStatus,
  }
})
