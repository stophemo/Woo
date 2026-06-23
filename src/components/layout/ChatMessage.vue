<template>
  <div class="chat-message" :class="message.role">
    <div class="msg-body" @click="handleBubbleClick">
      <!-- 思考步骤 -->
      <div v-if="message.role === 'assistant' && message.thinkingSteps?.length" class="thinking-steps">
        <div
          v-for="step in message.thinkingSteps"
          :key="step.id"
          class="thinking-step"
          :class="[`type-${step.type}`, `status-${step.status}`]"
        >
          <span class="step-icon">
            <template v-if="step.type === 'tool_call' && step.status === 'running'">
              <svg class="spin" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                <circle cx="12" cy="12" r="10" stroke-dasharray="31.4 10" stroke-linecap="round"/>
              </svg>
            </template>
            <template v-else-if="step.status === 'done'">✓</template>
            <template v-else-if="step.status === 'error'">✕</template>
            <template v-else>○</template>
          </span>
          <span class="step-label">{{ step.label }}</span>
        </div>
      </div>

      <!-- 消息内容 -->
      <div v-if="rendered" class="message-content" v-html="rendered"></div>
      <div v-if="message.isStreaming && !message.content && (!message.thinkingSteps || message.thinkingSteps.length === 0)" class="streaming-placeholder">思考中<span class="thinking-dots"><span>.</span><span>.</span><span>.</span></span></div>
    </div>

    <!-- 操作按钮 -->
    <div v-if="message.content && !message.isStreaming" class="msg-actions">
      <button class="action-btn" data-msg-copy="" title="复制">
        <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
        </svg>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onUnmounted } from 'vue'
import { marked } from 'marked'
import type { ChatMessage } from '../../types/ai'

marked.setOptions({ breaks: true, gfm: true })

interface Props { message: ChatMessage }
const props = defineProps<Props>()

const rendered = ref('')
let renderTimer: ReturnType<typeof setTimeout> | null = null

function injectCopyButtons(html: string): string {
  const div = document.createElement('div')
  div.innerHTML = html
  for (const pre of div.querySelectorAll('pre')) {
    const codeEl = pre.querySelector('code')
    if (!codeEl) continue
    const codeText = codeEl.textContent || ''
    const encoded = btoa(unescape(encodeURIComponent(codeText)))
    const btn = document.createElement('button')
    btn.className = 'code-copy-btn'
    btn.setAttribute('data-code', encoded)
    btn.title = '复制代码'
    btn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>'
    pre.style.position = 'relative'
    pre.appendChild(btn)
  }
  return div.innerHTML
}

function renderContent() {
  if (!props.message.content) {
    rendered.value = ''
    return
  }
  if (props.message.role === 'user') {
    rendered.value = props.message.content
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/\n/g, '<br>')
    return
  }
  const raw = marked.parse(props.message.content) as string
  rendered.value = injectCopyButtons(raw)
}

function handleBubbleClick(e: MouseEvent) {
  const target = e.target as HTMLElement

  if (target.classList.contains('code-copy-btn') || target.closest('.code-copy-btn')) {
    e.stopPropagation()
    const btn = target.classList.contains('code-copy-btn')
      ? target
      : target.closest('.code-copy-btn') as HTMLElement
    const encoded = btn.getAttribute('data-code')
    if (!encoded) return
    try {
      const text = decodeURIComponent(escape(atob(encoded)))
      navigator.clipboard.writeText(text).then(() => {
        btn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>'
        setTimeout(() => {
          btn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>'
        }, 1500)
      })
    } catch {}
    return
  }

  if (target.hasAttribute('data-msg-copy') || target.closest('[data-msg-copy]')) {
    e.stopPropagation()
    // 复制渲染后的可见文本（非 Markdown 源码）
    const bubble = target.closest('.msg-body')
    const renderedEl = bubble?.querySelector('.message-content')
    const text = renderedEl?.textContent || props.message.content
    if (!text) return
    navigator.clipboard.writeText(text).then(() => {
      const btn = target.hasAttribute('data-msg-copy')
        ? target
        : target.closest('[data-msg-copy]') as HTMLElement
      const original = btn.innerHTML
      btn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>'
      setTimeout(() => { btn.innerHTML = original }, 1500)
    })
    return
  }
}

