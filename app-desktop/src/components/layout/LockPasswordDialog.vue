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
            :placeholder="loggedIn ? '输入加锁密码（至少4位）' : '创建加锁密码（至少4位）'"
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
        <div v-if="loggedIn && !lockStore.hasPassword" class="lock-option">
          <label class="lock-checkbox">
            <input v-model="useAccountPw" type="checkbox" />
            <span>使用账号密码加解锁</span>
          </label>
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
        <p v-if="loggedIn && !lockStore.hasPassword && !showCustomOption" class="lock-hint">
          当前使用账号密码加解锁
        </p>
        <button v-if="loggedIn && !lockStore.hasPassword && !showCustomOption" class="lock-link-btn" @click="showCustomOption = true">
          使用独立加锁密码
        </button>
        <template v-if="showCustomOption">
          <div class="lock-input-group">
            <input
              v-model="newCustomPw"
              type="password"
              class="lock-input"
              placeholder="设置独立加锁密码"
              @keydown.enter="handleConfirm"
            />
          </div>
        </template>
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
import { useAuthStore } from '../../stores/auth'

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
const authStore = useAuthStore()

const loggedIn = computed(() => authStore.isLoggedIn)
const password = ref('')
const confirmPassword = ref('')
const newCustomPw = ref('')
const useAccountPw = ref(false)
const showCustomOption = ref(false)
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
    // 设置密码模式
    if (useAccountPw.value && loggedIn.value) {
      // 使用账号密码 → 不需要存储，直接标记 account 模式
      await lockStore.setPassword(password.value)
      emit('confirm', '')
      return
    }

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
    // 如果已登录且没有自定义密码 → 用账号密码
    if (loggedIn.value && !lockStore.hasPassword) {
      // 直接验证通过（IPC 端会检查账号密码）
      const ok = await lockStore.verify(pw)
      if (ok) {
        emit('confirm', pw)
      } else {
        errorMsg.value = '密码错误'
      }
      return
    }

    // 如果用户正在设置独立密码（验证模式下切换）
    if (showCustomOption.value && newCustomPw.value) {
      await lockStore.setPassword(newCustomPw.value)
      emit('confirm', newCustomPw.value)
      return
    }

    // 普通验证
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
