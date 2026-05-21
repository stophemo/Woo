import type { ChatMessage } from '../types/ai'

const BASE_URL = 'https://generativelanguage.googleapis.com/v1beta'

/**
 * 验证 API Key 是否有效
 */
export async function validateApiKey(apiKey: string): Promise<boolean> {
  try {
    const res = await fetch(`${BASE_URL}/models?key=${apiKey}`)
    return res.ok
  } catch {
    return false
  }
}

/**
 * 去除 HTML 标签，提取纯文本
 */
export function stripHtml(html: string): string {
  const tmp = document.createElement('div')
  tmp.innerHTML = html
  return tmp.textContent || tmp.innerText || ''
}

/**
 * 发送消息并流式接收响应
 */
export async function sendMessage(
  apiKey: string,
  model: string,
  messages: ChatMessage[],
  onChunk: (text: string) => void,
  signal?: AbortSignal
): Promise<string> {
  // 构建 Gemini 请求体，映射 role: assistant -> model
  const contents = messages.map(msg => ({
    role: msg.role === 'assistant' ? 'model' : 'user',
    parts: [{ text: msg.content }]
  }))

  const url = `${BASE_URL}/models/${model}:streamGenerateContent?alt=sse&key=${apiKey}`

  const res = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      contents,
      generationConfig: {
        temperature: 0.7,
        maxOutputTokens: 4096
      }
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

  const reader = res.body!.getReader()
  const decoder = new TextDecoder()
  let fullText = ''
  let buffer = ''

  while (true) {
    const { done, value } = await reader.read()
    if (done) break

    buffer += decoder.decode(value, { stream: true })
    const lines = buffer.split('\n')
    // 保留最后一个可能不完整的行
    buffer = lines.pop() || ''

    for (const line of lines) {
      const trimmed = line.trim()
      if (!trimmed.startsWith('data: ')) continue

      const jsonStr = trimmed.slice(6)
      if (jsonStr === '[DONE]') continue

      try {
        const parsed = JSON.parse(jsonStr)
        const text = parsed?.candidates?.[0]?.content?.parts?.[0]?.text
        if (text) {
          fullText += text
          onChunk(text)
        }
      } catch {
        // 跳过无法解析的行
      }
    }
  }

  return fullText
}