watch(
  () => props.message.content,
  () => {
    if (props.message.isStreaming) {
      if (!renderTimer) {
        renderTimer = setTimeout(() => {
          renderTimer = null
          renderContent()
        }, 50)
      }
    } else {
      if (renderTimer) { clearTimeout(renderTimer); renderTimer = null }
      renderContent()
    }
  },
  { immediate: true }
)

watch(
  () => props.message.isStreaming,
  (streaming) => {
    if (!streaming) {
      if (renderTimer) { clearTimeout(renderTimer); renderTimer = null }
      renderContent()
    }
  }
)

onUnmounted(() => { if (renderTimer) clearTimeout(renderTimer) })
</script>

<style scoped>
/* ══════════════════════════════════
   消息行 — 仿 DeepSeek Chat 样式
   ══════════════════════════════════ */
.chat-message {
  display: flex;
  flex-direction: column;
  padding: 12px 16px;
  position: relative;
}

.chat-message + .chat-message {
  border-top: 1px solid var(--border-secondary);
}

/* ── 消息体 ── */
.msg-body {
  width: 100%;
  font-size: 14px;
  line-height: 1.7;
  word-wrap: break-word;
  overflow-wrap: break-word;
  color: var(--text-primary);
}

/* 己方消息 — 蓝色气泡背景 */
.user .msg-body {
  background: var(--accent-light);
  border-radius: 8px;
  padding: 8px 12px;
  max-width: 85%;
  align-self: flex-end;
}

/* DeepSeek: 己方消息右对齐 */
.chat-message.user {
  align-items: flex-end;
}

.chat-message.assistant {
  align-items: flex-start;
}

/* ── 操作按钮 ── */
.msg-actions {
  position: absolute;
  right: 16px;
  bottom: -8px;
  opacity: 0;
  transition: opacity 0.15s;
  z-index: 1;
}

.chat-message:hover .msg-actions {
  opacity: 1;
}

@media (hover: none) and (pointer: coarse) {
  .msg-actions {
    opacity: 1;
  }
}

.action-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  padding: 0;
  border: 1px solid var(--border-primary);
  border-radius: 6px;
  background: var(--bg-elevated);
  color: var(--text-muted);
  cursor: pointer;
  transition: background 0.15s, color 0.15s, border-color 0.15s;
  box-shadow: var(--shadow-card);
}

.action-btn:hover {
  background: var(--accent);
  color: #fff;
  border-color: var(--accent);
}

/* ══════════════════════════════════
   Markdown 渲染
   ══════════════════════════════════ */
.message-content {
  font-size: 14px;
  line-height: 1.7;
}

.message-content :deep(p) {
  margin: 6px 0;
}
.message-content :deep(p:first-child) {
  margin-top: 0;
}
.message-content :deep(p:last-child) {
  margin-bottom: 0;
}

.message-content :deep(h1),
.message-content :deep(h2),
.message-content :deep(h3),
.message-content :deep(h4) {
  margin: 14px 0 6px;
  font-weight: 600;
  color: var(--text-primary);
}
.message-content :deep(h1) { font-size: 18px; }
.message-content :deep(h2) { font-size: 16px; }
.message-content :deep(h3) { font-size: 15px; }

.message-content :deep(ul),
.message-content :deep(ol) {
  margin: 4px 0;
  padding-left: 20px;
}
.message-content :deep(li) { margin: 2px 0; }

.message-content :deep(blockquote) {
  margin: 8px 0;
  padding: 4px 14px;
  border-left: 3px solid var(--accent);
  color: var(--text-secondary);
}

.message-content :deep(hr) {
  margin: 12px 0;
  border: none;
  border-top: 1px solid var(--border-secondary);
}

