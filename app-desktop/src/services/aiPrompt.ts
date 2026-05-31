/**
 * 通用 AI 提示工具。
 *
 * 使用当前 AI Chat Store 中配置的供应商、API Key 和模型，
 * 发送一次性 prompt 并返回完整响应（非流式）。
 * 用于 AI 续写补全、右键格式调整等场景。
 */
import { sendMessage as openaiSendMessage } from './deepseek'
import { sendGeminiMessage } from './gemini'
import { useAiChatStore } from '../stores/aiChat'

/**
 * 发送一次性 AI 提示，返回完整响应文本。
 *
 * @param systemPrompt  - 系统指令（如"请优化以下文本的格式"）
 * @param userText      - 用户输入/选中文本
 * @returns AI 响应文本
 */
export async function sendAiPrompt(systemPrompt: string, userText: string): Promise<string> {
  const store = useAiChatStore()
  const apiKey = store.getApiKey()
  if (!apiKey) throw new Error('请先在设置中配置 API Key')

  const model = store.currentModel
  if (!model) throw new Error('未选择模型')

  const messages = [
    { id: 'sys', role: 'assistant' as const, content: systemPrompt, timestamp: Date.now() },
    { id: 'usr', role: 'user' as const, content: userText, timestamp: Date.now() },
  ]

  const provider = store.provider
  let fullText = ''
  const onChunk = (chunk: string) => { fullText += chunk }

  if (provider === 'gemini') {
    await sendGeminiMessage(
      apiKey,
      model.model,
      messages.map(m => ({ role: m.role, content: m.content })),
      onChunk
    )
  } else {
    const baseUrl = provider === 'deepseek'
      ? store.getDeepseekBaseUrl()
      : store.getOpenaiBaseUrl()
    const actualModel = model.id === '_custom_'
      ? (store.getCustomModelName() || model.model)
      : model.model
    await openaiSendMessage(apiKey, baseUrl, actualModel, messages, onChunk)
  }

  return fullText
}
