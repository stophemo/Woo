/**
 * Google Gemini API 客户端。
 *
 * Gemini API 使用 API key 作为查询参数（?key=xxx），
 * 响应格式与 OpenAI 兼容 API 不同，需单独解析 SSE 流。
 */

const DEFAULT_BASE_URL = 'https://generativelanguage.googleapis.com'

/** 验证 Gemini API Key 是否有效 */
export async function validateGeminiKey(apiKey: string): Promise<boolean> {
  try {
    const url = `${DEFAULT_BASE_URL}/v1beta/models?key=${encodeURIComponent(apiKey)}`
    const res = await fetch(url)
    return res.ok
  } catch {
    return false
  }
}

/**
 * 获取 Gemini 可用模型列表。
 * 只返回支持文本生成的模型（如 gemini-*），排除 embed、tuning 等。
 */
export async function listGeminiModels(apiKey: string): Promise<string[]> {
  const url = `${DEFAULT_BASE_URL}/v1beta/models?key=${encodeURIComponent(apiKey)}`
  const res = await fetch(url)
  if (!res.ok) throw new Error(`获取模型列表失败 (${res.status})`)
  const body = await res.json()
  if (body?.models && Array.isArray(body.models)) {
    return body.models
      .map((m: any) => m.name?.replace(/^models\//, ''))
      .filter((id: string) => id && id.includes('gemini') && !id.includes('embed') && !id.includes('tuning'))
      .sort()
  }
  throw new Error('返回格式异常')
}

/**
 * 发送流式消息到 Gemini API。
 *
 * @param apiKey  - Gemini API Key
 * @param model   - 模型名，如 "gemini-2.5-flash"
 * @param messages - 对话历史 [{ role, content }]
 * @param onChunk  - 每次收到文本块的回调
 * @param signal   - AbortSignal 用于取消请求
 * @returns 完整响应文本
 */
export async function sendGeminiMessage(
  apiKey: string,
  model: string,
  messages: { role: string; content: string }[],
  onChunk: (text: string) => void,
  signal?: AbortSignal
): Promise<string> {
  const url = `${DEFAULT_BASE_URL}/v1beta/models/${model}:streamGenerateContent?key=${encodeURIComponent(apiKey)}&alt=sse`

  // 将通用格式转换为 Gemini contents 格式
  const contents = messages
    .filter(m => m.content.trim())
    .map(m => ({
      role: m.role === 'assistant' ? 'model' : 'user',
      parts: [{ text: m.content }]
    }))

  const res = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ contents }),
    signal
  })

  if (!res.ok) {
    if (res.status === 403 || res.status === 401) {
      throw new Error('API Key 无效或已过期，请在设置中重新配置')
    }
    if (res.status === 429) {
      throw new Error('请求过于频繁，请稍后再试')
    }
    throw new Error(`Gemini API 请求失败 (${res.status})，请稍后重试`)
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
      if (!data) continue

      try {
        const parsed = JSON.parse(data)
        const candidates = parsed?.candidates
        if (!candidates || !candidates.length) continue
        const chunk = candidates[0]?.content?.parts?.[0]?.text
        if (chunk) {
          fullText += chunk
          onChunk(chunk)
        }
      } catch {
        // 忽略格式异常的 SSE 事件
      }
    }
  }

  return fullText
}

/** 去除 HTML 标签，提取纯文本（原 gemini.ts 中的工具函数） */
export function stripHtml(html: string): string {
  const tmp = document.createElement('div')
  tmp.innerHTML = html
  return tmp.textContent || tmp.innerText || ''
}
