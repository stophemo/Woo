<template>
  <aside class="right-sidebar" :class="[{ collapsed: !isOpen }, { resizing: isResizing }]" :style="{ width: isOpen ? sidebarWidth + 'px' : '0px' }">
    <div class="resize-handle" @mousedown.prevent="startResize"></div>
    <div class="chat-header">
      <select class="model-select" v-model="aiStore.selectedModelId" @change="aiStore.setModel(($event.target as HTMLSelectElement).value)">
        <option v-for="model in aiStore.availableModels" :key="model.id" :value="model.id">
          {{ model.name }}
        </option>
      </select>
      <button class="header-btn" @click="aiStore.clearChat()" title="清空聊天" :disabled="aiStore.messages.length === 0">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="3 6 5 6 21 6"></polyline>
          <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
        </svg>
      </button>
    </div>

    <!-- 知识库开关 -->
    <div class="kb-bar">
      <label class="kb-toggle" title="启用后将搜索文稿内容作为 AI 上下文">
        <input type="checkbox" :checked="aiStore.kbEnabled" @change="aiStore.setKbEnabled(($event.target as HTMLInputElement).checked)" />
        <span class="kb-toggle-label">知识库</span>
      </label>
      <div class="kb-actions">
        <span v-if="aiStore.kbChunkCount > 0" class="kb-status">{{ aiStore.kbDocCount }} 篇 / {{ aiStore.kbChunkCount }} 块</span>
        <span v-if="aiStore.kbEmbedCount > 0 && aiStore.kbEmbedCount < aiStore.kbChunkCount" class="kb-status" style="color:#e53935;">嵌入中...</span>
        <button class="kb-build-btn" @click="handleRebuildKb" :disabled="aiStore.kbBuilding" :title="aiStore.kbBuilding ? '构建中...' : '重建索引'">
          <svg v-if="aiStore.kbBuilding" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="spin">
            <circle cx="12" cy="12" r="10" stroke-dasharray="31.4 10" stroke-linecap="round"/>
          </svg>
          <svg v-else width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0118.8-4.3M22 12.5a10 10 0 01-18.8 4.2"/>
          </svg>
        </button>
      </div>
    </div>

    <div class="chat-messages" ref="messagesContainer">
      <div v-if="aiStore.messages.length === 0" class="chat-empty">
        <h3>有什么我可以帮你的？</h3>
        <div class="quick-buttons">
          <button class="quick-btn" v-for="action in quickActions" :key="action" @click="handleQuickAction(action)">
            {{ action }}
          </button>
        </div>
      </div>

      <ChatMessage v-for="msg in aiStore.messages" :key="msg.id" :message="msg" />

      <!-- 内联确认：紧跟在 AI 回复之后，类似 Claude Code -->
      <div v-if="confirmPending" class="chat-confirm-inline">
        <div class="confirm-prompt">{{ confirmPending.details }}</div>
        <div class="confirm-actions">
          <button class="confirm-btn" @click="resolveConfirmation(false)">取消</button>
          <button class="confirm-btn primary" @click="resolveConfirmation(true)">确认</button>
        </div>
      </div>
    </div>

    <div v-if="aiStore.error" class="chat-error">
      <span>{{ aiStore.error }}</span>
      <button class="error-dismiss" @click="aiStore.error = null">×</button>
    </div>

    <div v-if="!aiStore.hasApiKey" class="api-key-banner">
      <span>请先配置 DeepSeek API Key</span>
      <button class="banner-btn" @click="$emit('open-settings', 'ai')">去设置</button>
    </div>

    <div class="chat-input-area">
      <textarea
        ref="inputRef"
        v-model="inputText"
        class="chat-textarea"
        placeholder="输入消息..."
        rows="1"
        @keydown="handleKeyDown"
        @input="autoResize"
        :disabled="!aiStore.hasApiKey"
      ></textarea>
      <button v-if="aiStore.isStreaming" class="stop-btn" @click="aiStore.cancelGeneration()" title="停止生成">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
          <rect x="6" y="6" width="12" height="12" rx="2"></rect>
        </svg>
      </button>
      <button v-else class="send-btn" @click="handleSend" :disabled="!inputText.trim() || !aiStore.hasApiKey" title="发送">
        <IconSend :size="16" />
      </button>
    </div>

  </aside>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import ChatMessage from './ChatMessage.vue'
import IconSend from '../icons/IconSend.vue'
import { useAiChatStore } from '../../stores/aiChat'
import { useWorkspaceStore } from '../../stores/workspace'
import { pendingConfirmation, resolveConfirmation } from '../../services/agent/confirmation'

interface Props {
  isOpen: boolean
}

const props = defineProps<Props>()
defineEmits<{ 'open-settings': [mode: 'file' | 'ai'] }>()

const aiStore = useAiChatStore()
const workspaceStore = useWorkspaceStore()

