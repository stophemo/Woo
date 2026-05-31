export interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: number
  isStreaming?: boolean
}

export type ProviderType = 'deepseek' | 'gemini' | 'openai-compatible'

export interface ModelConfig {
  id: string
  name: string
  provider: ProviderType
  model: string
}

export interface AiSettings {
  provider: ProviderType
  deepseekApiKey: string
  deepseekBaseUrl: string
  geminiApiKey: string
  openaiBaseUrl: string
  openaiApiKey: string
  selectedModelId: string
  customModelName: string
}
