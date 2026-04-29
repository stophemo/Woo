export interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: number
  isStreaming?: boolean
}

export interface ModelConfig {
  id: string
  name: string
  provider: 'gemini'
  model: string
}

export interface AiSettings {
  geminiApiKey: string
  selectedModelId: string
}