const inputText = ref('')
const inputRef = ref<HTMLTextAreaElement | null>(null)
const messagesContainer = ref<HTMLDivElement | null>(null)
const confirmPending = computed(() => pendingConfirmation.value)

const STORAGE_KEY = 'right-sidebar-width'
const MIN_WIDTH = 240
const MAX_WIDTH = window.innerWidth * 0.5

const sidebarWidth = ref(loadWidth())
const isResizing = ref(false)

function loadWidth(): number {
  try {
    const saved = localStorage.getItem(STORAGE_KEY)
    if (saved) {
      const w = parseInt(saved, 10)
      if (w >= MIN_WIDTH && w <= MAX_WIDTH) return w
    }
  } catch {}
  return 320
}

function saveWidth(w: number) {
  try { localStorage.setItem(STORAGE_KEY, String(w)) } catch {}
}

function startResize(e: MouseEvent) {
  e.preventDefault()
  isResizing.value = true
  const startX = e.clientX
  const startWidth = sidebarWidth.value

  function onMove(ev: MouseEvent) {
    const newWidth = startWidth - (ev.clientX - startX)
    const clamped = Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, newWidth))
    sidebarWidth.value = clamped
  }

  function onUp() {
    isResizing.value = false
    document.removeEventListener('mousemove', onMove)
    document.removeEventListener('mouseup', onUp)
    saveWidth(sidebarWidth.value)
  }

  document.addEventListener('mousemove', onMove)
  document.addEventListener('mouseup', onUp)
}

const quickActions = [
  '帮我优化这段文本',
  '帮我梳理写作思路',
  '给我三个标题备选',
  '为这段内容写一个开头',
  '总结当前文档要点',
  '检查语气和逻辑问题'
]

function handleQuickAction(action: string) {
  inputText.value = action
  handleSend()
}

async function handleSend() {
  const text = inputText.value.trim()
  if (!text || !aiStore.hasApiKey) return

  inputText.value = ''
  resetTextareaHeight()

  const docContent = workspaceStore.currentDocument?.content
  await aiStore.sendUserMessage(text, docContent || undefined)
}

function handleKeyDown(e: KeyboardEvent) {
  // Ctrl/Cmd + A: 全选输入框文本
  if ((e.metaKey || e.ctrlKey) && e.key === 'a') {
    // 让浏览器默认行为生效（textarea 全选）
    return
  }
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    handleSend()
  }
}

function autoResize() {
  const el = inputRef.value
  if (!el) return
  el.style.height = 'auto'
  el.style.height = Math.min(el.scrollHeight, 80) + 'px'
}

function resetTextareaHeight() {
  const el = inputRef.value
  if (!el) return
  el.style.height = 'auto'
}

let scrollRafId = 0

function scrollToBottom() {
  if (scrollRafId) return
  scrollRafId = requestAnimationFrame(() => {
    scrollRafId = 0
    const container = messagesContainer.value
    if (!container) return
    const threshold = 60
    const isNearBottom = container.scrollHeight - container.scrollTop - container.clientHeight < threshold
    if (isNearBottom) {
      container.scrollTop = container.scrollHeight
    }
  })
}

async function handleRebuildKb() {
  await aiStore.rebuildKb()
}

// 侧边栏打开时刷新知识库状态
watch(() => props.isOpen, (val) => {
  if (val) aiStore.refreshKbStatus()
})

// 单一声明式滚动 — 消息数或最后一条内容变化时，防抖滚动到底部
watch(
  [() => aiStore.messages.length, () => {
    const msgs = aiStore.messages
    return msgs.length > 0 ? msgs[msgs.length - 1]?.content : ''
  }],
  () => scrollToBottom(),
  { flush: 'post' }
)
</script>

<style scoped>
.right-sidebar {
  background-color: var(--bg-tertiary);
  border-left: 1px solid var(--border-primary);
  display: flex;
  flex-direction: column;
  transition: opacity 0.3s, var(--theme-transition);
  overflow: hidden;
  position: relative;
  flex-shrink: 0;
}

.right-sidebar:not(.resizing) {
  transition: width 0.25s ease, opacity 0.25s ease, var(--theme-transition);
}

.right-sidebar.collapsed {
  opacity: 0;
  overflow: hidden;
}

/* ── 拖拽手柄 ── */

.resize-handle {
  position: absolute;
  top: 0;
  left: 0;
  width: 4px;
  height: 100%;
  cursor: col-resize;
  z-index: 10;
  transition: background-color 0.15s;
}

.resize-handle:hover,
.right-sidebar.resizing .resize-handle {
  background-color: var(--accent);
}

.right-sidebar.collapsed .resize-handle {
  display: none;
}

.chat-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border-secondary);
  flex-shrink: 0;
}