.message-content :deep(a) {
  color: var(--accent);
  text-decoration: none;
}
.message-content :deep(a:hover) { text-decoration: underline; }

.message-content :deep(code) {
  background: var(--bg-hover);
  padding: 1px 5px;
  border-radius: 3px;
  font-size: 13px;
  font-family: 'SF Mono', 'Fira Code', 'Consolas', monospace;
  color: var(--text-primary);
}

.message-content :deep(pre) {
  margin: 8px 0;
  padding: 12px 14px;
  background: var(--editor-pre-bg);
  border: 1px solid var(--border-secondary);
  border-radius: 8px;
  overflow-x: auto;
  position: relative;
}

.message-content :deep(pre code) {
  background: none;
  padding: 0;
  font-size: 13px;
  line-height: 1.6;
  color: var(--text-primary);
  font-family: 'SF Mono', 'Fira Code', 'Consolas', monospace;
}

.message-content :deep(table) {
  border-collapse: collapse;
  margin: 8px 0;
  font-size: 13px;
}
.message-content :deep(th),
.message-content :deep(td) {
  padding: 6px 10px;
  border: 1px solid var(--border-primary);
  text-align: left;
}
.message-content :deep(th) {
  background: var(--bg-hover);
  font-weight: 600;
}

.message-content :deep(strong) {
  font-weight: 600;
  color: var(--text-primary);
}

/* ── 代码块复制按钮 ── */
:deep(.code-copy-btn) {
  position: absolute;
  top: 6px;
  right: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  padding: 0;
  border: 1px solid var(--border-primary);
  border-radius: 5px;
  background: var(--bg-elevated);
  color: var(--text-muted);
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.15s, background 0.15s, color 0.15s, border-color 0.15s;
}

:deep(pre:hover .code-copy-btn) {
  opacity: 1;
}

:deep(.code-copy-btn:hover) {
  background: var(--accent);
  color: #fff;
  border-color: var(--accent);
}

/* ══════════════════════════════════
   思考步骤
   ══════════════════════════════════ */
.thinking-steps {
  margin-bottom: 6px;
  padding-bottom: 6px;
  border-bottom: 1px solid var(--border-secondary);
}

.thinking-step {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 2px 0;
  font-size: 13px;
  line-height: 1.4;
}

.thinking-step.type-tool_call.status-running { color: var(--text-muted); }
.thinking-step.type-tool_call.status-done { color: var(--text-tertiary); }
.thinking-step.type-tool_result.status-done {
  color: var(--text-tertiary);
  font-size: 12px;
  padding-left: 20px;
}
.thinking-step.type-tool_call.status-error { color: #e53935; }

.step-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  flex-shrink: 0;
  font-size: 11px;
  font-weight: 700;
}
.thinking-step.status-done .step-icon { color: #43a047; }
.thinking-step.status-error .step-icon { color: #e53935; }

.step-label {
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

@keyframes spin { to { transform: rotate(360deg); } }
.spin { animation: spin 1s linear infinite; }

/* ══════════════════════════════════
   流式指示
   ══════════════════════════════════ */
.streaming-cursor {
  display: inline-block;
  animation: blink 1s step-end infinite;
  font-size: 14px;
  line-height: 1;
  vertical-align: text-bottom;
  color: var(--text-muted);
}

@keyframes blink { 0%, 50% { opacity: 1; } 51%, 100% { opacity: 0; } }

.streaming-placeholder {
  font-size: 14px;
  color: var(--text-tertiary);
}

.thinking-dots span {
  animation: dotPulse 1.4s infinite;
  opacity: 0;
}
.thinking-dots span:nth-child(1) { animation-delay: 0s; }
.thinking-dots span:nth-child(2) { animation-delay: 0.2s; }
.thinking-dots span:nth-child(3) { animation-delay: 0.4s; }

@keyframes dotPulse {
  0%, 60%, 100% { opacity: 0; }
  30% { opacity: 1; }
}
</style>
