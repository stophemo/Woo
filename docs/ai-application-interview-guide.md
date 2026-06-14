# AI 应用开发：技术点深度解析

> 基于 Woo 项目的真实代码，梳理 AI 应用开发工程师需要掌握的核心技术点，
> 以及面试中可能被问到的设计问题与思路。

---

## 目录

1. [Function Calling / Tool Use](#1-function-calling--tool-use)
2. [流式 SSE 解析](#2-流式-sse-解析)
3. [RAG 检索增强生成](#3-rag-检索增强生成)
4. [多供应商抽象层](#4-多供应商抽象层)
5. [系统提示词工程](#5-系统提示词工程)
6. [LLM API 错误处理](#6-llm-api-错误处理)
7. [流式写入与编辑器集成](#7-流式写入与编辑器集成)
8. [AI 操作的安全模型](#8-ai-操作的安全模型)
9. [状态管理与并发控制](#9-状态管理与并发控制)
10. [AI 应用的架构决策](#10-ai-应用的架构决策)

---

## 1. Function Calling / Tool Use

### 面试中常问

> "请设计一个能让 LLM 调用外部工具的架构。"

### Woo 的实现

Woo 实现了一个**多轮 function calling 循环**（`agent/index.ts`），核心逻辑约 150 行：

```typescript
// 伪代码 — 多轮工具调用循环
for (let round = 0; round < maxRounds; round++) {
  // 1. 发流式请求（附带所有工具定义）
  const { fullText, toolCalls } = await openaiStream(messages, tools)

  // 2. 检测是否有工具调用
  if (toolCalls.length === 0) return fullText  // 完成

  // 3. 逐条执行工具，结果加入消息列表
  for (const tc of toolCalls) {
    const result = await executeTool(tc)
    messages.push({ role: 'tool', content: result })
  }
  // 4. 继续下一轮推理
}
```

### 关键设计点

**1. 为什么是多轮而不是一轮执行完所有工具？**

模型看到第一个工具的结果后，可能需要据此决定下一步动作。例如：先搜索文档 → 发现文档不存在 → 决定创建 → 输出结果。这需要多轮推理，时序很重要。

面试中可以展开：什么时候需要多轮？什么时候一轮就够？一轮场景如"提取这篇文档中的三个关键词，然后搜索知识库"——模型可以一次发出两个独立工具调用。

**2. 工具调用的流式累积**

OpenAI / DeepSeek 的 tool_calls 是流式到达的，参数被拆成多个 delta chunk：

```typescript
// delta.tool_calls[0].function.arguments = "{\"query\": \"项目"
// delta.tool_calls[0].function.arguments += "推进"
// delta.tool_calls[0].function.arguments += "\"}"
```

Woo 的 `accumulateToolCall()` 函数处理两种标识方式：
- `id` + `function.name` 均在第一个 delta 中完整给出 → 用 id 查找合并
- 部分 API 用 `index` 而非 `id` → 用数组索引引用

这个细节面试常见："流式 function calling 的参数是如何到达的？"

**3. 工具定义格式**

采用 JSON Schema 描述参数，兼容 OpenAI / MCP 标准：

```typescript
const searchKbTool: Tool = {
  name: 'search_kb',
  description: '语义搜索知识库，根据含义查找相关文档片段',
  parameters: {
    type: 'object',
    properties: {
      query: { type: 'string', description: '搜索关键词...' },
      limit: { type: 'number', description: '返回结果数量，默认 5' }
    },
    required: ['query']
  },
  handler: async (args) => { /* 执行并返回字符串 */ }
}
```

**面试点**：为什么 `handler` 返回 `string` 而不是结构化数据？因为工具结果是给**模型**看的，不是给代码的。模型需要自然语言描述的上下文来决定下一步。

**4. 适配器模式解耦**

`tools.ts` 不直接调用 API 或操作数据库，而是通过 `ContextProvider` 接口：

```typescript
// tools.ts — 纯工具定义，不依赖具体实现
let ctx: ContextProvider | null = null

// aiChat.ts — 提供具体实现
agent.setContextProvider({
  searchKb: async (query) => { /* IPC 调用主进程 */ },
  updateNote: async (id, content) => { /* 操作 Pinia store */ },
  // ...
})
```

这使工具定义可测试、可替换。面试中可以展开："如果我想把工具定义暴露为 REST API 供外部调用，需要改什么？"——只需要替换 ContextProvider 实现。

---

## 2. 流式 SSE 解析

### 面试中常问

> "流式 API 的 SSE 数据是如何解析的？有什么坑？"

### Woo 的实现

```typescript
const reader = res.body.getReader()
const decoder = new TextDecoder()
let fullText = ''
let buffer = ''

while (true) {
  const { done, value } = await reader.read()
  if (done) break

  buffer += decoder.decode(value, { stream: true })
  const lines = buffer.split('\n')
  buffer = lines.pop() || ''  // 保存不完整的最后一行

  for (const rawLine of lines) {
    const line = rawLine.trim()
    if (!line.startsWith('data: ')) continue
    const data = line.slice(6).trim()
    if (!data || data === '[DONE]') continue

    const parsed = JSON.parse(data)
    const chunk = parsed?.choices?.[0]?.delta?.content
    if (chunk) onChunk(chunk)
  }
}
```

### 关键设计点

**1. Buffer 管理**

`decoder.decode(value, { stream: true })` 和 `buffer` 是必须的。因为 SSE 数据帧可能在任意位置被 TCP 分段：

```
收到帧1: "data: {\"choices\":[{\"delta\":{\"content\":"
收到帧2: "\"你好"
收到帧3: "\"}}]}\n\ndata: [DONE]\n\n"
```

没有 buffer 的话，第一帧的 JSON 是不完整的，`JSON.parse` 会抛异常。`buffer` 累积不完整的行，等下一帧补全后再处理。

**面试点**：`decoder.decode(value, { stream: true })` 中的 `stream: true` 参数有什么用？—— 它告诉 TextDecoder 当前 chunk 可能不是完整的 UTF-8 序列末尾，不要因为不完整而报错，等下一次 decode 再处理。

**2. [DONE] 信号**

OpenAI 兼容 API 在流结束时发送 `data: [DONE]`，这需要显式跳过 `JSON.parse`。不同供应商可能有不同的结束信号格式。

**3. Gemini 的 SSE 差异**

Gemini 的流式 API 响应格式不同，数据体结构也不同：
- OpenAI: `{ choices: [{ delta: { content: "..." } }] }`
- Gemini: `{ candidates: [{ content: { parts: [{ text: "..." }] } }] }`

且 Gemini 不支持流式 function calling，必须先用非流式请求检测工具调用。

---

## 3. RAG 检索增强生成

### 面试中常问

> "请设计一个纯本地的 RAG 系统。"

### Woo 的实现

分三步：

**第一步：文档分块**（`kbService.cjs`）

```typescript
function chunkText(text, title) {
  const paragraphs = text.split(/\n\s*\n/).filter(Boolean)
  // 合并段落，每块 150~350 字
  // 最后一块 < 100 字则合并到前一块
}
```

设计考量：
- 按 `\n\n` 切分，保留段落语义边界
- 块大小 150~350 字：太小则语义不完整，太大则噪声多
- 小尾巴处理：防止大量单行块

**面试点**：分块策略有哪些选择？(固定 token 数 / 语义分块 / 递归分块) 各有什么利弊？

**第二步：嵌入**

```typescript
// 使用 bge-small-zh-v1.5 (512维，中文优化)
const input = `为这个句子生成表示以用于检索相关文章：${text}`
const result = await embedder(input, { pooling: 'mean', normalize: true })
return result.data  // Float32Array
```

关键细节：bge 模型需要添加**查询前缀**来获得更好的检索效果。不加前缀的嵌入质量会显著下降。这个细节常被忽略。

**第三步：向量搜索**

```sql
-- sqlite-vec 的 KNN 搜索
SELECT v.id, v.distance
FROM kb_vectors v
WHERE v.embedding MATCH ?
AND v.k = ?
ORDER BY v.distance
```

`sqlite-vec` 是一个 SQLite 扩展，用虚拟表 `vec0` 实现近似最近邻搜索。相当于把 Pinecone 的功能嵌入到 SQLite 中，零外部依赖。

### 与其他方案的对比

| 方案 | 优势 | 劣势 |
|------|------|------|
| **sqlite-vec** (本方案) | 零外部依赖，离线可用 | 仅进程内搜索，不支持分布式 |
| Pinecone / Milvus | 高性能分布式 | 需要外部服务，不适合本地应用 |
| 内存向量索引 (如 hnswlib) | 速度快 | 重启丢失，不适合持久化 |

---

## 4. 多供应商抽象层

### 面试中常问

> "如何设计 AI 应用的供应商切换机制？"

### Woo 的实现

```
             ┌──────────────┐
             │  Agent 引擎   │
             │  (统一入口)    │
             └──────┬───────┘
                    │
          ┌─────────┴──────────┐
          ▼                    ▼
   OpenAI 兼容路径        Gemini 路径
   (streaming +           (非流式检测
    tool_calls in delta)    + 流式输出)
          │                    │
          ▼                    ▼
   deepseek.ts             gemini.ts
   sendMessage()           sendGeminiMessage()
```

**核心差异处理**：

Gemini 的流式 API 不支持 function calling，所以实现不同：

```typescript
// OpenAI 路径：一步到位，流式响应中可能包含 tool_calls
const { fullText, toolCalls } = await openaiStream(messages, tools);

// Gemini 路径：先非流式检测工具，再流式输出
const fnCall = await detectToolCall(messages, tools);
if (fnCall) {
  await executeTool(fnCall);
  return await geminiStream(messages); // 输出最终回复
}
return await geminiStream(messages);
```

**面试点**：这个差异说明了什么？——不同供应商的 API 能力不对称。设计抽象层时需要为**能力差异**（而非仅仅是 URL 差异）做准备。一个统一的 `stream()` 接口可能不够。

**配置管理**：每个供应商的 API Key 分开存储：

```typescript
// localStorage 存储结构
{
  "provider": "deepseek",
  "deepseekApiKey": "sk-xxx",
  "deepseekBaseUrl": "https://api.deepseek.com",
  "geminiApiKey": "AIza...",
  "openaiBaseUrl": "http://localhost:11434",
  // ...
}
```

---

## 5. 系统提示词工程

### Woo 的提示词设计

Woo 的系统提示词（`agent/index.ts` 中 `DEFAULT_SYSTEM_PROMPT`）包含 5 个层次：

**1. 身份与能力声明**
```
你是 Woo 无我笔记的 AI 助手，专注于帮助用户写作和整理知识。
```

**2. 行为约束**
```
- 直接给出答案，不要加"让我看看""我先查一下"等前缀
- 工具调用是后台自动执行的，用户看不到。不要告诉用户你在做什么——只给出结果
```

这点很关键——模型默认会详细解释它的思考过程（"我先搜索知识库，找到了相关文档..."），这在笔记助手中显得啰嗦。

**3. 工具使用规则**
```
- 能不查就不查：闲聊、问工具本身怎么用等，直接回答
- 每次工具调用的 query 必须从当前问题提取新关键词，不要复用旧词
- 创建笔记前先 list_folders 了解目录结构
```

第 2 条解决了一个常见问题：模型连续两次调用同一个工具的 query 可能完全相同，导致搜索结果重复。第 3 条防止模型假设目录存在。

**4. 输出格式**
```
- 写入笔记的内容使用 Markdown 格式
- 系统会自动将 Markdown 转为编辑器兼容的 HTML
```

**5. 格式指令**
```
- 回答简洁，中文优先
- 不确定时坦诚说"不知道"
```

### 面试点

> "你怎么确保模型遵守提示词中的约束？"

提示词不是万能的。实践中还需要：
- **工具定义中的 `description` 要精确**：模型更倾向于遵循它认为"需要"的工具，而不是被提示词禁止的行为
- **失败处理**：当模型不遵守时，Woo 的处理方式是默认信任模型判断，不对输出做后处理裁剪。这是一种权衡——保持输出自然，代价是偶尔的违规
- **迭代优化**：提示词是通过反复实验调整的。例如 "不要告诉用户你在做什么" 这条就是观察到模型在每次工具调用后都会输出"让我查一下..."才加上的

---

## 6. LLM API 错误处理

### Woo 的错误处理体系

```typescript
// agent/index.ts — API 调用
if (!res.ok) {
  if (res.status === 401 || res.status === 403)
    throw new Error('API Key 无效或已过期')
  if (res.status === 429)
    throw new Error('请求过于频繁，请稍后再试')
  throw new Error(`API 请求失败 (${res.status})`)
}

// deepseek.ts — 网络级错误
try {
  res = await fetch(url, { ... })
} catch (err) {
  if (err.name === 'TypeError' && err.message === 'Failed to fetch')
    throw new Error('无法连接到 API 服务，请检查网络连接和 Base URL 配置')
  throw err
}
```

### 常见的 LLM API 错误类型

| 错误 | 状态码 | 处理策略 |
|------|--------|----------|
| API Key 无效/过期 | 401/403 | 引导用户重新配置，设置中测试连接 |
| 速率限制 | 429 | 建议稍后重试 |
| 模型不可用 | 404 | 检查模型名称 |
| 上下文过长 | 400 (context length) | 需要实现上下文窗口管理 |
| 网络错误 | N/A (fetch 异常) | 检查网络和 Base URL |

**面试点**：流式请求中的错误处理有什么不同？—— 非流式请求的错误在 `await fetch()` 之后立即拿到；流式请求中，连接建立后可能在某次 `reader.read()` 时断流，这需要在读取循环中处理异常。

### 取消机制

```typescript
const controller = new AbortController()
abortController.value = controller

// 用户点击停止
function cancelGeneration() {
  controller.abort()
  clearConfirmation()  // 同时清除待处理的确认
}

// fetch 时传入 signal
const res = await fetch(url, { signal })
```

`AbortController` 是一个标准的 Web API，在 AI 应用中非常重要——用户可能在 AI 输出过程中改变主意，需要能立即中断请求。

---

## 7. 流式写入与编辑器集成

### Woo 的流式写入

当 AI 执行 `update_note` 时，不是一次性写入完成，而是**模拟打字效果**逐段写入：

```typescript
// 每批写 3~5 字符，总耗时至少 1.5 秒
const chunkSize = totalLen > 800 ? 5 : 3
const minDuration = 1500
const delayMs = Math.max(25, Math.min(80, Math.ceil(minDuration / totalChunks)))

ws.isExternalStreaming = true
while (pos < html.length) {
  ws.updateContentStream(noteId, html.slice(0, pos))

  // 注意：避免卡在 HTML 标签中间
  // 如果当前片段含 < 但不含 >，继续往后读到 >
  while (frag.includes('<') && !frag.includes('>')) { ... }

  await new Promise(r => setTimeout(r, delayMs))
}
```

### 面试点

**1. 为什么不在主进程做流式写入？**

因为编辑器在渲染进程，更新编辑器内容需要：
- 直接操作 Pinia store → 触发 Vue reactive 更新
- 通过 IPC 跨进程传每次的增量？开销太大，时延不可控
- 所以 Agent 在渲染进程运行，这是经过权衡后的架构决策

**2. Vue 响应式技巧**

```typescript
// 每次创建新对象引用，触发 watch
currentDocumentData.value = { ...currentDocumentData.value, content }
```

如果**原地修改** `currentDocumentData.value.content = newContent`，Vue 无法检测到嵌套对象的变化（除非用 `reactive` + 深层 watch）。通过创建新引用赋值，确保 `watch` 回调被触发。

但更新编辑器的内容时（`updateDocumentContent`），使用的是**原地修改**，因为编辑器需要引用不变来保持用户输入不被打断。流式写入和用户编辑用了不同的更新策略——这也是一个设计细节。

---

## 8. AI 操作的安全模型

### 面试中常问

> "AI Agent 执行破坏性操作时如何确保安全？"

### Promise 暂停模式

Woo 用一种简洁的模式实现：工具 handler 的 `await` 自然暂停了 Agent 循环，不阻塞主线程。

```typescript
// tools.ts — 破坏性操作 handler
handler: async (args) => {
  const confirmed = await requestConfirmation(
    'delete_note',
    '将删除笔记"会议记录"'
  )
  if (!confirmed) return '操作已取消'
  // 继续执行...
}

// confirmation.ts
export function requestConfirmation(details: string): Promise<boolean> {
  return new Promise(resolve => {
    pendingConfirmation.value = { details, resolve }
    // ↑ 这行让 Vue 模板渲染出确认按钮
    // 用户点击 → resolveConfirmation(true/false) → Promise 被 resolve
  })
}
```

### 细节考量

**为什么不用 confirm() 弹窗？**
- `window.confirm()` 会阻塞整个渲染进程，包括正在流式输出的 UI
- 自定义内联确认框让用户可以同时看到 AI 输出+确认请求

**取消时安全**：用户点击停止按钮时，`cancelGeneration()` 调用 `clearConfirmation()` 自动拒绝所有待处理的确认，避免"生成了一个确认框没人处理"的悬挂问题。

---

## 9. 状态管理与并发控制

### Woo 的 AI 状态管理

`aiChat` store（`stores/aiChat.ts`）管理：

1. **消息列表**：支持流式状态标记
```typescript
interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  isStreaming?: boolean     // 当前正在流式输出
  thinkingSteps?: ThinkingStep[]  // 工具调用步骤
}
```

2. **思维链追踪**
```typescript
interface ThinkingStep {
  id: string
  type: 'tool_call' | 'tool_result'
  label: string
  status: 'running' | 'done' | 'error'
}
```

3. **并发保护**：通过 `AbortController` 确保同一时间只有一个请求进行中

4. **持久化**：消息列表存到 `localStorage`，刷新页面后恢复

### 面试点

> "AI 聊天消息列表持续增长，如何处理上下文窗口限制？"

Woo 目前没有实现上下文窗口管理。在 v0.4.8 阶段，每次请求发送全部历史消息。生产级方案需要：
- **滑动窗口**：截取最近 N 轮对话 + 摘要历史
- **摘要压缩**：当历史超过阈值时，用 LLM 压缩早期对话
- **token 计数**：精确计算 token 用量，在接近限制前截断

---

## 10. AI 应用的架构决策

### Woo 的关键架构选择

| 决策 | Woo 的选择 | 另一种选择 | 为什么这么选 |
|------|-----------|-----------|-------------|
| Agent 运行位置 | **渲染进程** | 主进程 / 独立 worker | 可直接操作 Pinia store 实现流式写入，减少 IPC 开销 |
| 工具执行 | **渲染进程通过 IPC** | 全部在主进程 | 边界清晰：工具定义在渲染进程，数据操作委托给主进程 |
| 向量数据库 | **sqlite-vec** | Pinecone / Milvus | 本地优先，零外部依赖 |
| 嵌入模型 | **本地 BGE** | OpenAI Embeddings API | 离线可用，隐私保护 |
| 编辑器集成 | **逐字流式写入** | 一次性写入 | 用户可见的"打字效果"，体验更好 |
| 错误处理 | **用户可见的错误消息** | 静默重试 | 透明沟通，用户可据此调整配置 |

### 面试价值

这些决策对面试的价值在于能展示你的**权衡意识**——不是单纯的技术选型，而是理解每个选择背后的 tradeoff。

例如 Agent 在渲染进程运行：

**优势**：
- 直接访问 Pinia store，流式写入零延迟
- SSE 连接不会经过 IPC 序列化/反序列化
- 工具执行的确认 UI 在同一进程，响应即时

**代价**：
- 渲染进程的 JavaScript 主线程承担更多负担
- 大模型请求涉及大量 JSON 解析，可能影响 UI 渲染性能
- 如果渲染进程崩溃，Agent 状态丢失

> 面试官想知道的是：你是否理解这些 tradeoff，并为你的选择提供了合理的理由。

---

## 总结：AI 应用开发工程师需要掌握什么

基于 Woo 项目的实际代码，以下是核心能力清单：

| 能力 | 对应章节 | 面试常见问题 |
|------|---------|-------------|
| Tool Use 架构设计 | §1 | 设计一个 function calling 系统；流式参数的累积方式 |
| SSE 流式处理 | §2 | SSE 协议的坑；缓冲区管理 |
| RAG 管线 | §3 | 分块策略对比；向量搜索的选择 |
| 抽象层设计 | §4 | 如何兼容多个 LLM 供应商 |
| 提示词工程 | §5 | 如何设计有效的 system prompt |
| 错误处理与容错 | §6 | LLM API 的常见错误及处理策略 |
| 实时 UI 集成 | §7 | 流式输出如何驱动前端更新 |
| 安全模型 | §8 | AI 操作的确认机制设计 |
| 状态管理 | §9 | 聊天状态管理；上下文窗口控制 |
| 架构权衡 | §10 | Agent 运行在哪；IPC 边界在哪 |

---

*本文档基于 Woo 项目 v0.4.8 的源代码，建议对照 `app-desktop/src/services/agent/`、`app-desktop/src/stores/aiChat.ts`、`app-desktop/electron/services/kbService.cjs` 阅读。*
