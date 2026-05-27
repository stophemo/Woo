<template>
  <Teleport to="body">
    <div v-if="visible" class="login-overlay">
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
              <img v-if="user?.avatarUrl" :src="user.avatarUrl" alt="avatar" />
              <span v-else class="account-avatar-fallback">{{ avatarText }}</span>
            </div>
            <div class="account-meta">
              <div class="account-nickname">{{ user?.nickname || '未命名用户' }}</div>
              <div class="account-username">{{ user?.email || '' }}</div>
            </div>
          </div>

          <div class="account-info-list">
            <div class="account-info-row">
              <span class="account-info-label">用户 ID</span>
              <span class="account-info-value">{{ user?.id ? user.id.slice(0, 8) + '...' : '-' }}</span>
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

        <!-- 未登录：展示登录/注册表单 -->
        <div v-else class="login-body">
          <div class="login-tabs">
            <button :class="{ active: loginMode === 'login' }" @click="loginMode = 'login'">登录</button>
            <button :class="{ active: loginMode === 'signup' }" @click="loginMode = 'signup'">注册</button>
          </div>

          <div class="login-field">
            <label>{{ loginMode === 'login' ? '邮箱 / 用户名' : '邮箱' }}</label>
            <input
              ref="emailInputRef"
              :type="loginMode === 'login' ? 'text' : 'email'"
              v-model="email"
              :placeholder="loginMode === 'login' ? '邮箱或用户名' : '请输入邮箱'"
              class="login-input"
              :class="{ error: loginMode === 'signup' && fieldErrors.email }"
              @input="onFieldInput('email')"
              @blur="onFieldBlur('email')"
              @keydown.enter="handleSubmit"
            />
            <div v-if="loginMode === 'signup' && fieldErrors.email" class="field-error">{{ fieldErrors.email }}</div>
          </div>
          <div v-if="loginMode === 'signup'" class="login-field">
            <label>用户名</label>
            <input
              type="text"
              v-model="username"
              placeholder="用于登录和标识"
              class="login-input"
              :class="{ error: fieldErrors.username }"
              @input="onFieldInput('username')"
              @blur="onFieldBlur('username')"
              @keydown.enter="handleSubmit"
            />
            <div v-if="fieldErrors.username" class="field-error">{{ fieldErrors.username }}</div>
          </div>
          <div class="login-field">
            <label>密码</label>
            <input
              type="password"
              v-model="password"
              placeholder="请输入密码"
              class="login-input"
              :class="{ error: loginMode === 'signup' && fieldErrors.password }"
              @input="onFieldInput('password')"
              @blur="onFieldBlur('password')"
              @keydown.enter="handleSubmit"
            />
            <div v-if="loginMode === 'signup' && fieldErrors.password" class="field-error">{{ fieldErrors.password }}</div>
          </div>
          <div v-if="loginMode === 'signup'" class="login-field">
            <label>确认密码</label>
            <input
              type="password"
              v-model="confirmPassword"
              placeholder="再次输入密码"
              class="login-input"
              :class="{ error: fieldErrors.confirmPwd }"
              @input="onFieldInput('confirmPwd')"
              @blur="onFieldBlur('confirmPwd')"
              @keydown.enter="handleSubmit"
            />
            <div v-if="fieldErrors.confirmPwd" class="field-error">{{ fieldErrors.confirmPwd }}</div>
          </div>
          <div v-if="errorMsg" class="login-error">{{ errorMsg }}</div>
          <div class="login-actions">
            <button class="login-btn primary" @click="handleSubmit" :disabled="submitting">
              {{ submitting ? (loginMode === 'login' ? '登录中...' : '注册中...') : (loginMode === 'login' ? '登录' : '注册') }}
            </button>
            <button class="login-btn" @click="$emit('close')">取消</button>
          </div>
          <div v-if="loginMode === 'signup'" class="login-tip">
            注册后请查看邮箱完成确认；用户名仅限字母、数字、下划线
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch, computed, nextTick } from 'vue'
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

const emailInputRef = ref<HTMLInputElement | null>(null)
const loginMode = ref<'login' | 'signup'>('login')
const email = ref('')
const username = ref('')
const password = ref('')
const confirmPassword = ref('')
const submitting = ref(false)
const errorMsg = ref('')

