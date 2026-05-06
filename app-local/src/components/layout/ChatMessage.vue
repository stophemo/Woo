<template>
  <div class="chat-message" :class="message.role">
    <div class="message-bubble">
      <div class="message-content" v-html="renderedContent"></div>
      <span v-if="message.isStreaming" class="streaming-cursor">&#9608;</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { marked } from 'marked'
import type { ChatMessage } from '../../types/ai'

interface Props {
  message: ChatMessage
}

const props = defineProps<Props>()

// 配置 marked
marked.setOptions({
  breaks: true,
  gfm: true
})

const renderedContent = computed(() => {
  if (!props.message.content) return ''
  if (props.message.role === 'user') {
    // 用户消息：简单转义，保留换行
    return props.message.content
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/\n/g, '<br>')
  }
  // 助手消息：渲染 Markdown
  return marked.parse(props.message.content) as string
})
</script>

<style scoped>
.chat-message {
  display: flex;
  margin-bottom: 12px;
}

.chat-message.user {
  justify-content: flex-end;
}

.chat-message.assistant {
  justify-content: flex-start;
}

.message-bubble {
  max-width: 85%;
  padding: 10px 14px;
  border-radius: 12px;
  font-size: 13px;
  line-height: 1.6;
  word-wrap: break-word;
  overflow-wrap: break-word;
}

.user .message-bubble {
  background-color: var(--accent);
  color: #ffffff;
  border-bottom-right-radius: 4px;
}

.assistant .message-bubble {
  background-color: var(--bg-elevated);
  color: var(--text-primary);
  border: 1px solid var(--border-secondary);
  border-bottom-left-radius: 4px;
}

.streaming-cursor {
  display: inline-block;
  animation: blink 1s step-end infinite;
  font-size: 14px;
  line-height: 1;
  vertical-align: text-bottom;
  color: var(--text-muted);
}

@keyframes blink {
  0%, 50% { opacity: 1; }
  51%, 100% { opacity: 0; }
}
</style>
