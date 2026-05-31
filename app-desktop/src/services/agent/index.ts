/**
 * 轻量 Agent 引擎。
 *
 * 工作流程（全程流式）：
 *   1. 发流式请求（含 tool 定义）
 *   2. 实时流式输出文本
 *   3. 流结束后检测 tool_calls：
 *      - 有工具调用 → 执行 → 用结果发新流式请求 → 返回 2
 *      - 无工具调用 → 结束
 *
 * 遵循 MCP 风格的 Tool 定义，工具在渲染进程通过 IPC 执行。
 */
import type {
  AgentConfig,
  AgentMessage,
  Tool,
  ToolCall,
  StreamCallbacks,
  ContextProvider,
} from './types'
import { BUILTIN_TOOLS, setContextProvider } from './tools'
import { agentLog } from './logger'

// ─── System Prompt ─────────────────────────────────

const DEFAULT_SYSTEM_PROMPT = `你是 Woo 无我笔记的 AI 助手，专注于帮助用户写作和整理知识。

当前日期：${new Date().toLocaleDateString('zh-CN')}

## 核心能力
- 拥有用户的笔记知识库，可以通过搜索工具查找相关内容
- 可以读取用户当前正在编辑的文档
- 可以搜索文档标题和获取文档详情
- **可以创建、修改、删除笔记和目录**
- **可以基于笔记内容回答用户的提问**

## 可用工具

### 查询工具
- search_kb：语义搜索知识库内容
- search_documents：按标题搜索文档
- get_document_detail：获取文档的创建/修改时间等详情
- read_current_document：读取当前打开的文档
- list_folders：列出目录层级结构
- list_recent_notes：列出最近修改的笔记

### 笔记操作工具
- create_note：创建新笔记（可指定目录，不指定则放入默认目录）
- update_note：更新笔记完整内容（⚠️ 需要用户确认）
- delete_note：将笔记移入废纸篓（⚠️ 需要用户确认）

### 目录操作工具
- create_folder：创建新目录
- rename_folder：重命名目录
- delete_folder：删除目录及其笔记（⚠️ 需要用户确认）

## ⚠️ 安全确认机制
标记"需要用户确认"的操作会弹出确认对话框。执行这些操作前，必须先调用工具完成操作，系统会自动弹出确认框。如果用户取消，工具会返回"操作已取消"。不要拒绝执行用户要求的修改操作——你只管发出工具调用，确认框会自动处理。

## 内容格式
- 写入笔记的内容（update_note 的 newContent、create_note 的 content）使用 **Markdown 格式**，不需要写 HTML
- 系统会自动将 Markdown 转为编辑器兼容的 HTML
- 支持的标准 Markdown 语法：标题、列表、表格、代码块、引用、加粗、斜体、行内代码

## 行为原则
### 回答格式
- **直接给出答案**，不要加"让我看看""我先查一下""根据搜索结果""我找到"等前缀
- 工具调用是后台自动执行的，用户看不到。不要告诉用户你在做什么——只给出结果
- 例如用户问"项目推进写了多少天"，你搜完直接答"从3月1日到今天共92天"——不要加任何搜索过程描述

### 工具使用规则
1. **能不查就不查**：闲聊、问工具本身怎么用、写作建议等，直接回答，不要调用工具
2. **需要信息时再查**：确实需要笔记内容才调用工具搜索
3. 回答时引用笔记内容用【来源：《标题》】格式
4. 写作（润色、续写、改写等）直接操作，不要解释
5. 翻译、总结给出干净结果
6. 不确定时坦诚说"不知道"
7. 回答简洁，中文优先
8. **每次工具调用的 query 必须从当前问题提取新关键词**，不要复用旧词
9. 创建笔记前先 list_folders 了解目录结构
10. 删除/修改前先 search_documents 确认笔记存在`

// ─── 工具格式转换 ──────────────────────────────────

function toOpenAITools(tools: Tool[]) {
  return tools.map((t) => ({
    type: 'function' as const,
    function: {
      name: t.name,
      description: t.description,
      parameters: t.parameters,
    },
  }))
}

function toGeminiTools(tools: Tool[]) {
  return [
    {
      functionDeclarations: tools.map((t) => ({
        name: t.name,
        description: t.description,
        parameters: t.parameters,
      })),
    },
  ]
}

// ─── Agent 核心 ────────────────────────────────────

export class Agent {
  private config: AgentConfig
  private tools: Tool[]
  private systemPrompt: string

  constructor(config: AgentConfig, tools: Tool[] = BUILTIN_TOOLS) {
    this.config = config
    this.tools = tools
    this.systemPrompt = config.systemPrompt || DEFAULT_SYSTEM_PROMPT
  }

  setContextProvider(provider: ContextProvider) {
    setContextProvider(provider)
  }

  updateConfig(config: Partial<AgentConfig>) {
    Object.assign(this.config, config)
  }

  /** 执行单个工具调用 */
  private async executeTool(tc: ToolCall): Promise<string> {
    const tool = this.tools.find((t) => t.name === tc.function.name)
    if (!tool) return `错误: 未知工具 "${tc.function.name}"`
    try {
      const args = JSON.parse(tc.function.arguments)
      return await tool.handler(args || {})
    } catch (e: any) {
      return `工具执行错误: ${e.message}`
    }
  }

