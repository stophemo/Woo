<template>
  <div class="lock-dialog-overlay" @click.self="handleCancel">
    <div class="lock-dialog" @click.stop>
      <h3 class="lock-dialog-title">{{ dialogTitle }}</h3>
      <p class="lock-dialog-desc">{{ dialogDesc }}</p>

      <!-- 设置密码模式（首次） -->
      <template v-if="mode === 'set'">
        <div class="lock-input-group">
          <input
            ref="pwInput"
            v-model="password"
            type="password"
            class="lock-input"
            placeholder="创建加锁密码（至少4位）"
            @keydown.enter="handleConfirm"
          />
          <input
            v-model="confirmPassword"
            type="password"
            class="lock-input"
            placeholder="再次确认密码"
            @keydown.enter="handleConfirm"
          />
        </div>
      </template>

      <!-- 验证密码模式（加解锁操作） -->
      <template v-else>
        <div class="lock-input-group">
          <input
            ref="pwInput"
            v-model="password"
            type="password"
            class="lock-input"
            placeholder="输入加锁密码"
            @keydown.enter="handleConfirm"
          />
        </div>
      </template>

      <p v-if="errorMsg" class="lock-error">{{ errorMsg }}</p>

      <div class="lock-actions">
        <button class="lock-btn lock-btn-secondary" @click="handleCancel">取消</button>
        <button class="lock-btn lock-btn-primary" :disabled="loading" @click="handleConfirm">
          {{ loading ? '处理中…' : confirmLabel }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, nextTick, onMounted } from 'vue'
import { useLockStore } from '../../stores/lock'

type DialogMode = 'set' | 'verify'

interface Props {
  mode?: DialogMode
  title?: string
  description?: string
  confirmText?: string
}

const props = withDefaults(defineProps<Props>(), {
  mode: 'verify',
  title: '',
  description: '',
  confirmText: ''
})

const emit = defineEmits<{
  (e: 'confirm', password: string): void
  (e: 'cancel'): void
}>()

const lockStore = useLockStore()

const password = ref('')
const confirmPassword = ref('')
const errorMsg = ref('')
const loading = ref(false)
const pwInput = ref<HTMLInputElement | null>(null)

const dialogTitle = computed(() => {
  if (props.title) return props.title
  return props.mode === 'set' ? '设置加锁密码' : '验证密码'
})

const dialogDesc = computed(() => {
  if (props.description) return props.description
  if (props.mode === 'set') {
    return '设置加锁密码后，可对文件夹和文稿进行加锁保护'
  }
  return '请输入加锁密码以继续'
})

const confirmLabel = computed(() => {
  if (props.confirmText) return props.confirmText
  if (props.mode === 'set') return '确认设置'
  return '确认'
})

function handleCancel() {
  emit('cancel')
}

async function handleConfirm() {
  errorMsg.value = ''

  if (props.mode === 'set') {
    // 设置密码模式：始终使用独立的本地加锁密码
    const pw = password.value
    if (!pw || pw.length < 4) {
      errorMsg.value = '密码长度不能少于 4 位'
      return
    }
    if (pw !== confirmPassword.value) {
      errorMsg.value = '两次输入的密码不一致'
      return
    }
    loading.value = true
    try {
      await lockStore.setPassword(pw)
      emit('confirm', pw)
    } catch (e: any) {
      errorMsg.value = e?.message || '设置失败'
    } finally {
      loading.value = false
    }
    return
  }

  // 验证密码模式
  const pw = password.value
  if (!pw) {
    errorMsg.value = '请输入密码'
    return
  }

  loading.value = true
  try {
    const ok = await lockStore.verify(pw)
    if (ok) {
      emit('confirm', pw)
    } else {
      errorMsg.value = '密码错误'
    }
  } catch (e: any) {
    errorMsg.value = e?.message || '验证失败'
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  nextTick(() => pwInput.value?.focus())
})
</script>

<style scoped>
.lock-dialog-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 10000;
}
.lock-dialog {
  background: var(--bg-elevated);
  border-radius: 12px;
  padding: 24px;
  width: 380px;
  max-width: 90vw;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
}

@media (max-width: 480px) {
  .lock-dialog-overlay {
    align-items: flex-end;
    padding: 0 0 env(safe-area-inset-bottom) 0;
  }
  .lock-dialog {
    width: 100%;
    max-width: 100%;
    border-radius: 16px 16px 0 0;
    border: 1px solid var(--border-primary);
    border-bottom: none;
  }
}
.lock-dialog-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0 0 8px 0;
}
.lock-dialog-desc {
  font-size: 13px;
  color: var(--text-secondary);
  margin: 0 0 16px 0;
}
.lock-input-group {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 12px;
}
.lock-input {
  width: 100%;
  padding: 10px 12px;
  border: 1px solid var(--border-primary);
  border-radius: 8px;
  background: var(--bg-tertiary);
  color: var(--text-primary);
  font-size: 14px;
  outline: none;
  box-sizing: border-box;
  transition: border-color 0.2s;
}
.lock-input:focus {
  border-color: var(--accent);
}
.lock-error {
  font-size: 12px;
  color: #e53935;
  margin: 0 0 12px 0;
}
.lock-hint {
  font-size: 12px;
  color: var(--text-muted);
  margin: 0 0 8px 0;
}
.lock-link-btn {
  background: none;
  border: none;
  color: var(--accent);
  font-size: 12px;
  cursor: pointer;
  padding: 0;
  margin-bottom: 12px;
}
.lock-link-btn:hover {
  text-decoration: underline;
}
.lock-option {
  margin-bottom: 12px;
}
.lock-checkbox {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--text-primary);
  cursor: pointer;
}
.lock-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.lock-btn {
  padding: 8px 20px;
  border-radius: 8px;
  font-size: 13px;
  cursor: pointer;
  border: none;
  transition: opacity 0.2s;
}
.lock-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.lock-btn-primary {
  background: var(--accent);
  color: #fff;
}
.lock-btn-secondary {
  background: var(--bg-tertiary);
  color: var(--text-primary);
  border: 1px solid var(--border-primary);
}
</style>
