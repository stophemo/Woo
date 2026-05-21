<template>
  <Teleport to="body">
    <div v-if="visible" class="settings-overlay" @click.self="$emit('close')" @keydown.esc="$emit('close')">
      <div class="settings-dialog">
        <div class="settings-header">
          <h2>{{ mode === 'ai' ? '模型配置' : '设置' }}</h2>
          <button class="settings-close-btn" @click="$emit('close')">
            <IconClose />
          </button>
        </div>
        <div class="settings-body">
          <div v-if="mode === 'ai'" class="settings-section">
            <h3>AI 配置</h3>
            <div class="settings-field">
              <label>DeepSeek API Key</label>
              <div class="api-key-input-row">
                <input :type="showKey ? 'text' : 'password'" v-model="apiKeyInput" placeholder="输入你的 DeepSeek API Key" class="settings-input" />
                <button class="toggle-visibility-btn" @click="showKey = !showKey">{{ showKey ? '隐藏' : '显示' }}</button>
              </div>
            </div>

            <div class="settings-field">
              <label>DeepSeek Base URL</label>
              <input type="text" v-model="baseUrlInput" placeholder="https://api.deepseek.com" class="settings-input" />
              <p class="settings-help">默认值: https://api.deepseek.com</p>
            </div>

            <div class="settings-field">
              <label>模型说明</label>
              <p class="settings-help">`deepseek v4 pro` 映射为 <code>deepseek-reasoner</code>；`deepseek v4 flash` 映射为 <code>deepseek-chat</code>（基于当前可用官方模型参数）。</p>
            </div>

            <div class="api-key-actions">
              <button class="settings-btn validate-btn" @click="handleValidate" :disabled="!apiKeyInput.trim() || validating">
                {{ validating ? '验证中...' : '验证' }}
              </button>
              <button class="settings-btn save-btn" @click="handleSave" :disabled="!apiKeyInput.trim()">保存</button>
              <span v-if="saveSuccess" class="validation-status success">已保存</span>
              <span v-else-if="validationResult === 'success'" class="validation-status success">有效</span>
              <span v-else-if="validationResult === 'fail'" class="validation-status fail">无效</span>
            </div>

            <p class="settings-help">
              获取 API Key：<a href="#" @click.prevent="openApiKeyLink">DeepSeek Platform</a>
            </p>
          </div>

          <div v-if="mode === 'file'" class="settings-section">
            <h3>静态资源链接</h3>
            <div class="settings-field">
              <label>Provider</label>
              <input type="text" v-model="assetProviderInput" placeholder="如 qiniu / s3 / r2 / aliyun-oss" class="settings-input" />
            </div>
            <div class="settings-field">
              <label>Base URL</label>
              <input type="text" v-model="assetBaseUrlInput" placeholder="https://cdn.example.com" class="settings-input" />
            </div>
            <div class="settings-field">
              <label>Path Prefix (可选)</label>
              <input type="text" v-model="assetPathPrefixInput" placeholder="uploads/images" class="settings-input" />
            </div>
            <p class="settings-help">编辑区插入图片时，若输入相对路径将自动拼接为 `baseUrl/pathPrefix/relativePath`。</p>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import IconClose from '../icons/IconClose.vue'
import { useAiChatStore } from '../../stores/aiChat'
import { validateApiKey } from '../../services/deepseek'
import { getAssetLinkSettings, saveAssetLinkSettings } from '../../services/assetLink'

interface Props {
  visible: boolean
  mode?: 'file' | 'ai'
}

const props = withDefaults(defineProps<Props>(), {
  mode: 'file'
})
const emit = defineEmits<{ close: [] }>()

const store = useAiChatStore()
const apiKeyInput = ref('')
const baseUrlInput = ref('https://api.deepseek.com')
const showKey = ref(false)
const validating = ref(false)
const validationResult = ref<'success' | 'fail' | null>(null)
const saveSuccess = ref(false)

const assetProviderInput = ref('custom')
const assetBaseUrlInput = ref('')
const assetPathPrefixInput = ref('')

watch(() => props.visible, (val) => {
  if (val) {
    if (props.mode === 'ai') {
      apiKeyInput.value = store.getApiKey()
      baseUrlInput.value = store.getBaseUrl()
      validationResult.value = null
      saveSuccess.value = false
    } else {
      const asset = getAssetLinkSettings()
      assetProviderInput.value = asset.provider
      assetBaseUrlInput.value = asset.baseUrl
      assetPathPrefixInput.value = asset.pathPrefix
    }
  }
})

