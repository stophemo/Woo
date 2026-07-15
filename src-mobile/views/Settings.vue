<script setup lang="ts">
import { ref, reactive } from 'vue'
import { useRouter } from 'vue-router'
import { showToast } from 'vant'
import { useAuthStore } from '../../src/stores/auth'
import { useSyncStore } from '../../src/stores/sync'
import { useWorkspaceStore } from '../../src/stores/workspace'
import { useLockStore } from '../../src/stores/lock'
import { useThemeStore } from '../../src/stores/theme'

const router = useRouter()
const appVersion = __APP_VERSION__
const authStore = useAuthStore()
const syncStore = useSyncStore()
const workspaceStore = useWorkspaceStore()
const lockStore = useLockStore()
const themeStore = useThemeStore()

const showLogin = ref(false)
const loginMode = ref<'login' | 'signup'>('login')
const form = reactive({
  identifier: '',
  email: '',
  username: '',
  password: '',
  confirmPassword: '',
})

function resetForm() {
  form.identifier = ''
  form.email = ''
  form.username = ''
  form.password = ''
  form.confirmPassword = ''
  authStore.errorMsg = ''
}

function openLogin() {
  loginMode.value = 'login'
  resetForm()
  showLogin.value = true
}

function switchMode(mode: 'login' | 'signup') {
  loginMode.value = mode
  authStore.errorMsg = ''
}

async function onSubmit() {
  if (loginMode.value === 'login') {
    if (!form.identifier.trim() || !form.password) {
      showToast('请填写账号和密码')
      return
    }
    const ok = await authStore.login(form.identifier.trim(), form.password)
    if (ok) {
      showLogin.value = false
      const synced = await syncStore.triggerSync()
      // 同步完成后再读取用户库，避免首屏停留在同步前的空状态。
      await workspaceStore.bootstrap()
      showToast(synced ? '登录并同步成功' : (syncStore.errorMsg || '登录成功，同步失败'))
    } else {
      showToast(authStore.errorMsg || '登录失败')
    }
    return
  }
  // 注册
  if (!form.email.trim() || !form.username.trim() || !form.password) {
    showToast('请完整填写邮箱、用户名和密码')
    return
  }
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(form.email.trim())) {
    showToast('邮箱格式不正确')
    return
  }
  if (form.password !== form.confirmPassword) {
    showToast('两次输入的密码不一致')
    return
  }
  const res = await authStore.signup(form.email.trim(), form.username.trim(), form.password)
  if (!res.ok) {
    showToast(authStore.errorMsg || '注册失败')
    return
  }
  if (res.message) {
    // 需邮箱确认，尚未登录：提示并切回登录模式
    showToast(res.message)
    switchMode('login')
  } else {
    // 直接注册并登录成功
    showLogin.value = false
    const synced = await syncStore.triggerSync()
    await workspaceStore.bootstrap()
    showToast(synced ? '注册并同步成功' : (syncStore.errorMsg || '注册成功，同步失败'))
  }
}

async function onLogout() {
  await authStore.logout()
  showToast('已退出登录')
  await workspaceStore.bootstrap()
}

async function onSyncNow() {
  if (!authStore.isLoggedIn) {
    showToast('请先登录后再同步')
    return
  }
  const ok = await syncStore.triggerSync()
  showToast(ok ? '同步完成' : (syncStore.errorMsg || '同步失败'))
}

// ------- 密码锁 -------
const showPwdDialog = ref(false)
const pwd = reactive({ value: '', confirm: '' })

function openPwdDialog() {
  pwd.value = ''
  pwd.confirm = ''
  showPwdDialog.value = true
}

// 返回 false 阻止对话框关闭（校验不通过时保持打开）
async function beforePwdClose(action: string) {
  if (action !== 'confirm') return true
  if (pwd.value.length < 4) {
    showToast('密码至少 4 位')
    return false
  }
  if (pwd.value !== pwd.confirm) {
    showToast('两次输入不一致')
    return false
  }
  try {
    const existed = lockStore.hasPassword
    await lockStore.setPassword(pwd.value)
    showToast(existed ? '密码已更新' : '密码锁已设置')
    return true
  } catch (e: any) {
    showToast(e?.message || '设置失败')
    return false
  }
}
</script>