/* ===== 知识库栏 ===== */
.kb-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 12px;
  border-bottom: 1px solid var(--border-secondary);
  flex-shrink: 0;
  font-size: 0.81rem;
  gap: 6px;
}
.kb-toggle {
  display: flex;
  align-items: center;
  gap: 5px;
  cursor: pointer;
  user-select: none;
}
.kb-toggle input { margin: 0; }
.kb-toggle-label {
  color: var(--text-secondary);
  font-weight: 500;
}
.kb-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}
.kb-status {
  color: var(--text-tertiary);
  font-size: 0.75rem;
  white-space: nowrap;
}
.kb-build-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-tertiary);
  cursor: pointer;
  transition: color 0.2s, background 0.2s;
}
.kb-build-btn:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--accent);
}
.kb-build-btn:disabled { opacity: 0.4; cursor: default; }
@keyframes spin { to { transform: rotate(360deg); } }
.spin { animation: spin 1s linear infinite; }

.model-select {
  flex: 1;
  padding: 5px 8px;
  border: 1px solid var(--border-primary);
  border-radius: 4px;
  font-size: 12px;
  background-color: var(--bg-elevated);
  color: var(--text-primary);
  outline: none;
  cursor: pointer;
}

.model-select:focus {
  border-color: var(--accent);
}

.header-btn {
  background: transparent;
  border: none;
  cursor: pointer;
  padding: 5px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  color: var(--text-secondary);
  transition: all 0.2s;
}

.header-btn:hover:not(:disabled) {
  background-color: var(--bg-hover);
  color: var(--text-primary);
}

.header-btn:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}

.chat-messages {
  flex: 1;
  overflow-y: auto;
  padding: 0;
}

.chat-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding-top: 24px;
}

.chat-empty h3 {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0 0 16px 0;
}

.quick-buttons {
  display: flex;
  flex-direction: column;
  gap: 8px;
  width: 100%;
}

.quick-btn {
  padding: 10px 12px;
  background-color: var(--bg-elevated);
  border: 1px solid var(--border-secondary);
  border-radius: 6px;
  cursor: pointer;
  font-size: 13px;
  color: var(--text-primary);
  text-align: left;
  transition: all 0.2s;
}

.quick-btn:hover {
  background-color: var(--bg-hover);
  border-color: var(--border-primary);
}

.chat-error {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  margin: 0 12px;
  background-color: #fef2f2;
  border: 1px solid #fecaca;
  border-radius: 6px;
  font-size: 12px;
  color: #dc2626;
  flex-shrink: 0;
}

:root[data-theme="dark"] .chat-error {
  background-color: #3b1818;
  border-color: #5c2020;
  color: #f87171;
}

.error-dismiss {
  background: none;
  border: none;
  cursor: pointer;
  font-size: 16px;
  color: inherit;
  padding: 0 2px;
  line-height: 1;
}

.api-key-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  margin: 8px 12px 0;
  background-color: var(--editor-highlight-bg);
  border-radius: 6px;
  font-size: 12px;
  color: var(--editor-highlight-text);
  flex-shrink: 0;
}

.banner-btn {
  padding: 3px 10px;
  border: none;
  border-radius: 4px;
  background-color: var(--accent);
  color: #ffffff;
  cursor: pointer;
  font-size: 12px;
  transition: background-color 0.2s;
}

.banner-btn:hover {
  background-color: var(--accent-hover);
}

.chat-input-area {
  display: flex;
  align-items: flex-end;
  gap: 8px;
  padding: 10px 12px;
  border-top: 1px solid var(--border-secondary);
  flex-shrink: 0;
}

.chat-textarea {
  flex: 1;
  padding: 8px 10px;
  border: 1px solid var(--border-primary);
  border-radius: 6px;
  font-size: 13px;
  font-family: inherit;
  background-color: var(--bg-elevated);
  color: var(--text-primary);
  outline: none;
  resize: none;
  line-height: 1.5;
  transition: border-color 0.2s;
}

.chat-textarea:focus {
  border-color: var(--accent);
}

.chat-textarea::placeholder {
  color: var(--text-muted);
}

.chat-textarea:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.send-btn,
.stop-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  flex-shrink: 0;
  transition: all 0.2s;
}

.send-btn {
  background-color: var(--accent);
  color: #ffffff;
}

.send-btn:hover:not(:disabled) {
  background-color: var(--accent-hover);
}

.send-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.stop-btn {
  background-color: #ef4444;
  color: #ffffff;
}

.stop-btn:hover {
  background-color: #dc2626;
}

/* ── 内联确认（在消息流中） ── */

.chat-confirm-inline {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 16px 16px;
}

.confirm-prompt {
  font-size: 13px;
  color: var(--text-primary);
  line-height: 1.4;
  flex: 1;
}

.confirm-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}

.confirm-btn {
  padding: 5px 14px;
  border: none;
  border-radius: 5px;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.confirm-btn.primary {
  background: var(--accent);
  color: #fff;
}
.confirm-btn.primary:hover {
  opacity: 0.9;
}

.confirm-btn:not(.primary) {
  background: var(--bg-hover);
  color: var(--text-primary);
}
.confirm-btn:not(.primary):hover {
  background: var(--bg-active);
}
</style>
