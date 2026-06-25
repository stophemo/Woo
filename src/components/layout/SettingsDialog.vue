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
          <div class="settings-footer">
            <button class="settings-btn save-btn" @click="handleSave">保存</button>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import IconClose from '../icons/IconClose.vue'
import { getAssetLinkSettings, saveAssetLinkSettings } from '../../services/assetLink'

interface Props {
  visible: boolean
}

const props = defineProps<Props>()
const emit = defineEmits<{ close: [] }>()

/* ========== 资产链接配置状态 ========== */
const assetProviderInput = ref('custom')
const assetBaseUrlInput = ref('')
const assetPathPrefixInput = ref('')

watch(() => props.visible, (val) => {
  if (val) {
    const asset = getAssetLinkSettings()
    assetProviderInput.value = asset.provider
    assetBaseUrlInput.value = asset.baseUrl
    assetPathPrefixInput.value = asset.pathPrefix
  }
})

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
.settings-field { display: flex; flex-direction: column; gap: 8px; margin-bottom: 12px; }
.settings-field label { font-size: 13px; color: var(--text-secondary); }
.settings-input { flex: 1; padding: 8px 12px; border: 1px solid var(--border-primary); border-radius: 4px; font-size: 13px; background-color: var(--bg-elevated); color: var(--text-primary); outline: none; transition: border-color 0.2s; }
.settings-input:focus { border-color: var(--accent); }
.settings-help { font-size: 12px; color: var(--text-muted); margin: 0; }
.settings-help a { color: var(--accent); text-decoration: none; }
.settings-help a:hover { text-decoration: underline; }
.settings-footer { display: flex; justify-content: flex-end; padding-top: 12px; border-top: 1px solid var(--border-secondary); margin-top: 16px; }
.settings-btn { padding: 6px 16px; border: none; border-radius: 4px; cursor: pointer; font-size: 13px; transition: all 0.2s; }
.save-btn { background-color: var(--accent); color: #ffffff; }
.save-btn:hover:not(:disabled) { background-color: var(--accent-hover); }

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
}
</style>
