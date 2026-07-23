<template>
  <Teleport to="body">
    <div v-if="visible" class="settings-overlay" @click.self="$emit('close')" @keydown.esc="$emit('close')">
      <div class="settings-dialog">
        <div class="settings-header">
          <h2>设置</h2>
          <button class="settings-close-btn" @click="$emit('close')">
            <IconClose />
          </button>
        </div>
        <div class="settings-body">
          <div class="settings-section appearance-section">
            <h3>外观</h3>
            <div class="theme-grid" role="radiogroup" aria-label="选择主题">
              <button
                v-for="option in themeStore.themeOptions"
                :key="option.id"
                type="button"
                class="theme-option"
                :class="{ selected: themeStore.theme === option.id }"
                role="radio"
                :aria-checked="themeStore.theme === option.id"
                @click="themeStore.theme = option.id"
              >
                <span class="theme-swatch" aria-hidden="true">
                  <span v-for="color in option.colors" :key="color" :style="{ backgroundColor: color }"></span>
                </span>
                <span class="theme-option-copy">
                  <strong>{{ option.label }}</strong>
                  <small>{{ option.description }}</small>
                </span>
                <span v-if="themeStore.theme === option.id" class="theme-selected-mark" aria-hidden="true">✓</span>
              </button>
            </div>
          </div>
          <div class="settings-section">
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
          <div class="settings-section about-section">
            <h3>关于</h3>
            <div class="about-row">
              <div>
                <div class="about-name">Woo 无我笔记</div>
                <div class="about-version">版本 {{ appVersion ? `v${appVersion}` : '读取中...' }}</div>
              </div>
              <button class="settings-btn secondary-btn" @click="emit('check-update')">检查更新</button>
            </div>
          </div>
          <div class="settings-footer">
            <button class="settings-btn save-btn" @click="handleSave">保存</button>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import IconClose from '../icons/IconClose.vue'
import { getAssetLinkSettings, saveAssetLinkSettings } from '../../services/assetLink'
import { useThemeStore } from '../../stores/theme'

interface Props {
  visible: boolean
  appVersion: string
}

defineProps<Props>()
const emit = defineEmits<{ close: []; 'check-update': [] }>()
const themeStore = useThemeStore()

/* ========== 资产链接配置状态 ========== */
const assetProviderInput = ref('custom')
const assetBaseUrlInput = ref('')
const assetPathPrefixInput = ref('')
function loadSettings() {
  const asset = getAssetLinkSettings()
  assetProviderInput.value = asset.provider
  assetBaseUrlInput.value = asset.baseUrl
  assetPathPrefixInput.value = asset.pathPrefix
}

loadSettings()

function handleSave() {
  saveAssetLinkSettings({
    provider: assetProviderInput.value.trim() || 'custom',
    baseUrl: assetBaseUrlInput.value.trim(),
    pathPrefix: assetPathPrefixInput.value.trim()
  })
  emit('close')
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
.theme-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
.theme-option { position: relative; display: flex; align-items: center; gap: 10px; min-width: 0; padding: 10px; border: 1px solid var(--border-primary); border-radius: 6px; background: var(--bg-elevated); color: var(--text-primary); text-align: left; cursor: pointer; transition: border-color 0.2s, background-color 0.2s, box-shadow 0.2s; }
.theme-option:hover { background: var(--bg-hover); }
.theme-option.selected { border-color: var(--accent); box-shadow: 0 0 0 2px var(--accent-light); }
.theme-swatch { display: flex; width: 34px; height: 34px; flex-shrink: 0; overflow: hidden; border: 1px solid var(--border-primary); border-radius: 5px; }
.theme-swatch span { flex: 1; }
.theme-option-copy { display: flex; min-width: 0; flex-direction: column; gap: 2px; }
.theme-option-copy strong { font-size: 13px; font-weight: 600; }
.theme-option-copy small { overflow: hidden; color: var(--text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
.theme-selected-mark { position: absolute; top: 6px; right: 8px; color: var(--accent); font-size: 14px; font-weight: 700; }
.settings-field { display: flex; flex-direction: column; gap: 8px; margin-bottom: 12px; }
.settings-field label { font-size: 13px; color: var(--text-secondary); }
.settings-input { flex: 1; padding: 8px 12px; border: 1px solid var(--border-primary); border-radius: 4px; font-size: 13px; background-color: var(--bg-elevated); color: var(--text-primary); outline: none; transition: border-color 0.2s; }
.settings-input:focus { border-color: var(--accent); }
.settings-help { font-size: 12px; color: var(--text-muted); margin: 0; }
.settings-help a { color: var(--accent); text-decoration: none; }
.settings-help a:hover { text-decoration: underline; }
.about-section { padding-top: 18px; border-top: 1px solid var(--border-secondary); }
.about-row { display: flex; align-items: center; justify-content: space-between; gap: 16px; flex-wrap: wrap; }
.about-name { color: var(--text-primary); font-size: 13px; font-weight: 600; }
.about-version { margin-top: 4px; color: var(--text-muted); font-size: 12px; }
.settings-footer { display: flex; justify-content: flex-end; padding-top: 12px; border-top: 1px solid var(--border-secondary); margin-top: 16px; }
.settings-btn { padding: 6px 16px; border: none; border-radius: 4px; cursor: pointer; font-size: 13px; transition: all 0.2s; }
.save-btn { background-color: var(--accent); color: #ffffff; }
.save-btn:hover:not(:disabled) { background-color: var(--accent-hover); }
.secondary-btn { border: 1px solid var(--border-primary); background: var(--bg-elevated); color: var(--text-primary); }
.secondary-btn:hover { background: var(--bg-hover); }

@media (max-width: 640px) {
  .settings-overlay {
    align-items: flex-end;
    padding: 0 0 env(safe-area-inset-bottom) 0;
  }
  .settings-dialog {
    width: 100%;
    max-width: 100%;
    max-height: 90vh;
    border-radius: 12px 12px 0 0;
    border-left: none;
    border-right: none;
    border-bottom: none;
  }
  .theme-grid { grid-template-columns: 1fr; }
}
</style>
