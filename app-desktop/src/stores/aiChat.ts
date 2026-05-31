import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { ChatMessage, ModelConfig, ProviderType } from '../types/ai'
import { sendMessage as deepseekSendMessage, validateApiKey as validateOpenAIKey, listModels as listOpenAIModels } from '../services/deepseek'
import { sendGeminiMessage, validateGeminiKey, stripHtml, listGeminiModels } from '../services/gemini'

const STORAGE_KEY = 'ai-settings'
const CHAT_STORAGE_KEY = 'ai-chat-messages'
const KB_STORAGE_KEY = 'ai-kb-enabled'

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
  async function searchKb(query: string): Promise<string> {
    try {
      const chunks = await ipc<any[]>('kb:search', query, 6)
      if (!chunks || chunks.length === 0) return ''
      // 合并为上下文文本
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

  /* ---------- 发送消息 ---------- */

  async function sendUserMessage(userText: string, documentContext?: string) {
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

    error.value = null

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
      isStreaming: true
    }
    messages.value.push(assistantMsg)
    saveMessages()

    isStreaming.value = true

    // 构建 API 消息列表
    const apiMessages: ChatMessage[] = []
    const isFirstUserMsg = messages.value.filter(m => m.role === 'user').length === 1

    // 收集上下文片段
    const contextParts: string[] = []

    // 1. 当前编辑的文档内容
    if (documentContext && isFirstUserMsg) {
      const plainText = stripHtml(documentContext)
      if (plainText.trim()) {
        const contextText = plainText.length > 3000 ? plainText.slice(0, 3000) + '...' : plainText
        contextParts.push(`[当前编辑的文档]:\n${contextText}`)
      }
    }

    // 2. 知识库检索（仅在首次消息或 KB 启用时）
    if (kbEnabled.value && isFirstUserMsg) {
      try {
        const kbContext = await searchKb(userText)
        if (kbContext) contextParts.push(`[知识库相关文档]:\n${kbContext}`)
      } catch { /* KB 搜索失败不影响主流程 */ }
    }

    if (contextParts.length > 0) {
      const combinedContext = contextParts.join('\n\n---\n\n')
      apiMessages.push({
        id: 'ctx',
        role: 'user',
        content: `以下是与问题相关的文档内容，请结合上下文回答我的问题（如有不足可结合自身知识补充，但优先参考以下资料）：\n\n${combinedContext}\n\n我的问题是：${userText}`,
        timestamp: Date.now()
      })
    } else {
      for (const msg of messages.value) {
        if (msg.id === assistantMsg.id) continue
        if (msg.content.trim()) {
          apiMessages.push(msg)
        }
      }
      // 如果是首次消息但没有上下文，直接推送原始消息
      if (isFirstUserMsg && apiMessages.length === 0) {
        apiMessages.push(userMsg)
      }
    }

    const controller = new AbortController()
    abortController.value = controller

    try {
      const model = currentModel.value
      if (!model) throw new Error('未选择模型')

      if (provider.value === 'gemini') {
        await sendGeminiMessage(
          apiKey,
          model.model,  // e.g. "gemini-2.5-flash"
          apiMessages.map(m => ({ role: m.role, content: m.content })),
          (chunk: string) => {
            const msg = messages.value.find(m => m.id === assistantMsg.id)
            if (msg) {
              msg.content += chunk
              saveMessages()
            }
          },
          controller.signal
        )
      } else {
        // DeepSeek / OpenAI 兼容 — 使用相同的 OpenAI 兼容 API
        const baseUrl = provider.value === 'deepseek' ? getDeepseekBaseUrl() : getOpenaiBaseUrl()
        const actualModel = model.id === '_custom_' ? getCustomModelName() || model.model : model.model
        await deepseekSendMessage(
          apiKey,
          baseUrl,
          actualModel,
          apiMessages,
          (chunk: string) => {
            const msg = messages.value.find(m => m.id === assistantMsg.id)
            if (msg) {
              msg.content += chunk
              saveMessages()
            }
          },
          controller.signal
        )
      }
    } catch (err: any) {
      if (err.name !== 'AbortError') {
        const detail = err.cause ? ` (${err.cause})` : ''
        error.value = (err.message || '请求失败') + detail
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
      const msg = messages.value.find(m => m.id === assistantMsg.id)
      if (msg) {
        msg.isStreaming = false
      }
      saveMessages()
    }
  }

  function cancelGeneration() {
    abortController.value?.abort()
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
