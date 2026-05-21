<template>
  <Teleport to="body">
    <div v-if="visible" class="login-overlay" @click.self="$emit('close')" @keydown.esc="$emit('close')">
      <div class="login-dialog">
        <div class="login-header">
          <h2>{{ isLoggedIn ? '账户信息' : '登录账户' }}</h2>
          <button class="login-close-btn" @click="$emit('close')">
            <IconClose />
          </button>
        </div>

        <!-- 已登录：展示账户信息 -->
        <div v-if="isLoggedIn" class="login-body">
          <div class="account-profile">
            <div class="account-avatar">
              <img v-if="user?.avatar" :src="user.avatar" alt="avatar" />
              <span v-else class="account-avatar-fallback">{{ avatarText }}</span>
            </div>
            <div class="account-meta">
              <div class="account-nickname">{{ user?.nickname || user?.username || '未命名用户' }}</div>
              <div class="account-username">@{{ user?.username }}</div>
            </div>
          </div>

          <div class="account-info-list">
            <div class="account-info-row">
              <span class="account-info-label">用户 ID</span>
              <span class="account-info-value">{{ user?.userId || '-' }}</span>
            </div>
            <div class="account-info-row">
              <span class="account-info-label">邮箱</span>
              <span class="account-info-value">{{ user?.email || '未绑定' }}</span>
            </div>
          </div>

          <div v-if="errorMsg" class="login-error">{{ errorMsg }}</div>
          <div class="login-actions">
            <button class="login-btn danger" @click="handleLogout" :disabled="submitting">
              {{ submitting ? '退出中...' : '退出登录' }}
            </button>
            <button class="login-btn" @click="$emit('close')">关闭</button>
          </div>
        </div>

        <!-- 未登录：展示登录表单 -->
        <div v-else class="login-body">
          <div class="login-field">
            <label>账号</label>
            <input
              type="text"
              v-model="username"
              placeholder="请输入账号 / 邮箱"
              class="login-input"
            />
          </div>
          <div class="login-field">
            <label>密码</label>
            <input
              type="password"
              v-model="password"
              placeholder="请输入密码"
              class="login-input"
              @keydown.enter="handleLogin"
            />
          </div>
          <div v-if="errorMsg" class="login-error">{{ errorMsg }}</div>
          <div class="login-actions">
            <button class="login-btn primary" @click="handleLogin" :disabled="submitting">
              {{ submitting ? '登录中...' : '登录' }}
            </button>
            <button class="login-btn" @click="$emit('close')">取消</button>
          </div>
          <div class="login-footer">
            <a href="#" @click.prevent>忘记密码？</a>
            <a href="#" @click.prevent>注册新账号</a>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { storeToRefs } from 'pinia'
import IconClose from '../icons/IconClose.vue'
import { useAuthStore } from '../../stores/auth'

interface Props {
  visible: boolean
}

const props = defineProps<Props>()
const emit = defineEmits<{ close: [], 'login-success': [], logout: [] }>()

const authStore = useAuthStore()
const { user, isLoggedIn } = storeToRefs(authStore)

const username = ref('')
const password = ref('')
const submitting = ref(false)
const errorMsg = ref('')

const avatarText = computed(() => {
  const name = user.value?.nickname || user.value?.username || ''
  return name ? name.charAt(0).toUpperCase() : '?'
})

watch(() => props.visible, (val) => {
  if (val) {
    username.value = ''
    password.value = ''
    errorMsg.value = ''
    submitting.value = false
  }
})

async function handleLogin() {
  if (!username.value.trim() || !password.value.trim()) {
    errorMsg.value = '请输入账号和密码'
    return
  }
  errorMsg.value = ''
  submitting.value = true
  try {
    const ok = await authStore.login(username.value.trim(), password.value)
    if (ok) {
      emit('login-success')
      emit('close')
    } else {
      errorMsg.value = authStore.errorMsg || '登录失败'
    }
  } catch (e: any) {
    errorMsg.value = e?.message || '登录失败，请重试'
  } finally {
    submitting.value = false
  }
}

function handleLogout() {
  submitting.value = true
  try {
    authStore.logout()
    emit('logout')
    emit('close')
  } finally {
    submitting.value = false
  }
}
</script>

<style scoped>
.login-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.4);
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
}

.login-dialog {
  background-color: var(--bg-surface);
  border: 1px solid var(--border-primary);
  border-radius: 8px;
  width: 380px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
}

.login-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-secondary);
}

.login-header h2 {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0;
}

.login-close-btn {
  background: transparent;
  border: none;
  cursor: pointer;
  padding: 4px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  color: var(--text-secondary);
  transition: all 0.2s;
}

.login-close-btn:hover {
  background-color: var(--bg-hover);
  color: var(--text-primary);
}

.login-body {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.login-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.login-field label {
  font-size: 13px;
  color: var(--text-secondary);
}

.login-input {
  padding: 8px 12px;
  border: 1px solid var(--border-primary);
  border-radius: 4px;
  font-size: 13px;
  background-color: var(--bg-elevated);
  color: var(--text-primary);
  outline: none;
  transition: border-color 0.2s;
}

.login-input:focus {
  border-color: var(--accent);
}

.login-error {
  font-size: 12px;
  color: #e53935;
}

.login-actions {
  display: flex;
  gap: 8px;
  margin-top: 4px;
}

.login-btn {
  flex: 1;
  padding: 8px 16px;
  border: 1px solid var(--border-primary);
  border-radius: 4px;
  background-color: var(--bg-elevated);
  color: var(--text-primary);
  cursor: pointer;
  font-size: 13px;
  transition: all 0.2s;
}

.login-btn:hover:not(:disabled) {
  background-color: var(--bg-hover);
}

.login-btn.primary {
  background-color: var(--accent);
  color: #ffffff;
  border-color: var(--accent);
}

.login-btn.primary:hover:not(:disabled) {
  background-color: var(--accent-hover);
}

.login-btn.danger {
  background-color: transparent;
  color: #e53935;
  border-color: #e53935;
}

.login-btn.danger:hover:not(:disabled) {
  background-color: rgba(229, 57, 53, 0.08);
}

.login-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.account-profile {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 4px 0 12px;
  border-bottom: 1px solid var(--border-secondary);
}

.account-avatar {
  width: 56px;
  height: 56px;
  border-radius: 50%;
  background-color: var(--accent);
  color: #ffffff;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  flex-shrink: 0;
}

.account-avatar img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.account-avatar-fallback {
  font-size: 22px;
  font-weight: 600;
}

.account-meta {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.account-nickname {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.account-username {
  font-size: 12px;
  color: var(--text-secondary);
}

.account-info-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.account-info-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 13px;
}

.account-info-label {
  color: var(--text-secondary);
}

.account-info-value {
  color: var(--text-primary);
  max-width: 70%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.login-footer {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
}

.login-footer a {
  color: var(--accent);
  text-decoration: none;
}

.login-footer a:hover {
  text-decoration: underline;
}
</style>
