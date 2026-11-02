<script setup lang="ts">
/**
 * 登录 / 注册（全屏页）。
 *
 * 之前是设置页里的底部弹层：键盘弹出后弹层不动，输入框直接被键盘挡住，
 * 而且 Vant 的 tabs + 带标签表单在移动端显得笨重。这里改成独立页面：
 * 上方大标题、下方无边框输入行、底部一颗实心主按钮；键盘弹出时整页
 * 按原生上报的遮挡高度上收，按钮始终可见。
 */
import { computed, nextTick, onMounted, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import { showToast } from 'vant'
import { useAuthStore } from '../../src/stores/auth'
import { useSyncStore } from '../../src/stores/sync'
import { useWorkspaceStore } from '../../src/stores/workspace'
import { useKeyboardInset } from '../composables/useKeyboardInset'

const router = useRouter()
const authStore = useAuthStore()
const syncStore = useSyncStore()
const workspaceStore = useWorkspaceStore()
const { inset } = useKeyboardInset()

type Mode = 'login' | 'signup'

const mode = ref<Mode>('login')
const localError = ref('')
const notice = ref('')
const revealPassword = ref(false)
const submitting = ref(false)

const form = reactive({
  identifier: '',
  email: '',
  username: '',
  password: '',
  confirmPassword: '',
})

const identifierInput = ref<HTMLInputElement | null>(null)
const emailInput = ref<HTMLInputElement | null>(null)
const usernameInput = ref<HTMLInputElement | null>(null)
const passwordInput = ref<HTMLInputElement | null>(null)
const confirmInput = ref<HTMLInputElement | null>(null)

const title = computed(() => (mode.value === 'login' ? '登录' : '注册'))
const meta = computed(() => (
  mode.value === 'login'
    ? '登录后可在多台设备间同步文稿'
    : '用邮箱创建账号，文稿会自动同步'
))
const errorText = computed(() => localError.value || authStore.errorMsg)
const submitLabel = computed(() => {
  if (submitting.value || authStore.loading) {
    return mode.value === 'login' ? '登录中…' : '注册中…'
  }
  return mode.value === 'login' ? '登录' : '注册'
})

function focus(el: HTMLInputElement | null) {
  el?.focus()
}

function switchMode(next: Mode) {
  if (mode.value === next) return
  if (next === 'login' && form.email.trim()) {
    form.identifier = form.email.trim()
  }
  mode.value = next
  localError.value = ''
  notice.value = ''
  authStore.errorMsg = ''
  nextTick(() => focus(next === 'login' ? identifierInput.value : emailInput.value))
}

function onBack() {
  if (window.history.length > 1) router.back()
  else router.replace('/settings')
}

async function submit() {
  if (submitting.value || authStore.loading) return
  localError.value = ''
  notice.value = ''
  mode.value === 'login' ? await doLogin() : await doSignup()
}

async function doLogin() {
  const identifier = form.identifier.trim()
  if (!identifier) return void (localError.value = '请填写邮箱或用户名', focus(identifierInput.value))
  if (!form.password) return void (localError.value = '请填写密码', focus(passwordInput.value))

  submitting.value = true
  try {
    const ok = await authStore.login(identifier, form.password)
    if (!ok) {
      localError.value = authStore.errorMsg || '登录失败，请检查账号与密码'
      return
    }
    form.password = ''
    await afterAuthSuccess('登录成功')
  } finally {
    submitting.value = false
  }
}

async function doSignup() {
  const email = form.email.trim()
  const username = form.username.trim()
  if (!email) return void (localError.value = '请填写邮箱', focus(emailInput.value))
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
    return void (localError.value = '邮箱格式不正确', focus(emailInput.value))
  }
  if (!username) return void (localError.value = '请填写用户名', focus(usernameInput.value))
  if (!form.password) return void (localError.value = '请设置密码', focus(passwordInput.value))
  if (form.password !== form.confirmPassword) {
    return void (localError.value = '两次输入的密码不一致', focus(confirmInput.value))
  }

  submitting.value = true
  try {
    const res = await authStore.signup(email, username, form.password)
    if (!res.ok) {
      localError.value = authStore.errorMsg || '注册失败'
      return
    }
    if (res.message) {
      // 项目开启了邮箱确认，账号已创建但还没有会话。
      form.password = ''
      form.confirmPassword = ''
      switchMode('login')
      notice.value = res.message
      return
    }
    form.password = ''
    form.confirmPassword = ''
    await afterAuthSuccess('注册成功')
  } finally {
    submitting.value = false
  }
}

