<template>
  <div class="md-vault-section">
    <h3>本地 Markdown</h3>
    <p class="md-vault-help">
      选择一个本地文件夹，Woo 会为其中的 <code>.md</code> / <code>.txt</code> 建立块索引。
      0.9.0 先提供索引和旧笔记迁移，跨设备同步仍走上面的 WebDAV 空间。
    </p>

    <div class="md-vault-box">
      <div class="md-vault-row">
        <span class="md-vault-label">文件夹</span>
        <code class="md-vault-path">{{ root || '尚未选择' }}</code>
      </div>
      <div class="md-vault-actions">
        <button class="md-vault-btn" :disabled="busy" @click="pickRoot">选择文件夹</button>
        <button class="md-vault-btn primary" :disabled="busy || !root" @click="scan">
          {{ busy ? '处理中…' : '扫描索引' }}
        </button>
      </div>
      <p v-if="notice" class="md-vault-notice">{{ notice }}</p>
      <p v-if="error" class="md-vault-error">{{ error }}</p>
    </div>

    <div class="md-vault-box">
      <h4>迁移旧笔记</h4>
      <p class="md-vault-help">
        把当前数据库里的 HTML 笔记导出为 Markdown 文件。原数据库不会被修改。
      </p>
      <div class="md-vault-actions">
        <button class="md-vault-btn" :disabled="busy" @click="pickMigrateTarget">选择目标文件夹</button>
        <button class="md-vault-btn" :disabled="busy || !migrateTarget" @click="previewMigration">
          预览
        </button>
        <button
          class="md-vault-btn danger"
          :disabled="busy || !migrateTarget || !preview || preview.total === 0"
          @click="runMigration"
        >
          开始迁移
        </button>
      </div>
      <p v-if="migrateTarget" class="md-vault-path">目标：{{ migrateTarget }}</p>
      <div v-if="preview" class="md-vault-preview">
        <p>
          共 {{ preview.total }} 篇，可写入 {{ preview.willWrite }} 篇，冲突
          {{ preview.conflicts }} 篇，警告 {{ preview.warnings }} 条。
        </p>
        <ul>
          <li v-for="item in preview.items.slice(0, 8)" :key="item.path">
            {{ item.path }}
            <span v-if="item.warning" class="md-vault-warning">（{{ item.warning }}）</span>
          </li>
        </ul>
        <p v-if="preview.items.length > 8" class="md-vault-help">
          仅显示前 8 条，共 {{ preview.items.length }} 条。
        </p>
      </div>
      <p v-if="migrateNotice" class="md-vault-notice">{{ migrateNotice }}</p>
      <p v-if="migrateError" class="md-vault-error">{{ migrateError }}</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '../../services/api'

interface ScanSummary {
  root: string
  files: number
  blocks: number
  changed: number
  removed: number
  skipped: number
}

interface MigrateItem {
  title: string
  path: string
  status: string
  warning?: string | null
}

interface MigratePreview {
  total: number
  willWrite: number
  conflicts: number
  warnings: number
  items: MigrateItem[]
}

interface MigrateReport {
  written: number
  skipped: number
  failed: number
  errors: string[]
}

const root = ref('')
const migrateTarget = ref('')
const preview = ref<MigratePreview | null>(null)
const busy = ref(false)
const notice = ref('')
const error = ref('')
const migrateNotice = ref('')
const migrateError = ref('')

async function pickRoot() {
  const picked = await open({ directory: true, multiple: false, title: '选择 Markdown 文件夹' })
  if (typeof picked === 'string') {
    root.value = picked
    notice.value = ''
    error.value = ''
  }
}

async function scan() {
  if (!root.value || busy.value) return
  busy.value = true
  notice.value = ''
  error.value = ''
  try {
    const summary = await invoke<ScanSummary>('markdown-vault-scan', { root: root.value })
    notice.value = `索引完成：${summary.files} 个文件，${summary.blocks} 个块，更新 ${summary.changed}，移除 ${summary.removed}。`
  } catch (err: any) {
    error.value = err?.message || '索引失败'
  } finally {
    busy.value = false
  }
}

async function pickMigrateTarget() {
  const picked = await open({ directory: true, multiple: false, title: '选择迁移目标文件夹' })
  if (typeof picked === 'string') {
    migrateTarget.value = picked
    preview.value = null
    migrateNotice.value = ''
    migrateError.value = ''
  }
}

async function previewMigration() {
  if (!migrateTarget.value || busy.value) return
  busy.value = true
  migrateNotice.value = ''
  migrateError.value = ''
  try {
    preview.value = await invoke<MigratePreview>('markdown-vault-migrate-preview', {
      target: migrateTarget.value,
    })
  } catch (err: any) {
    migrateError.value = err?.message || '预览失败'
  } finally {
    busy.value = false
  }
}

async function runMigration() {
  if (!migrateTarget.value || busy.value) return
  if (!window.confirm('将把数据库中的笔记写入目标文件夹，已存在的文件会跳过。继续吗？')) return
  busy.value = true
  migrateNotice.value = ''
  migrateError.value = ''
  try {
    const report = await invoke<MigrateReport>('markdown-vault-migrate', {
      target: migrateTarget.value,
    })
    migrateNotice.value = `迁移完成：写入 ${report.written}，跳过 ${report.skipped}，失败 ${report.failed}。`
    if (report.errors.length > 0) {
      migrateError.value = report.errors.slice(0, 3).join('；')
    }
  } catch (err: any) {
    migrateError.value = err?.message || '迁移失败'
  } finally {
    busy.value = false
  }
}
</script>

<style scoped>
.md-vault-section { display: flex; flex-direction: column; gap: 12px; }
.md-vault-section h3 { margin: 0; font-size: 15px; }
.md-vault-section h4 { margin: 0 0 8px; font-size: 13px; }
.md-vault-help { margin: 0; color: var(--text-secondary, #8a8a8a); font-size: 12px; line-height: 1.6; }
.md-vault-help code { font-size: 11px; }
.md-vault-box { border: 1px solid var(--border-color, #e5e5e5); border-radius: 12px; padding: 14px; display: flex; flex-direction: column; gap: 10px; background: color-mix(in srgb, var(--bg-secondary, #fff) 72%, transparent); }
.md-vault-row { display: flex; align-items: baseline; gap: 10px; min-width: 0; }
.md-vault-label { flex: 0 0 auto; color: var(--text-secondary, #8a8a8a); font-size: 12px; }
.md-vault-path { min-width: 0; overflow-wrap: anywhere; color: var(--text-primary); font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; }
.md-vault-actions { display: flex; flex-wrap: wrap; gap: 8px; }
.md-vault-btn { border: 1px solid var(--border-color, #d9d9d9); background: var(--bg-secondary, #fff); color: inherit; border-radius: 8px; padding: 7px 12px; font-size: 12px; cursor: pointer; }
.md-vault-btn.primary { background: var(--accent-color, #2f6fed); border-color: transparent; color: #fff; }
.md-vault-btn.danger { color: #c0392b; }
.md-vault-btn:disabled { opacity: .55; cursor: not-allowed; }
.md-vault-notice { margin: 0; color: #1d7a46; font-size: 12px; }
.md-vault-error { margin: 0; color: #c0392b; font-size: 12px; }
.md-vault-preview { display: flex; flex-direction: column; gap: 6px; font-size: 12px; }
.md-vault-preview p { margin: 0; }
.md-vault-preview ul { margin: 0; padding-left: 18px; color: var(--text-secondary, #8a8a8a); }
.md-vault-warning { color: #b26a00; }
</style>