<template>
  <div class="settings-page">
    <van-nav-bar title="设置" />

    <!-- 账户 -->
    <van-cell-group inset title="账户">
      <template v-if="authStore.isLoggedIn">
        <van-cell title="账号" :value="authStore.user?.email || authStore.user?.username || '已登录'" />
        <van-cell title="退出登录" is-link :label="authStore.loading ? '正在退出…' : ''" @click="onLogout" />
      </template>
      <van-cell v-else title="登录 / 注册" is-link value="未登录" @click="openLogin" />
    </van-cell-group>

    <!-- 云同步 -->
    <van-cell-group inset title="云同步">
      <van-cell title="同步状态">
        <template #value>
          <span>{{ syncStore.lastSyncLabel }}</span>
          <van-tag v-if="syncStore.pendingChanges > 0" type="warning" class="pending-tag">
            待同步 {{ syncStore.pendingChanges }}
          </van-tag>
        </template>
      </van-cell>
      <van-cell
        title="立即同步"
        is-link
        :label="!authStore.isLoggedIn ? '登录后可用' : ''"
        @click="onSyncNow"
      >
        <template #value>
          <van-loading v-if="syncStore.isSyncing" size="16" />
        </template>
      </van-cell>
    </van-cell-group>

    <!-- 数据 -->
    <van-cell-group inset title="数据">
      <van-cell title="回收站" is-link icon="delete-o" @click="router.push('/trash')" />
    </van-cell-group>

    <!-- 安全 -->
    <van-cell-group inset title="安全">
      <van-cell
        :title="lockStore.hasPassword ? '修改密码锁' : '设置密码锁'"
        is-link
        :label="lockStore.hasPassword ? '已启用，可在笔记列表滑动加锁' : '设置后可对笔记加锁隐藏'"
        @click="openPwdDialog"
      />
    </van-cell-group>

    <!-- 外观 -->
    <van-cell-group inset title="外观">
      <van-cell title="深色模式" center>
        <template #right-icon>
          <van-switch
            :model-value="themeStore.theme === 'dark'"
            size="20"
            @update:model-value="(v: boolean) => (themeStore.theme = v ? 'dark' : 'light')"
          />
        </template>
      </van-cell>
    </van-cell-group>

    <!-- 关于 -->
    <van-cell-group inset title="关于">
      <van-cell title="版本" :value="`v${appVersion}`" />
      <van-cell title="项目主页" value="GitHub" is-link @click="showToast('打开链接')" />
    </van-cell-group>

    <!-- 登录 / 注册 弹层 -->
    <van-popup v-model:show="showLogin" position="bottom" round :style="{ paddingBottom: '24px' }">
      <div class="login-sheet">
        <van-tabs v-model:active="loginMode" @change="(m: string) => switchMode(m as 'login' | 'signup')">
          <van-tab title="登录" name="login" />
          <van-tab title="注册" name="signup" />
        </van-tabs>

        <van-form class="login-form" @submit="onSubmit">
          <template v-if="loginMode === 'login'">
            <van-field
              v-model="form.identifier"
              label="账号"
              placeholder="邮箱或用户名"
              clearable
            />
            <van-field
              v-model="form.password"
              type="password"
              label="密码"
              placeholder="请输入密码"
            />
          </template>

          <template v-else>
            <van-field
              v-model="form.email"
              label="邮箱"
              placeholder="用于登录与找回"
              clearable
            />
            <van-field
              v-model="form.username"
              label="用户名"
              placeholder="用于云端分库标识"
              clearable
            />
            <van-field
              v-model="form.password"
              type="password"
              label="密码"
              placeholder="请设置密码"
            />
            <van-field
              v-model="form.confirmPassword"
              type="password"
              label="确认密码"
              placeholder="请再次输入密码"
            />
          </template>

          <div v-if="authStore.errorMsg" class="login-error">{{ authStore.errorMsg }}</div>

          <div class="login-actions">
            <van-button
              block
              round
              type="primary"
              native-type="submit"
              :loading="authStore.loading"
            >
              {{ loginMode === 'login' ? '登录' : '注册' }}
            </van-button>
          </div>
        </van-form>
      </div>
    </van-popup>

    <!-- 密码锁设置对话框 -->
    <van-dialog
      v-model:show="showPwdDialog"
      :title="lockStore.hasPassword ? '修改密码锁' : '设置密码锁'"
      show-cancel-button
      :before-close="beforePwdClose"
    >
      <div class="pwd-fields">
        <van-field v-model="pwd.value" type="password" label="密码" placeholder="至少 4 位" />
        <van-field v-model="pwd.confirm" type="password" label="确认" placeholder="再次输入" />
      </div>
    </van-dialog>
  </div>
</template>

<style scoped>
.pending-tag {
  margin-left: 8px;
}
.login-sheet {
  padding: 8px 0 0;
}
.login-form {
  padding: 12px 0;
}
.login-error {
  color: #ee0a24;
  font-size: 13px;
  padding: 8px 16px 0;
}
.login-actions {
  padding: 16px;
}
.pwd-fields {
  padding: 8px 0;
}
</style>