  /** 构建消息列表 */
  private buildMessages(
    history: { role: string; content: string }[],
    userText: string
  ): AgentMessage[] {
    const msgs: AgentMessage[] = [{ role: 'system', content: this.systemPrompt }]
    for (const h of history) {
      if (h.role === 'user' || h.role === 'assistant') {
        msgs.push({ role: h.role as 'user' | 'assistant', content: h.content })
      }
    }
    msgs.push({ role: 'user', content: userText })
    return msgs
  }

  /** 从 SSE delta 累积 tool_calls（兼容 id/index 两种标识方式） */
  private accumulateToolCall(delta: any, accumulated: ToolCall[]) {
    if (!delta?.tool_calls) return
    for (const tc of delta.tool_calls) {
      // 查找已有：优先 id，其次 index
      const existing = tc.id
        ? accumulated.find((a) => a.id === tc.id)
        : accumulated[tc.index]
      if (existing) {
        existing.function.arguments += tc.function?.arguments || ''
      } else if (tc.function?.name) {
        // 第一个 delta 块有完整 name + arguments 开头
        const call: ToolCall = {
          id: tc.id || `tc_${Date.now()}_${tc.index}`,
          type: 'function',
          function: {
            name: tc.function.name,
            arguments: tc.function?.arguments || '',
          },
        }
        accumulated.push(call)
        // 用 index 缓存引用，供后续 delta 追加参数
        if (typeof tc.index === 'number') {
          ;(accumulated as any)[tc.index] = call
        }
      }
    }
  }

  /** OpenAI 兼容 / DeepSeek 流式请求 */
  private async openaiStream(
    messages: AgentMessage[],
    onChunk: (text: string) => void,
    signal?: AbortSignal
  ): Promise<{ fullText: string; toolCalls: ToolCall[] }> {
    const { apiKey, baseUrl, model } = this.config
    const url = `${(baseUrl || 'https://api.deepseek.com').replace(/\/+$/, '')}/v1/chat/completions`

    agentLog.log(`[Agent] API 请求 → ${url.replace(/\/v1.*/, '/...')} | 模型: ${model} | 附带工具: ${this.tools.length > 0}`)

    const body: Record<string, unknown> = {
      model,
      messages: messages.map((m) => {
        const base: Record<string, unknown> = { role: m.role, content: m.content }
        if (m.tool_call_id) base.tool_call_id = m.tool_call_id
        if (m.name) base.name = m.name
        if (m.tool_calls) base.tool_calls = m.tool_calls
        return base
      }),
      stream: true,
      temperature: 0.7,
    }

    // 每一轮都附带 tool 定义，让模型始终可用 function calling
    if (this.tools.length > 0) {
      body.tools = toOpenAITools(this.tools)
    }

    const res = await fetch(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${apiKey}`,
      },
      body: JSON.stringify(body),
      signal,
    })

    if (!res.ok) {
      if (res.status === 401 || res.status === 403) throw new Error('API Key 无效或已过期')
      if (res.status === 429) throw new Error('请求过于频繁，请稍后再试')
      throw new Error(`API 请求失败 (${res.status})`)
    }

    const reader = res.body?.getReader()
    if (!reader) throw new Error('响应流不可用')

    const decoder = new TextDecoder()
    let fullText = ''
    let buffer = ''
    const toolCalls: ToolCall[] = []

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
          const delta = parsed?.choices?.[0]?.delta
          const finish = parsed?.choices?.[0]?.finish_reason
          if (finish) {
            agentLog.log(`[Agent] API finish_reason: ${finish} | toolCalls累积: ${toolCalls.length}`)
          }
          if (delta?.content) {
            fullText += delta.content
            onChunk(delta.content)
          }
          this.accumulateToolCall(delta, toolCalls)
        } catch {
          // ignore malformed SSE
        }
      }
    }

    agentLog.log(`[Agent] API 流结束 | 文本: ${fullText.length}字 | 工具: ${toolCalls.length}个`)
    return { fullText, toolCalls }
  }

  /** Gemini 非流式（工具检测）+ 流式（输出） */
  private async geminiChat(
    messages: AgentMessage[],
    onChunk: (text: string) => void,
    signal?: AbortSignal
  ): Promise<string> {
    const { apiKey, model } = this.config

    // Gemini 的工具检测需要非流式请求
    const detectUrl = `https://generativelanguage.googleapis.com/v1beta/models/${model}:generateContent?key=${apiKey}`
    const detectBody: Record<string, unknown> = {
      contents: messages
        .filter((m) => m.role !== 'system')
        .map((m) => ({
          role: m.role === 'assistant' ? 'model' : 'user',
          parts: [{ text: m.content }],
        })),
      systemInstruction: { parts: [{ text: this.systemPrompt }] },
    }

    if (this.tools.length > 0 && !messages.some((m) => m.role === 'tool')) {
      detectBody.tools = toGeminiTools(this.tools)
    }

    const detectRes = await fetch(detectUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(detectBody),
      signal,
    })

    if (!detectRes.ok) {
      const text = await detectRes.text().catch(() => '')
      throw new Error(`Gemini API 错误 (${detectRes.status}): ${text.slice(0, 200)}`)
    }

    const data = await detectRes.json()
    const candidate = data?.candidates?.[0]
    const fnCall = candidate?.content?.parts?.[0]?.functionCall

    if (fnCall) {
      // Gemini 需要工具调用
      const tc: ToolCall = {
        id: `fc_${Date.now()}`,
        type: 'function',
        function: {
          name: fnCall.name,
          arguments: JSON.stringify(fnCall.args || {}),
        },
      }

      messages.push({ role: 'assistant', content: '' })
      const result = await this.executeTool(tc)
      messages.push({ role: 'tool', content: result, tool_call_id: tc.id, name: fnCall.name })

      // 流式输出最终回复
      return await this.geminiStream(messages, onChunk, signal)
    }

    // 无工具调用，直接流式输出
    return await this.geminiStream(messages, onChunk, signal)
  }

