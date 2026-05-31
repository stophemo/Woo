import type { ChatMessage } from '../types/ai'

const DEFAULT_BASE_URL = 'https://api.deepseek.com'

function normalizeBaseUrl(baseUrl?: string): string {
  return (baseUrl || DEFAULT_BASE_URL).trim().replace(/\/+$/, '')
}

/**
 * 验证 API Key（适用于 DeepSeek / OpenAI 兼容接口）。
 * 通过列出可用模型来判断 key 是否有效。
 */
export async function validateApiKey(apiKey: string, baseUrl?: string): Promise<boolean> {
  try {
    const url = `${normalizeBaseUrl(baseUrl)}/v1/models`
    const res = await fetch(url, {
      headers: { Authorization: `Bearer ${apiKey}` }
    })
    return res.ok
  } catch {
    return false
  }
}

/**
 * 发送流式消息（使用 OpenAI 兼容 API 格式）。
 * DeepSeek / Ollama / 任何 OpenAI 兼容接口均可使用此函数。
 */
export async function sendMessage(
  apiKey: string,
  baseUrl: string,
  model: string,
  messages: ChatMessage[],
  onChunk: (text: string) => void,
  signal?: AbortSignal
): Promise<string> {
  const url = `${normalizeBaseUrl(baseUrl)}/v1/chat/completions`

  const payloadMessages = messages.map(msg => ({
    role: msg.role,
    content: msg.content
  }))

  const res = await fetch(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${apiKey}`
    },
    body: JSON.stringify({
      model,
      messages: payloadMessages,
      stream: true,
      temperature: 0.7
    }),
    signal
  })

  if (!res.ok) {
    if (res.status === 401 || res.status === 403) {
      throw new Error('API Key 无效或已过期，请在设置中重新配置')
    }
    if (res.status === 429) {
      throw new Error('请求过于频繁，请稍后再试')
    }
    throw new Error(`API 请求失败 (${res.status})，请稍后重试`)
  }

  const reader = res.body?.getReader()
  if (!reader) throw new Error('响应流不可用')

  const decoder = new TextDecoder()
  let fullText = ''
  let buffer = ''

  while (true) {
    const { done, value } = await reader.read()
    if (done) break

    buffer += decoder.decode(value, { stream: true })
    const lines = buffer.split('\n')
    buffer = lines.pop() || ''

    for (const rawLine of lines) {
      const line = rawLine.trim()
      if (!line.startsWith('data: ')) continue

      const data = line.slice(6).trim()
      if (!data || data === '[DONE]') continue

      try {
        const parsed = JSON.parse(data)
        const chunk = parsed?.choices?.[0]?.delta?.content
        if (chunk) {
          fullText += chunk
          onChunk(chunk)
        }
      } catch {
        // Ignore malformed SSE events.
      }
    }
  }

  return fullText
}