async function handleValidate() {
  validating.value = true
  validationResult.value = null
  try {
    const valid = await validateApiKey(apiKeyInput.value.trim(), baseUrlInput.value.trim())
    validationResult.value = valid ? 'success' : 'fail'
  } catch {
    validationResult.value = 'fail'
  } finally {
    validating.value = false
  }
}

function handleSave() {
  if (props.mode === 'ai') {
    store.saveApiKey(apiKeyInput.value.trim())
    store.saveBaseUrl(baseUrlInput.value.trim())
  } else {
    saveAssetLinkSettings({
      provider: assetProviderInput.value.trim() || 'custom',
      baseUrl: assetBaseUrlInput.value.trim(),
      pathPrefix: assetPathPrefixInput.value.trim()
    })
  }
  saveSuccess.value = true
  emit('close')
}

function openApiKeyLink() {
  if (window.electronAPI) {
    window.electronAPI.openExternalLink('https://platform.deepseek.com/api_keys')
  } else {
    window.open('https://platform.deepseek.com/api_keys', '_blank')
  }
}
</script>

<style scoped>
.settings-overlay { position: fixed; top: 0; left: 0; right: 0; bottom: 0; background-color: rgba(0, 0, 0, 0.4); z-index: 9999; display: flex; align-items: center; justify-content: center; }
.settings-dialog { background-color: var(--bg-surface); border: 1px solid var(--border-primary); border-radius: 8px; width: 520px; max-height: 80vh; overflow-y: auto; box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2); }
.settings-header { display: flex; align-items: center; justify-content: space-between; padding: 16px 20px; border-bottom: 1px solid var(--border-secondary); }
.settings-header h2 { font-size: 16px; font-weight: 600; color: var(--text-primary); margin: 0; }
.settings-close-btn { background: transparent; border: none; cursor: pointer; padding: 4px; border-radius: 4px; display: flex; align-items: center; color: var(--text-secondary); transition: all 0.2s; }
.settings-close-btn:hover { background-color: var(--bg-hover); color: var(--text-primary); }
.settings-body { padding: 20px; }
.settings-section { margin-bottom: 18px; }
.settings-section h3 { font-size: 14px; font-weight: 600; color: var(--text-primary); margin: 0 0 12px 0; }
.settings-field { display: flex; flex-direction: column; gap: 8px; margin-bottom: 12px; }
.settings-field label { font-size: 13px; color: var(--text-secondary); }
.api-key-input-row { display: flex; gap: 8px; }
.settings-input { flex: 1; padding: 8px 12px; border: 1px solid var(--border-primary); border-radius: 4px; font-size: 13px; background-color: var(--bg-elevated); color: var(--text-primary); outline: none; transition: border-color 0.2s; }
.settings-input:focus { border-color: var(--accent); }
.toggle-visibility-btn { padding: 8px 12px; border: 1px solid var(--border-primary); border-radius: 4px; background-color: var(--bg-elevated); color: var(--text-secondary); cursor: pointer; font-size: 12px; white-space: nowrap; transition: all 0.2s; }
.toggle-visibility-btn:hover { background-color: var(--bg-hover); color: var(--text-primary); }
.api-key-actions { display: flex; align-items: center; gap: 8px; margin-top: 8px; }
.settings-btn { padding: 6px 16px; border: none; border-radius: 4px; cursor: pointer; font-size: 13px; transition: all 0.2s; }
.settings-btn:disabled { opacity: 0.5; cursor: not-allowed; }
.validate-btn { background-color: var(--bg-hover); color: var(--text-primary); }
.validate-btn:hover:not(:disabled) { background-color: var(--bg-active); }
.save-btn { background-color: var(--accent); color: #ffffff; }
.save-btn:hover:not(:disabled) { background-color: var(--accent-hover); }
.validation-status { font-size: 13px; font-weight: 500; }
.validation-status.success { color: #4caf50; }
.validation-status.fail { color: #e53935; }
.settings-help { font-size: 12px; color: var(--text-muted); margin: 0; }
.settings-help a { color: var(--accent); text-decoration: none; }
.settings-help a:hover { text-decoration: underline; }
</style>