  /** Gemini 流式输出 */
  private async geminiStream(
    messages: AgentMessage[],
    onChunk: (text: string) => void,
    signal?: AbortSignal
  ): Promise<string> {
    const { apiKey, model } = this.config
    const url = `https://generativelanguage.googleapis.com/v1beta/models/${model}:streamGenerateContent?alt=sse&key=${apiKey}`

    const body = {
      contents: messages
        .filter((m) => m.role !== 'system')
        .map((m) => ({
          role: m.role === 'assistant' ? 'model' : 'user',
          parts: [{ text: m.content }],
        })),
    }

    const res = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
      signal,
    })

    if (!res.ok) {
      const text = await res.text().catch(() => '')
      throw new Error(`Gemini 流式错误 (${res.status}): ${text.slice(0, 200)}`)
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
          const text = parsed?.candidates?.[0]?.content?.parts?.[0]?.text
          if (text) { fullText += text; onChunk(text) }
        } catch { /* ignore */ }
      }
    }

    return fullText
  }

  /**
   * Agent 主入口：发送消息 → 流式思考 → 调用工具 → 回复。
   */
  async chat(
    userText: string,
    history: { role: string; content: string }[],
    callbacks: StreamCallbacks = {},
    signal?: AbortSignal
  ): Promise<string> {
    const model = this.config.model
    const provider = this.config.provider
    agentLog.log(`[Agent] chat 开始 | 模型: ${provider}/${model} | 输入: "${userText.slice(0, 60)}"`)
    agentLog.log(`[Agent] 历史消息: ${history.length}条 | 可用工具: ${this.tools.map(t => t.name).join(', ')}`)

    let messages = this.buildMessages(history, userText)
    const { onChunk, onToolCall, onToolResult, onError } = callbacks
    const maxRounds = 25
    const isGemini = provider === 'gemini'
    const noop = () => {}

    for (let round = 0; round < maxRounds; round++) {
      agentLog.log(`[Agent] ── 推理轮次 ${round + 1}/${maxRounds} ──`)
      agentLog.log(`[Agent] 当前消息数: ${messages.length} (system:1, user/history:${messages.filter(m => m.role !== 'system').length})`)

      try {
        if (isGemini) {
          agentLog.log('[Agent] Gemini 路径')
          return await this.geminiChat(messages, onChunk || noop, signal)
        }

        // OpenAI / DeepSeek：全程流式
        const { fullText, toolCalls } = await this.openaiStream(
          messages, onChunk || noop, signal
        )

        if (toolCalls.length > 0) {
          agentLog.log(`[Agent] 检测到 ${toolCalls.length} 个工具调用`)
          for (const tc of toolCalls) {
            agentLog.log(`[Agent]   工具: ${tc.function.name} | 参数: ${tc.function.arguments.slice(0, 120)}`)
          }

          // 有工具调用 → 加入消息并继续
          messages.push({ role: 'assistant', content: '', tool_calls: toolCalls })

          for (const tc of toolCalls) {
            onToolCall?.(tc.function.name, tc.function.arguments)
            agentLog.log(`[Agent] ⚡ 执行工具: ${tc.function.name}`)
            const result = await this.executeTool(tc)
            agentLog.log(`[Agent] ✓ 工具结果 (${tc.function.name}): ${result.slice(0, 100)}...`)
            onToolResult?.(tc.function.name, result)
            messages.push({ role: 'tool', content: result, tool_call_id: tc.id, name: tc.function.name })
          }
          continue // 下一轮：用工具结果生成回复
        }

        agentLog.log(`[Agent] 无工具调用, 文本长度: ${fullText.length}`)
        // 无工具调用 → 文本已流式输出完毕
        return fullText
      } catch (e: any) {
        agentLog.error(`[Agent] ✗ 错误: ${e.message}`)
        onError?.(e.message || '请求失败')
        throw e
      }
    }

    agentLog.warn('[Agent] 达到最大推理轮次，强制结束')
    return ''
  }
}
