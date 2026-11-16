<script setup lang="ts">
/**
 * WebDAV 同步设置。
 *
 * 文稿直接存到用户自己的 WebDAV（坚果云、NAS、Nextcloud 等），不经过任何第三方云。
 * 免费版每月 1GB 上传 / 3GB 下载，对纯文本笔记足够；图片会在编辑器里压缩。
 */
import { computed, onMounted, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import { showToast } from 'vant'
import { invoke } from '../../src/services/api'
import { useSyncStore } from '../../src/stores/sync'

interface WebdavConfigView {
  enabled: boolean
  active: boolean
  url: string
  username: string
  folder: string
  hasPassword: boolean
  configured: boolean
}

interface LanPairResult {
  url: string
  username: string
  password: string
  folder: string
}

const router = useRouter()
const syncStore = useSyncStore()

const loading = ref(true)
const saving = ref(false)
const testing = ref(false)
const pairCode = ref('')
const pairing = ref(false)
const passwordTouched = ref(false)
const saved = ref<WebdavConfigView | null>(null)

const form = reactive({
  enabled: false,
  url: '',
  username: '',
  password: '',
  folder: 'Woo',
})

const statusText = computed(() => {
  if (syncStore.backend === 'webdav') return 'WebDAV 同步生效中'
  if (syncStore.backend === 'lan') return '局域网同步生效中'
  return '尚未启用同步'
})

const canReuseSavedPassword = computed(() =>
  saved.value?.active === true
    && saved.value?.hasPassword === true
    && form.username.trim() === saved.value?.username,
)

const hasUsablePassword = computed(() => !!form.password || canReuseSavedPassword.value)

const passwordHint = computed(() => {
  if (canReuseSavedPassword.value) return '已保存，留空则不修改'
  return 'WebDAV 密码/应用密码，或电脑局域网同步的配对码'
})

const bindingMismatch = computed(() =>
  saved.value?.enabled === true && saved.value?.active === false,
)

const canSave = computed(() => {
  if (!form.url.trim()) return false
  if (form.enabled && !form.username.trim()) return false
  if (form.enabled && !hasUsablePassword.value) return false
  return true
})

async function loadConfig() {
  try {
    const config = await invoke<WebdavConfigView>('webdav:getConfig')
    saved.value = config
    form.enabled = config.enabled
    form.url = config.url || ''
    form.username = config.username
    form.folder = config.folder || 'Woo'
    form.password = ''
    passwordTouched.value = false
  } catch (error: any) {
    showToast(error?.message || '读取配置失败')
  } finally {
    loading.value = false
  }
}

function fillJianguoyunPreset() {
  form.url = 'https://dav.jianguoyun.com/dav'
  form.folder = 'Woo'
  form.enabled = true
  showToast('已填入坚果云地址，再填写账号和应用密码')
}

function onPasswordInput() {
  passwordTouched.value = true
}

function buildPayload() {
  return {
    enabled: form.enabled,
    url: form.url.trim(),
    username: form.username.trim(),
    password: passwordTouched.value ? form.password : null,
    folder: form.folder.trim() || 'Woo',
  }
}

async function testConnection() {
  if (testing.value) return
  if (!form.url.trim()) {
    showToast('请先填写 WebDAV 地址')
    return
  }
  testing.value = true
  try {
    const message = await invoke<string>('webdav:test', {
      url: form.url.trim(),
      username: form.username.trim(),
      password: passwordTouched.value ? form.password : null,
      folder: form.folder.trim() || 'Woo',
    })
    showToast(message)
  } catch (error: any) {
    showToast(error?.message || '连接失败')
  } finally {
    testing.value = false
  }
}

async function pairWithMac() {
  if (pairing.value) return
  const code = pairCode.value.trim()
  if (!/^\d{6}$/.test(code)) {
    showToast('请输入 6 位数字配对码')
    return
  }
  pairing.value = true
  try {
    const remote = await invoke<LanPairResult>('lanPair:discover', { code })
    form.url = remote.url
    form.username = remote.username
    form.password = remote.password
    form.folder = remote.folder || 'Woo'
    form.enabled = true
    passwordTouched.value = true
    saving.value = true
    try {
      const config = await invoke<WebdavConfigView>('webdav:saveConfig', buildPayload())
      saved.value = config
      form.enabled = config.enabled
      form.password = ''
      passwordTouched.value = false
      pairCode.value = ''
      await syncStore.refreshStatus()
      showToast('配对成功，已开始同步')
    } finally {
      saving.value = false
    }
  } catch (error: any) {
    showToast(error?.message || '未找到电脑，请确认同一 Wi-Fi 且 Mac 已启用局域网')
  } finally {
    pairing.value = false
  }
}

async function saveConfig() {
  if (saving.value) return
  if (!canSave.value) {
    showToast(form.enabled ? '启用前请填写地址和用户名' : '请填写 WebDAV 地址')
    return
  }
  saving.value = true
  try {
    const config = await invoke<WebdavConfigView>('webdav:saveConfig', buildPayload())
    saved.value = config
    form.enabled = config.enabled
    form.password = ''
    passwordTouched.value = false
    await syncStore.refreshStatus()
    showToast(config.enabled ? '已启用 WebDAV 同步' : '已保存，同步未启用')
  } catch (error: any) {
    showToast(error?.message || '保存失败')
  } finally {
    saving.value = false
  }
}

async function syncNow() {
  if (syncStore.isSyncing) return
  const ok = await syncStore.triggerSync()
  showToast(ok ? '同步完成' : (syncStore.errorMsg || '同步失败'))
}

async function openJianguoyun() {
  const url = 'https://www.jianguoyun.com/#/safe'
  try {
    const { openUrl } = await import('@tauri-apps/plugin-opener')
    await openUrl(url)
  } catch {
    window.open(url, '_blank', 'noopener')
  }
}

function goBack() {
  if (window.history.length > 1) router.back()
  else router.replace('/settings')
}

onMounted(async () => {
  await syncStore.refreshStatus()
  await loadConfig()
})
</script>

<template>
  <div class="webdav-page">
    <header class="w-head webdav-head">
      <div class="w-head__row">
        <button type="button" class="w-icon-btn" aria-label="返回" @click="goBack">
          <van-icon name="arrow-left" />
        </button>
        <div class="w-head__titles">
          <span class="w-head__kicker">同步</span>
          <h1 class="w-head__title">WebDAV</h1>
        </div>
      </div>
    </header>

    <van-loading v-if="loading" class="webdav-loading" />

    <template v-else>
      <van-cell-group inset title="电脑配对">
        <van-field
          v-model="pairCode"
          label="配对码"
          placeholder="Mac 设置里显示的 6 位数字"
          type="digit"
          maxlength="6"
          autocomplete="off"
        />
        <van-cell
          title="配对并连接"
          :label="pairing ? '正在查找电脑…' : '手机与 Mac 连同一个 Wi-Fi 即可'"
          is-link
          @click="pairWithMac"
        >
          <template #value>
            <van-loading v-if="pairing" size="16" />
          </template>
        </van-cell>
      </van-cell-group>

      <van-cell-group inset title="连接">
        <van-field
          v-model="form.url"
          label="地址"
          placeholder="https://dav.example.com/dav"
          autocapitalize="off"
          autocomplete="off"
          spellcheck="false"
        />
        <van-field
          v-model="form.username"
          label="用户名"
          placeholder="用户名或邮箱"
          autocapitalize="off"
          autocomplete="username"
          spellcheck="false"
        />
        <van-field
          v-model="form.password"
          type="password"
          label="密码"
          :placeholder="passwordHint"
          autocomplete="new-password"
          @update:model-value="onPasswordInput"
        />
        <van-field
          v-model="form.folder"
          label="同步目录"
          placeholder="Woo"
          autocapitalize="off"
          autocomplete="off"
          spellcheck="false"
        />
        <van-cell title="启用 WebDAV 同步">
          <template #right-icon>
            <van-switch v-model="form.enabled" size="22px" active-color="var(--w-accent)" />
          </template>
        </van-cell>
      </van-cell-group>

      <div class="webdav-hint-row">
        <button type="button" class="webdav-text-btn" @click="fillJianguoyunPreset">填入坚果云示例</button>
        <button type="button" class="webdav-text-btn" @click="openJianguoyun">坚果云：获取应用密码</button>
      </div>

      <div v-if="bindingMismatch" class="webdav-warning">
        这组 WebDAV 配置绑定在另一个账号或本地库上。当前账号下不会自动同步，
        点「保存」即可重新绑定到当前账号。
      </div>

      <div class="webdav-actions">
        <button type="button" class="webdav-btn" :disabled="testing" @click="testConnection">
          {{ testing ? '测试中…' : '测试连接' }}
        </button>
        <button type="button" class="webdav-btn webdav-btn--primary" :disabled="saving || !canSave" @click="saveConfig">
          {{ saving ? '保存中…' : '保存' }}
        </button>
      </div>

      <van-cell-group inset title="状态">
        <van-cell title="当前方式" :value="statusText" />
        <van-cell title="上次同步">
          <template #value>
            <span>{{ syncStore.lastSyncLabel }}</span>
            <van-tag v-if="syncStore.pendingChanges > 0" type="warning" class="webdav-pending">
              待同步 {{ syncStore.pendingChanges }}
            </van-tag>
          </template>
        </van-cell>
        <van-cell
          title="立即同步"
          is-link
          :label="syncStore.backend === 'off' ? '配置 WebDAV 后可用' : ''"
          @click="syncNow"
        >
          <template #value>
            <van-loading v-if="syncStore.isSyncing" size="16" />
          </template>
        </van-cell>
      </van-cell-group>

    </template>
  </div>
</template>

<style scoped>
.webdav-page {
  min-height: 100vh;
  min-height: 100dvh;
  padding-bottom: calc(28px + env(safe-area-inset-bottom, 0px));
  background: var(--w-paper);
}

.webdav-head .w-head__titles {
  padding-left: 2px;
}

.webdav-head .w-icon-btn {
  margin-left: -4px;
}

.webdav-loading {
  display: flex;
  justify-content: center;
  padding: 64px 0;
}

.webdav-page :deep(.van-field__label) {
  color: var(--w-ink-2);
  font-size: 13px;
}

.webdav-page :deep(.van-field__control) {
  font-size: 16px;
}

.webdav-page :deep(.van-field__control::placeholder) {
  color: var(--w-ink-3);
  font-size: 12.5px;
}

.webdav-hint-row {
  display: flex;
  justify-content: space-between;
  padding: 10px 26px 0;
}

.webdav-text-btn {
  padding: 4px 0;
  border: 0;
  background: transparent;
  color: var(--w-accent);
  font-size: 12px;
  font-weight: 600;
}

.webdav-actions {
  display: flex;
  gap: 10px;
  margin: 14px 12px 0;
}

.webdav-btn {
  min-height: 44px;
  flex: 1;
  border: 1px solid var(--w-line-strong);
  border-radius: 12px;
  background: var(--w-canvas);
  color: var(--w-ink-2);
  font-size: 14px;
  font-weight: 600;
}

.webdav-btn--primary {
  border-color: transparent;
  background: var(--w-accent);
  color: #fff;
}

.webdav-btn:disabled {
  opacity: 0.45;
}

.webdav-pending {
  margin-left: 8px;
}

.webdav-warning {
  margin: 14px 18px 0;
  padding: 11px 14px;
  border-radius: 12px;
  background: color-mix(in srgb, #d97706 9%, var(--w-canvas));
  color: var(--w-ink-2);
  font-size: 11.5px;
  line-height: 1.65;
}

</style>