// 实时字段校验（仅注册模式生效）
const fieldErrors = ref<Record<string, string>>({
  email: '',
  username: '',
  password: '',
  confirmPwd: ''
})

function validateField(field: string): string {
  if (loginMode.value !== 'signup') return ''
  const e = email.value.trim()
  const u = username.value.trim()
  const p = password.value
  const cp = confirmPassword.value
  switch (field) {
    case 'email':
      if (!e) return '请输入邮箱'
      return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(e) ? '' : '邮箱格式不正确'
    case 'username':
      if (!u) return '请输入用户名'
      if (u.length < 2) return '用户名至少 2 个字符'
      return /^[a-zA-Z0-9_]+$/.test(u) ? '' : '用户名仅限字母、数字、下划线'
    case 'password':
      if (!p) return '请输入密码'
      return p.length < 6 ? '密码长度至少 6 位' : ''
    case 'confirmPwd':
      if (!cp) return '请再次输入密码确认'
      return p !== cp ? '两次输入的密码不一致' : ''
  }
  return ''
}

function onFieldInput(field: string) {
  fieldErrors.value[field] = validateField(field)
}

function onFieldBlur(field: string) {
  fieldErrors.value[field] = validateField(field)
}

const avatarText = computed(() => {
  const name = user.value?.username || user.value?.email || user.value?.nickname || ''
  return name ? name.charAt(0).toUpperCase() : '?'
})

watch(() => props.visible, (val) => {
  if (val) {
    email.value = ''
    username.value = ''
    password.value = ''
    confirmPassword.value = ''
    errorMsg.value = ''
    submitting.value = false
    loginMode.value = 'login'
    clearErrors()
    nextTick(() => emailInputRef.value?.focus())
  }
})

watch(loginMode, () => clearErrors())

function clearErrors() {
  fieldErrors.value = { email: '', username: '', password: '', confirmPwd: '' }
}

async function handleSubmit() {
  // 注册模式：先校验所有必填字段
  if (loginMode.value === 'signup') {
    // 校验所有字段（触发实时校验的显示）
    for (const f of ['email', 'username', 'password', 'confirmPwd']) {
      fieldErrors.value[f] = validateField(f)
    }
    const hasError = Object.values(fieldErrors.value).some(v => !!v)
    if (hasError) return
  } else {
    if (!email.value.trim() || !password.value.trim()) {
      errorMsg.value = '请输入邮箱/用户名和密码'
      return
    }
  }
  errorMsg.value = ''
  submitting.value = true
  try {
    if (loginMode.value === 'login') {
      const ok = await authStore.login(email.value.trim(), password.value)
      if (ok) {
        emit('login-success')
        emit('close')
      } else {
        errorMsg.value = authStore.errorMsg || '登录失败'
      }
    } else {
      const result = await authStore.signup(email.value.trim(), username.value.trim(), password.value)
      if (result.ok) {
        if (result.message) {
          // 需要邮箱确认
          errorMsg.value = result.message
        } else {
          emit('login-success')
          emit('close')
        }
      } else {
        errorMsg.value = authStore.errorMsg || '注册失败'
      }
    }
  } catch (e: any) {
    errorMsg.value = e?.message || '操作失败，请重试'
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

.login-tabs {
  display: flex;
  border-bottom: 1px solid var(--border-secondary);
  margin-bottom: 4px;
}

.login-tabs button {
  flex: 1;
  padding: 6px 0;
  background: transparent;
  border: none;
  border-bottom: 2px solid transparent;
  color: var(--text-secondary);
  font-size: 14px;
  cursor: pointer;
  transition: all 0.2s;
}

.login-tabs button.active {
  color: var(--accent);
  border-bottom-color: var(--accent);
}

.login-tabs button:hover {
  color: var(--text-primary);
}

.login-tip {
  font-size: 12px;
  color: var(--text-secondary);
  text-align: center;
  line-height: 1.5;
}

.login-input.error {
  border-color: #e74c3c;
}

.field-error {
  font-size: 11px;
  color: #e74c3c;
  margin-top: 2px;
  line-height: 1.3;
}
</style>
