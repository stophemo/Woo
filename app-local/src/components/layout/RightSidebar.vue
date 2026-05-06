<template>
  <aside class="right-sidebar" :class="{ 'collapsed': !isOpen }">
    <!-- 头部栏：模型选择 + 清空 -->
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

    <!-- 消息区域 -->
    <div class="chat-messages" ref="messagesContainer">
      <!-- 空状态：欢迎 + 快捷操作 -->
      <div v-if="aiStore.messages.length === 0" class="chat-empty">
        <h3>有什么我可以帮你的？</h3>
        <div class="quick-buttons">
          <button class="quick-btn" v-for="action in quickActions" :key="action" @click="handleQuickAction(action)">
            {{ action }} →
          </button>
        </div>
      </div>

      <!-- 消息列表 -->
      <ChatMessage
        v-for="msg in aiStore.messages"
        :key="msg.id"
        :message="msg"
      />
    </div>

    <!-- 错误提示 -->
    <div v-if="aiStore.error" class="chat-error">
      <span>{{ aiStore.error }}</span>
      <button class="error-dismiss" @click="aiStore.error = null">×</button>
    </div>

    <!-- API Key 未配置提示 -->
    <div v-if="!aiStore.hasApiKey" class="api-key-banner">
      <span>请先配置 API Key</span>
      <button class="banner-btn" @click="$emit('open-settings')">去设置</button>
    </div>

    <!-- 输入区域 -->
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
      <button
        v-if="aiStore.isStreaming"
        class="stop-btn"
        @click="aiStore.cancelGeneration()"
        title="停止生成"
      >
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
          <rect x="6" y="6" width="12" height="12" rx="2"></rect>
        </svg>
      </button>
      <button
        v-else
        class="send-btn"
        @click="handleSend"
        :disabled="!inputText.trim() || !aiStore.hasApiKey"
        title="发送"
      >
        <IconSend :size="16" />
      </button>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { ref, nextTick, watch } from 'vue'
import ChatMessage from './ChatMessage.vue'
import IconSend from '../icons/IconSend.vue'
import { useAiChatStore } from '../../stores/aiChat'
import { useWorkspaceStore } from '../../stores/workspace'

interface Props {
  isOpen: boolean
}

defineProps<Props>()
defineEmits<{ 'open-settings': [] }>()

const aiStore = useAiChatStore()
const workspaceStore = useWorkspaceStore()

const inputText = ref('')
const inputRef = ref<HTMLTextAreaElement | null>(null)
const messagesContainer = ref<HTMLDivElement | null>(null)

const quickActions = [
  '试试 AI 写作助手',
  '能为我推荐一些写作灵感吗？',
  '帮我梳理一下写作思路',
  '能帮我生成一段开头吗？',
  '帮我润色这段文字',
  '帮我总结这篇文章'
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

function scrollToBottom() {
  const container = messagesContainer.value
  if (!container) return
  const threshold = 60
  const isNearBottom = container.scrollHeight - container.scrollTop - container.clientHeight < threshold
  if (isNearBottom) {
    nextTick(() => {
      container.scrollTop = container.scrollHeight
    })
  }
}

// 新消息时强制滚到底部
watch(
  () => aiStore.messages.length,
  () => {
    nextTick(() => {
      const container = messagesContainer.value
      if (container) {
        container.scrollTop = container.scrollHeight
      }
    })
  }
)

// 流式内容更新时智能滚动
watch(
  () => {
    const msgs = aiStore.messages
    if (msgs.length === 0) return ''
    return msgs[msgs.length - 1]?.content
  },
  () => scrollToBottom()
)
</script>

<style scoped>
.right-sidebar {
  width: 320px;
  background-color: var(--bg-tertiary);
  border-left: 1px solid var(--border-primary);
  display: flex;
  flex-direction: column;
  transition: width 0.3s, opacity 0.3s, var(--theme-transition);
  overflow: hidden;
}

.right-sidebar.collapsed {
  width: 0;
  opacity: 0;
}

/* 头部栏 */
.chat-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border-secondary);
  flex-shrink: 0;
}

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

/* 消息区域 */
.chat-messages {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
}

/* 空状态 */
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

/* 错误提示 */
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

/* API Key 未配置提示 */
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

/* 输入区域 */
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
</style>