async function afterAuthSuccess(tip: string) {
  const synced = await syncStore.triggerSync()
  await workspaceStore.bootstrap()
  showToast(synced ? `${tip}，已同步` : (syncStore.errorMsg || tip))
  router.replace('/settings')
}

onMounted(() => {
  nextTick(() => focus(identifierInput.value))
})
</script>

<template>
  <div
    class="login-page"
    :class="{ 'is-typing': inset > 0 }"
    :style="{ paddingBottom: `${inset}px` }"
  >
    <header class="login-nav">
      <button type="button" class="w-icon-btn" aria-label="返回" @click="onBack">
        <van-icon name="arrow-left" />
      </button>
    </header>

    <div class="login-body">
      <div class="w-head">
        <div class="w-head__row">
          <div class="w-head__titles">
            <span class="w-head__kicker">无我笔记</span>
            <h1 class="w-head__title">{{ title }}</h1>
          </div>
        </div>
        <p class="w-head__meta">{{ meta }}</p>
      </div>

      <form class="login-form" @submit.prevent="submit">
        <label v-if="mode === 'login'" class="fld">
          <span class="fld__label">邮箱或用户名</span>
          <input
            ref="identifierInput"
            v-model="form.identifier"
            class="fld__input"
            type="text"
            inputmode="email"
            autocomplete="username"
            autocapitalize="off"
            spellcheck="false"
            enterkeyhint="next"
            placeholder="you@example.com"
            @keydown.enter.prevent="focus(passwordInput)"
          >
        </label>

        <template v-else>
          <label class="fld">
            <span class="fld__label">邮箱</span>
            <input
              ref="emailInput"
              v-model="form.email"
              class="fld__input"
              type="email"
              inputmode="email"
              autocomplete="email"
              autocapitalize="off"
              spellcheck="false"
              enterkeyhint="next"
              placeholder="you@example.com"
              @keydown.enter.prevent="focus(usernameInput)"
            >
          </label>

          <label class="fld">
            <span class="fld__label">用户名</span>
            <input
              ref="usernameInput"
              v-model="form.username"
              class="fld__input"
              type="text"
              autocomplete="username"
              autocapitalize="off"
              spellcheck="false"
              enterkeyhint="next"
              placeholder="用于区分云端数据库"
              @keydown.enter.prevent="focus(passwordInput)"
            >
          </label>
        </template>

        <label class="fld">
          <span class="fld__label">密码</span>
          <span class="fld__row">
            <input
              ref="passwordInput"
              v-model="form.password"
              class="fld__input"
              :type="revealPassword ? 'text' : 'password'"
              :autocomplete="mode === 'login' ? 'current-password' : 'new-password'"
              :enterkeyhint="mode === 'login' ? 'go' : 'next'"
              :placeholder="mode === 'login' ? '请输入密码' : '至少 6 位'"
              @keydown.enter.prevent="mode === 'login' ? submit() : focus(confirmInput)"
            >
            <button
              type="button"
              class="fld__action"
              @click="revealPassword = !revealPassword"
            >{{ revealPassword ? '隐藏' : '显示' }}</button>
          </span>
        </label>

        <label v-if="mode === 'signup'" class="fld">
          <span class="fld__label">确认密码</span>
          <input
            ref="confirmInput"
            v-model="form.confirmPassword"
            class="fld__input"
            :type="revealPassword ? 'text' : 'password'"
            autocomplete="new-password"
            enterkeyhint="go"
            placeholder="再次输入密码"
            @keydown.enter.prevent="submit"
          >
        </label>

        <p v-if="errorText" class="login-error">{{ errorText }}</p>
        <p v-else-if="notice" class="login-notice">{{ notice }}</p>
      </form>
    </div>

    <!-- 主按钮固定在滚动区之外：字段再多、键盘再高也不会被挤到屏幕外 -->
    <footer class="login-foot">
      <button
        type="button"
        class="login-submit"
        :disabled="submitting || authStore.loading"
        @click="submit"
      >{{ submitLabel }}</button>
      <div class="login-foot__switch">
        <span class="login-foot__hint">{{ mode === 'login' ? '还没有账号？' : '已经有账号了？' }}</span>
        <button
          type="button"
          class="login-foot__link"
          @click="switchMode(mode === 'login' ? 'signup' : 'login')"
        >{{ mode === 'login' ? '立即注册' : '去登录' }}</button>
      </div>
    </footer>
  </div>
