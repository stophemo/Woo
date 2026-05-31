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

            <!-- 1. 选择供应商 -->
            <div class="settings-field">
              <label>供应商</label>
              <select v-model="providerInput" class="settings-input" @change="onProviderChange">
                <option value="deepseek">DeepSeek</option>
                <option value="gemini">Google Gemini</option>
                <option value="openai-compatible">OpenAI 兼容（Ollama 等）</option>
              </select>
            </div>

            <!-- 2. API Key（Gemini / DeepSeek / OpenAI 兼容显示不同 label） -->
            <div v-if="providerInput !== 'openai-compatible' || showKeyField" class="settings-field">
              <label>{{ apiKeyLabel }}</label>
              <div class="api-key-input-row">
                <input :type="showKey ? 'text' : 'password'" v-model="apiKeyInput" :placeholder="apiKeyPlaceholder" class="settings-input" />
                <button class="toggle-visibility-btn" @click="showKey = !showKey">{{ showKey ? '隐藏' : '显示' }}</button>
              </div>
            </div>

            <!-- 3. 获取模型 + 模型选择 -->
            <div class="settings-field">
              <label>
                模型
                <button class="settings-link-btn" @click="handleFetchModels" :disabled="fetchingModels" style="margin-left:8px;font-size:0.78rem;">
                  {{ fetchingModels ? '获取中...' : '获取模型' }}
                </button>
              </label>
              <div class="model-select-row">
                <select v-model="modelInput" class="settings-input" :disabled="filteredModels.length === 0">
                  <option v-if="filteredModels.length === 0" value="">暂无模型，请点击「获取模型」</option>
                  <option v-for="m in filteredModels" :key="m.id" :value="m.id">{{ m.name }}</option>
                </select>
              </div>
              <span v-if="fetchModelStatus" class="settings-help" :class="fetchModelOk ? 'validation-status success' : 'validation-status fail'">{{ fetchModelStatus }}</span>
              <input v-if="modelInput === '_custom_'" type="text" v-model="customModelInput" placeholder="输入模型名称（如 llama3.2, qwen2.5）" class="settings-input" style="margin-top:6px;" />
            </div>

            <!-- 4. Base URL（DeepSeek / OpenAI 兼容显示） -->
            <div v-if="providerInput !== 'gemini'" class="settings-field">
              <label>Base URL</label>
              <input type="text" v-model="baseUrlInput" :placeholder="baseUrlPlaceholder" class="settings-input" />
              <p class="settings-help">默认值: {{ baseUrlPlaceholder }}</p>
            </div>

            <!-- 5. 操作按钮 -->
            <div class="api-key-actions">
              <button class="settings-btn validate-btn" @click="handleValidate" :disabled="!apiKeyInput.trim() || validating">
                {{ validating ? '测试中...' : '测试连接' }}
              </button>
              <button class="settings-btn save-btn" @click="handleSave">保存</button>
              <span v-if="saveSuccess" class="validation-status success">已保存</span>
              <span v-else-if="validationResult === 'success'" class="validation-status success">连接正常</span>
              <span v-else-if="validationResult === 'fail'" class="validation-status fail">连接失败</span>
            </div>

            <p class="settings-help">
              获取 API Key：
              <a href="#" @click.prevent="openApiKeyLink">
                {{ providerInput === 'gemini' ? 'Google AI Studio' : providerInput === 'deepseek' ? 'DeepSeek Platform' : '参考文档' }}
              </a>
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
import { ref, watch, computed } from 'vue'
import IconClose from '../icons/IconClose.vue'
import { useAiChatStore } from '../../stores/aiChat'
import type { ProviderType } from '../../types/ai'
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

/* ========== AI 配置状态 ========== */
const providerInput = ref<ProviderType>('deepseek')
const apiKeyInput = ref('')
const baseUrlInput = ref('https://api.deepseek.com')
const modelInput = ref('')
const customModelInput = ref('')
const showKey = ref(false)
const showKeyField = ref(false)
const validating = ref(false)
const validationResult = ref<'success' | 'fail' | null>(null)
const saveSuccess = ref(false)
const fetchingModels = ref(false)
const fetchModelStatus = ref('')
const fetchModelOk = ref(false)

/** 模型列表（根据供应商过滤） */
const filteredModels = computed(() => {
  return store.availableModels.filter(m => m.provider === providerInput.value)
})

const apiKeyLabel = computed(() => {
  if (providerInput.value === 'gemini') return 'Gemini API Key'
  if (providerInput.value === 'openai-compatible') return 'API Key（Ollama 可留空）'
  return 'DeepSeek API Key'
})

const apiKeyPlaceholder = computed(() => {
  if (providerInput.value === 'gemini') return '输入你的 Gemini API Key'
  if (providerInput.value === 'openai-compatible') return '留空则使用 Ollama（无需认证）'
  return '输入你的 DeepSeek API Key'
})

