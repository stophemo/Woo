import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { ChatMessage, ModelConfig } from '../types/ai'
import { sendMessage as geminiSendMessage, stripHtml } from '../services/gemini'

const STORAGE_KEY = 'ai-settings'

export const useAiChatStore = defineStore('aiChat', () => {
  // ============ 状态 ============

  const messages = ref<ChatMessage[]>([])
  const selectedModelId = ref<string>('auto')
  const isStreaming = ref(false)
  const error = ref<string | null>(null)
  const abortController = ref<AbortController | null>(null)
  // 用于触发 hasApiKey 响应式更新的版本号
  const _apiKeyVersion = ref(0)

  // ============ 计算属性 ============

  const availableModels = computed<ModelConfig[]>(() => [
    { id: 'auto', name: 'Auto', provider: 'gemini', model: 'gemini-2.0-flash' },
    { id: 'gemini-flash', name: 'Gemini 2.0 Flash', provider: 'gemini', model: 'gemini-2.0-flash' }
  ])

  const currentModel = computed(() => {
    if (selectedModelId.value === 'auto') {
      return availableModels.value[1] // 默认用 Gemini Flash
    }
    return availableModels.value.find(m => m.id === selectedModelId.value) || availableModels.value[1]
  })

  const hasApiKey = computed(() => {
    return !!getApiKey()
  })

  // ============ API Key 管理 ============

  function getApiKey(): string {
    try {
      const raw = localStorage.getItem(STORAGE_KEY)
      if (!raw) return ''
      const settings = JSON.parse(raw)
      return settings.geminiApiKey || ''
    } catch {
      return ''
    }
  }

  function saveApiKey(key: string) {
    const raw = localStorage.getItem(STORAGE_KEY)
    let settings: Record<string, string> = {}
    try {
      if (raw) settings = JSON.parse(raw)
    } catch { /* ignore */ }
    settings.geminiApiKey = key
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings))
    _apiKeyVersion.value++ // 触发 hasApiKey 响应式更新
  }

  // ============ 模型选择 ============

  function setModel(modelId: string) {
    selectedModelId.value = modelId
  }

  // ============ 聊天操作 ============

  function generateId(): string {
    return Date.now().toString(36) + Math.random().toString(36).slice(2, 8)
  }

  async function sendUserMessage(userText: string, documentContext?: string) {
    const apiKey = getApiKey()
    if (!apiKey) {
      error.value = '请先在设置中配置 API Key'
      return
    }

    error.value = null

    // 创建用户消息
    const userMsg: ChatMessage = {
      id: generateId(),
      role: 'user',
      content: userText,
      timestamp: Date.now()
    }
    messages.value.push(userMsg)

    // 创建助手占位消息
    const assistantMsg: ChatMessage = {
      id: generateId(),
      role: 'assistant',
      content: '',
      timestamp: Date.now(),
      isStreaming: true
    }
    messages.value.push(assistantMsg)

    isStreaming.value = true

    // 构建发送给 API 的消息列表
    const apiMessages: ChatMessage[] = []

    // 如果有文档上下文且是首次消息，注入上下文
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
      // 发送完整对话历史（排除 streaming 占位）
      for (const msg of messages.value) {
        if (msg.id === assistantMsg.id) continue
        if (msg.content.trim()) {
          apiMessages.push(msg)
        }
      }
    }

    // 创建 AbortController
    const controller = new AbortController()
    abortController.value = controller

    try {
      await geminiSendMessage(
        apiKey,
        currentModel.value.model,
        apiMessages,
        (chunk: string) => {
          // 找到助手消息并追加内容
          const msg = messages.value.find(m => m.id === assistantMsg.id)
          if (msg) {
            msg.content += chunk
          }
        },
        controller.signal
      )
    } catch (err: any) {
      if (err.name === 'AbortError') {
        // 用户主动取消，不视为错误
      } else {
        error.value = err.message || '请求失败，请稍后重试'
        // 如果助手消息为空，移除它
        const msg = messages.value.find(m => m.id === assistantMsg.id)
        if (msg && !msg.content.trim()) {
          const idx = messages.value.indexOf(msg)
          if (idx !== -1) messages.value.splice(idx, 1)
        }
      }
    } finally {
      isStreaming.value = false
      abortController.value = null
      // 标记流式完成
      const msg = messages.value.find(m => m.id === assistantMsg.id)
      if (msg) {
        msg.isStreaming = false
      }
    }
  }

  function cancelGeneration() {
    abortController.value?.abort()
  }

  function clearChat() {
    messages.value = []
    error.value = null
  }

  return {
    // 状态
    messages,
    selectedModelId,
    isStreaming,
    error,
    // 计算属性
    availableModels,
    currentModel,
    hasApiKey,
    // 操作
    getApiKey,
    saveApiKey,
    setModel,
    sendUserMessage,
    cancelGeneration,
    clearChat
  }
})