</template>

<style scoped>
.login-page {
  position: fixed;
  inset: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--w-paper);
  /* 键盘遮挡高度由原生上报，避让后主按钮始终可见 */
  transition: padding-bottom 0.16s ease-out;
}

.login-nav {
  display: flex;
  align-items: center;
  height: 44px;
  padding: 0 8px;
}

.login-body {
  flex: 1;
  padding: 6px 24px 24px;
  overflow-y: auto;
  -webkit-overflow-scrolling: touch;
}

.login-form {
  margin-top: 30px;
}

/* 键盘弹出时收起大标题：否则注册的四个字段放不下，最后一个会被主按钮压住 */
.login-page.is-typing .w-head {
  display: none;
}

.login-page.is-typing .login-form {
  margin-top: 10px;
}

/* ---------- 输入行：无边框，只留一条底线 ---------- */
.fld {
  display: block;
  padding: 9px 0 11px;
  border-bottom: 1px solid var(--w-line);
}

.fld:focus-within {
  border-bottom-color: var(--w-accent);
}

.fld__label {
  display: block;
  color: var(--w-ink-3);
  font-size: 11.5px;
  letter-spacing: 0.06em;
}

.fld__row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.fld__input {
  width: 100%;
  margin-top: 6px;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--w-ink);
  font-family: inherit;
  font-size: 16.5px;
  line-height: 1.5;
  outline: none;
  -webkit-text-size-adjust: 100%;
}

.fld__input::placeholder {
  color: var(--w-ink-3);
  opacity: 0.75;
}

.fld__action {
  margin-top: 6px;
  padding: 2px 0 2px 8px;
  border: 0;
  background: transparent;
  color: var(--w-ink-3);
  font-size: 12px;
  white-space: nowrap;
}

.fld__action:active {
  color: var(--w-accent);
}

/* ---------- 提示与提交 ---------- */
.login-error,
.login-notice {
  margin: 14px 0 0;
  font-size: 12.5px;
  line-height: 1.6;
}

.login-error {
  color: var(--van-danger-color);
}

.login-notice {
  color: var(--w-accent);
}

.login-submit {
  width: 100%;
  height: 48px;
  border: 0;
  border-radius: 12px;
  background: var(--w-accent);
  /* 深色主题下主色是浅蓝，白字对比不足；用纸面色做前景可随主题自动翻转 */
  color: var(--w-paper);
  font-family: inherit;
  font-size: 16px;
  font-weight: 600;
  letter-spacing: 0.04em;
  transition: opacity 0.15s ease;
}

.login-submit:active {
  opacity: 0.85;
}

.login-submit:disabled {
  opacity: 0.55;
}

/* ---------- 底部模式切换 ---------- */
.login-foot {
  flex: 0 0 auto;
  padding: 10px 24px calc(14px + env(safe-area-inset-bottom));
  background: var(--w-paper);
}

.login-foot__switch {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding-top: 12px;
  font-size: 13px;
}

.login-foot__hint {
  color: var(--w-ink-3);
}

.login-foot__link {
  padding: 4px 2px;
  border: 0;
  background: transparent;
  color: var(--w-accent);
  font-family: inherit;
  font-size: 13px;
  font-weight: 500;
}
</style>