const baseUrlPlaceholder = computed(() => {
  if (providerInput.value === 'openai-compatible') return 'http://localhost:11434'
  return 'https://api.deepseek.com'
})

/* ========== 资产链接配置状态 ========== */
const assetProviderInput = ref('custom')
const assetBaseUrlInput = ref('')
const assetPathPrefixInput = ref('')

watch(() => props.visible, (val) => {
  if (val) {
    if (props.mode === 'ai') {
      const settings = getRawSettings()
      providerInput.value = (settings.provider as ProviderType) || 'deepseek'
      apiKeyInput.value = store.getApiKey()
      modelInput.value = settings.selectedModelId || filteredModels.value[0]?.id || ''
      customModelInput.value = settings.customModelName || ''

      if (providerInput.value === 'gemini') {
        baseUrlInput.value = ''
      } else if (providerInput.value === 'openai-compatible') {
        baseUrlInput.value = settings.openaiBaseUrl || 'http://localhost:11434'
      } else {
        baseUrlInput.value = settings.deepseekBaseUrl || 'https://api.deepseek.com'
      }
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

function getRawSettings(): Record<string, string> {
  try {
    const raw = localStorage.getItem('ai-settings')
    return raw ? JSON.parse(raw) : {}
  } catch { return {} }
}

/** 切换供应商时更新 UI */
function onProviderChange() {
  const models = store.availableModels.filter(m => m.provider === providerInput.value)
  if (models.length > 0) modelInput.value = models[0].id

  if (providerInput.value === 'gemini') {
    baseUrlInput.value = ''
  } else if (providerInput.value === 'openai-compatible') {
    baseUrlInput.value = 'http://localhost:11434'
  } else {
    baseUrlInput.value = 'https://api.deepseek.com'
  }

  const s = getRawSettings()
  if (providerInput.value === 'gemini') apiKeyInput.value = s.geminiApiKey || ''
  else if (providerInput.value === 'openai-compatible') apiKeyInput.value = s.openaiApiKey || ''
  else apiKeyInput.value = s.deepseekApiKey || ''

  validationResult.value = null
}

async function handleValidate() {
  validating.value = true
  validationResult.value = null
  try {
    const result = await store.testConnection(apiKeyInput.value.trim(), baseUrlInput.value.trim() || undefined)
    validationResult.value = result.ok ? 'success' : 'fail'
  } catch {
    validationResult.value = 'fail'
  } finally {
    validating.value = false
  }
}

async function handleFetchModels() {
  fetchingModels.value = true
  fetchModelStatus.value = ''
  try {
    const result = await store.fetchAvailableModels(apiKeyInput.value.trim())
    fetchModelStatus.value = result.message
    fetchModelOk.value = result.ok
    if (result.ok) {
      // 自动选中第一个模型
      if (filteredModels.value.length > 0) {
        modelInput.value = filteredModels.value[0].id
      }
    }
  } catch (err: any) {
    fetchModelStatus.value = err.message || '获取失败'
    fetchModelOk.value = false
  } finally {
    fetchingModels.value = false
  }
}

function handleSave() {
  if (props.mode === 'ai') {
    const actualModel = modelInput.value === '_custom_' ? '' : modelInput.value
    store.saveFullSettings({
      provider: providerInput.value,
      deepseekApiKey: apiKeyInput.value.trim(),
      deepseekBaseUrl: providerInput.value === 'deepseek' ? baseUrlInput.value.trim() : undefined,
      geminiApiKey: apiKeyInput.value.trim(),
      openaiBaseUrl: providerInput.value === 'openai-compatible' ? baseUrlInput.value.trim() : undefined,
      openaiApiKey: apiKeyInput.value.trim(),
      modelId: actualModel,
      customModelName: customModelInput.value.trim()
    })
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
  let url = 'https://platform.deepseek.com/api_keys'
  if (providerInput.value === 'gemini') {
    url = 'https://aistudio.google.com/apikey'
  } else if (providerInput.value === 'openai-compatible') {
    url = 'https://ollama.ai'
  }
  if (window.electronAPI) {
    window.electronAPI.openExternalLink(url)
  } else {
    window.open(url, '_blank')
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
.settings-link-btn {
  background: none;
  border: 1px solid var(--border-primary);
  border-radius: 4px;
  padding: 1px 8px;
  cursor: pointer;
  color: var(--accent);
  font-size: 0.78rem;
  transition: background 0.2s;
  vertical-align: middle;
}
.settings-link-btn:hover:not(:disabled) { background: var(--bg-hover); }
.settings-link-btn:disabled { opacity: 0.5; cursor: not-allowed; }
.settings-help { font-size: 12px; color: var(--text-muted); margin: 0; }
.settings-help a { color: var(--accent); text-decoration: none; }
.settings-help a:hover { text-decoration: underline; }
</style>
