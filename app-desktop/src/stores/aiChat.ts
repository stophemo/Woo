import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { ChatMessage, ModelConfig } from '../types/ai'
import { sendMessage as deepseekSendMessage } from '../services/deepseek'
import { stripHtml } from '../services/gemini'

const STORAGE_KEY = 'ai-settings'
const CHAT_STORAGE_KEY = 'ai-chat-messages'
const DEFAULT_DEEPSEEK_BASE_URL = 'https://api.deepseek.com'

export const useAiChatStore = defineStore('aiChat', () => {
  const messages = ref<ChatMessage[]>([])
  const selectedModelId = ref<string>('deepseek-v4-pro')
  const isStreaming = ref(false)
  const error = ref<string | null>(null)
  const abortController = ref<AbortController | null>(null)
  const _apiKeyVersion = ref(0)

  const availableModels = computed<ModelConfig[]>(() => [
    // DeepSeek 暂无公开 "v4 pro/flash" API 模型名，这里映射到当前可用官方模型参数。
    {
      id: 'deepseek-v4-pro',
      name: 'deepseek v4 pro',
      provider: 'deepseek',
      model: 'deepseek-reasoner'
    },
    { id: 'deepseek-v4-flash', name: 'deepseek v4 flash', provider: 'deepseek', model: 'deepseek-chat' }
  ])

  const currentModel = computed(
    () => availableModels.value.find(m => m.id === selectedModelId.value) || availableModels.value[0]
  )

  const hasApiKey = computed(() => {
    void _apiKeyVersion.value
    return !!getApiKey()
  })

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

  function getApiKey(): string {
    const settings = getSettings()
    return settings.deepseekApiKey || ''
  }

  function saveApiKey(key: string) {
    const settings = getSettings()
    settings.deepseekApiKey = key
    if (!settings.deepseekBaseUrl) {
      settings.deepseekBaseUrl = DEFAULT_DEEPSEEK_BASE_URL
    }
    saveSettings(settings)
    _apiKeyVersion.value++
  }

  function getBaseUrl(): string {
    const settings = getSettings()
    return settings.deepseekBaseUrl || DEFAULT_DEEPSEEK_BASE_URL
  }

  function saveBaseUrl(baseUrl: string) {
    const settings = getSettings()
    settings.deepseekBaseUrl = baseUrl.trim() || DEFAULT_DEEPSEEK_BASE_URL
    saveSettings(settings)
  }

  function setModel(modelId: string) {
    if (!availableModels.value.some(model => model.id === modelId)) return
    selectedModelId.value = modelId
    const settings = getSettings()
    settings.selectedModelId = modelId
    saveSettings(settings)
  }

  function loadSelectedModel() {
    const settings = getSettings()
    if (settings.selectedModelId && availableModels.value.some(model => model.id === settings.selectedModelId)) {
      selectedModelId.value = settings.selectedModelId
    }
  }

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

  async function sendUserMessage(userText: string, documentContext?: string) {
    const apiKey = getApiKey()
    if (!apiKey) {
      error.value = '请先在设置中配置 DeepSeek API Key'
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

    const apiMessages: ChatMessage[] = []
    if (documentContext && messages.value.filter(m => m.role === 'user').length === 1) {
      const plainText = stripHtml(documentContext)
      if (plainText.trim()) {
        const contextText = plainText.length > 4000 ? plainText.slice(0, 4000) + '...' : plainText
        apiMessages.push({
          id: 'ctx',
          role: 'user',
          content: `以下是我正在编辑的文档内容，请结合此上下文回答我的问题：\n\n${contextText}\n\n我的问题是：${userText}`,
          timestamp: Date.now()
        })
      } else {
        apiMessages.push(userMsg)
      }
    } else {
      for (const msg of messages.value) {
        if (msg.id === assistantMsg.id) continue
        if (msg.content.trim()) {
          apiMessages.push(msg)
        }
      }
    }

    const controller = new AbortController()
    abortController.value = controller

    try {
      await deepseekSendMessage(
        apiKey,
        getBaseUrl(),
        currentModel.value.model,
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
    } catch (err: any) {
      if (err.name !== 'AbortError') {
        error.value = err.message || '请求失败，请稍后重试'
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

  loadMessages()
  loadSelectedModel()

  return {
    messages,
    selectedModelId,
    isStreaming,
    error,
    availableModels,
    currentModel,
    hasApiKey,
    getApiKey,
    saveApiKey,
    getBaseUrl,
    saveBaseUrl,
    setModel,
    sendUserMessage,
    cancelGeneration,
    clearChat
  }
})
