<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { showToast, showConfirmDialog } from 'vant'
import { useWorkspaceStore } from '../../src/stores/workspace'
import { useLockStore } from '../../src/stores/lock'
import { markdownToEditorHtml } from '../../src/services/markdown'
import MobileRichEditor from '../components/MobileRichEditor.vue'

const route = useRoute()
const router = useRouter()
const store = useWorkspaceStore()
const lockStore = useLockStore()
const noteId = route.params.id as string

const loading = ref(true)
const saving = ref(false)
const editing = ref(false)
const editContent = ref('')
const editOriginal = ref('')

// 统一从 store 读取当前文档（草稿走 localStorage，正式文档走后端）
const doc = computed(() => store.currentDocument)
const title = computed(() => doc.value?.title || '无标题')
const content = computed(() => doc.value?.content || '')
const isLocked = computed(() => !!doc.value?.isLocked)
const isDraft = noteId.startsWith('draft_')

function openVersions() {
  router.push(`/versions/${noteId}`)
}

// ------- 分享/导出 -------
const showShare = ref(false)
const shareActions = [
  { name: '分享为 Markdown', key: 'markdown' },
  { name: '分享为纯文本', key: 'txt' },
]

async function shareAs(action: { key: string }) {
  showShare.value = false
  const html = content.value
  if (!html.trim()) { showToast('内容为空'); return }
  let text = ''
  if (action.key === 'markdown') {
    try {
      const { default: TurndownService } = await import('turndown')
      const { gfm } = await import('turndown-plugin-gfm')
      const td = new TurndownService({ headingStyle: 'atx', codeBlockStyle: 'fenced' })
      td.use(gfm)
      text = td.turndown(html)
    } catch { text = html }
  } else {
    const tmp = document.createElement('div')
    tmp.innerHTML = html
    tmp.querySelectorAll('br').forEach((br) => br.replaceWith('\n'))
    tmp.querySelectorAll('p,div,h1,h2,h3,h4,h5,h6,li,tr,blockquote,pre').forEach((el) => el.append('\n'))
    text = (tmp.textContent || '').replace(/\n{3,}/g, '\n\n').trim()
  }
  try {
    if (navigator.share) {
      await navigator.share({ title: title.value, text })
    } else {
      await navigator.clipboard.writeText(text)
      showToast('已复制到剪贴板')
    }
  } catch (e: any) {
    if (e?.name === 'AbortError') return // 用户取消分享
    try {
      await navigator.clipboard.writeText(text)
      showToast('已复制到剪贴板')
    } catch {
      showToast('分享失败')
    }
  }
}

// 解锁
const unlockPwd = ref('')
const unlocking = ref(false)
async function doUnlock() {
  if (!unlockPwd.value) return
  unlocking.value = true
  try {
    const ok = await lockStore.verify(unlockPwd.value)
    if (!ok) { showToast('密码错误'); return }
    await lockStore.unlockDocument(noteId)
    await store.selectDocument(noteId) // 重新加载，已解锁后含正文
    unlockPwd.value = ''
    showToast('已解锁')
  } finally {
    unlocking.value = false
  }
}

onMounted(async () => {
  try {
    await store.selectDocument(noteId)
  } catch {
    showToast('加载失败')
  } finally {
    loading.value = false
  }
})

function startEdit() {
  const source = content.value || ''
  if (!source.trim()) {
    editContent.value = ''
  } else if (/<(p|h[1-6]|table|ul|ol|blockquote|pre|div|br|hr|img|a|strong|em|code)\b[^>]*>/i.test(source)) {
    editContent.value = source
  } else {
    try { editContent.value = markdownToEditorHtml(source) } catch { editContent.value = source }
  }
  editOriginal.value = editContent.value
  editing.value = true
}

function cancelEdit() {
  editing.value = false
  editContent.value = ''
  editOriginal.value = ''
}

async function saveEdit() {
  saving.value = true
  try {
    // 移动端与桌面端统一保存 Tiptap HTML，表格和图片不会被降级成纯文本。
    store.updateDocumentContent(noteId, editContent.value)
    await store.flushPendingSave(noteId)
    // 正式文档保存后提交一个版本，便于历史回溯（草稿不产生版本）
    if (!isDraft) {
      try { await store.commitDocumentVersion(noteId, 'auto') } catch { /* 忽略 */ }
    }
    editing.value = false
    showToast('已保存')
  } catch {
    showToast('保存失败')
  } finally {
    saving.value = false
  }
}

async function confirmDelete() {
  try {
    await showConfirmDialog({ title: '删除笔记', message: '确定删除此笔记？' })
    await store.deleteDocument(noteId)
    showToast('已删除')
    router.back()
  } catch {
    // cancelled
  }
}

function goBack() {
  if (editing.value && editContent.value !== editOriginal.value) {
    showConfirmDialog({ title: '放弃编辑？', message: '未保存的更改将丢失' })
      .then(() => router.back())
      .catch(() => {})
    return
  }
  router.back()
}
</script>

<template>
  <div class="editor-page" :class="{ 'is-editing': editing }">
    <van-nav-bar
      :title="title"
      left-text="返回"
      left-arrow
      @click-left="goBack"
    >
      <template #right>
        <button
          v-if="!isLocked"
          type="button"
          class="nav-action-button"
          title="分享"
          aria-label="分享"
          @click="showShare = true"
        >
          <van-icon name="share-o" />
        </button>
        <button
          v-if="!isDraft && !isLocked"
          type="button"
          class="nav-action-button"
          title="版本历史"
          aria-label="版本历史"
          @click="openVersions"
        >
          <van-icon name="clock-o" />
        </button>
        <button
          type="button"
          class="nav-action-button is-danger"
          title="删除文稿"
          aria-label="删除文稿"
          @click="confirmDelete"
        >
          <van-icon name="delete-o" />
        </button>
      </template>
    </van-nav-bar>

    <van-loading v-if="loading" class="mobile-loading" />

    <!-- 加密遮罩 -->
    <template v-else-if="isLocked">
      <div class="locked-box">
        <span class="locked-icon"><van-icon name="lock" /></span>
        <h2 class="locked-title">此文稿已加密</h2>
        <p class="locked-copy">输入密码后可继续阅读和编辑</p>
        <van-field v-model="unlockPwd" type="password" placeholder="输入密码锁密码" class="locked-field" />
        <van-button type="primary" class="unlock-button" :loading="unlocking" @click="doUnlock">解锁查看</van-button>
      </div>
    </template>

    <!-- 阅读模式 -->
    <template v-else-if="!editing">
      <article class="editor-content" v-html="content" />
      <div v-if="!content" class="empty-hint">暂无内容</div>
      <button
        type="button"
        class="edit-btn"
        title="编辑文稿"
        aria-label="编辑文稿"
        @click="startEdit"
      >
        <van-icon name="edit" />
      </button>
    </template>

    <!-- 编辑模式：移动端触控富文本编辑器，与桌面端共用 HTML 内容模型 -->
    <template v-else>
      <div class="editor-workbench">
        <MobileRichEditor v-model="editContent" @shortcut-save="saveEdit" />
      </div>
      <div class="edit-actions safe-area-bottom">
        <van-button class="cancel-button" @click="cancelEdit">取消</van-button>
        <van-button
          type="primary"
          class="save-button"
          :loading="saving"
          @click="saveEdit"
        >
          保存
        </van-button>
      </div>
    </template>

    <!-- 分享/导出 -->
    <van-action-sheet
      v-model:show="showShare"
      :actions="shareActions"
      cancel-text="取消"
      close-on-click-action
      @select="shareAs"
    />
  </div>
</template>

<style scoped>
.editor-page {
  min-height: 100vh;
  min-height: 100dvh;
  background: var(--van-background-2);
}

.editor-page.is-editing {
  background: var(--van-background);
}

.nav-action-button {
  display: inline-flex;
  width: 32px;
  height: 32px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--van-text-color-2);
  vertical-align: middle;
}

.nav-action-button:active {
  background: color-mix(in srgb, var(--van-primary-color) 10%, transparent);
  color: var(--van-primary-color);
}

.nav-action-button.is-danger:active {
  background: color-mix(in srgb, var(--van-danger-color) 9%, transparent);
  color: var(--van-danger-color);
}

.nav-action-button .van-icon {
  font-size: 18px;
}

.locked-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  width: min(100%, 420px);
  margin: 0 auto;
  padding: 72px 28px 32px;
}

.locked-icon {
  display: flex;
  width: 52px;
  height: 52px;
  align-items: center;
  justify-content: center;
  border: 1px solid color-mix(in srgb, #d3a32c 28%, var(--van-border-color));
  border-radius: 8px;
  background: color-mix(in srgb, #d3a32c 10%, var(--van-background-2));
  color: #d3a32c;
}

.locked-icon .van-icon {
  font-size: 24px;
}

.locked-title {
  margin: 18px 0 0;
  color: var(--van-text-color);
  font-size: 18px;
  line-height: 1.4;
}

.locked-copy {
  margin: 6px 0 20px;
  color: var(--van-text-color-2);
  font-size: 12px;
}

.locked-field {
  width: 100%;
  border: 1px solid var(--van-border-color);
  border-radius: 7px;
  background: var(--van-background-2);
}

.unlock-button {
  width: 100%;
  height: 42px;
  margin-top: 12px;
  border-radius: 7px;
}

.editor-content {
  width: min(100%, 680px);
  min-height: calc(100vh - 55px - env(safe-area-inset-top, 0px));
  margin: 0 auto;
  padding: 30px 22px 108px;
  color: var(--van-text-color);
  font-size: 16px;
  line-height: 1.78;
  overflow-wrap: break-word;
}

.editor-content :deep(h1),
.editor-content :deep(h2),
.editor-content :deep(h3),
.editor-content :deep(h4) {
  color: var(--van-text-color);
  letter-spacing: 0;
}

.editor-content :deep(h1) { margin: 0 0 20px; font-size: 28px; line-height: 1.35; }
.editor-content :deep(h2) { margin: 28px 0 10px; font-size: 21px; line-height: 1.4; }
.editor-content :deep(h3) { margin: 24px 0 8px; font-size: 18px; line-height: 1.45; }
.editor-content :deep(h4) { margin: 20px 0 6px; font-size: 16px; line-height: 1.5; }
.editor-content :deep(p) { margin: 0 0 15px; }
.editor-content :deep(ul),
.editor-content :deep(ol) { margin: 12px 0 18px; padding-left: 22px; }
.editor-content :deep(li) { margin: 5px 0; }
.editor-content :deep(blockquote) {
  margin: 20px 0;
  padding: 11px 14px;
  border-left: 3px solid var(--van-primary-color);
  background: color-mix(in srgb, var(--van-primary-color) 7%, var(--van-background));
  color: var(--van-text-color-2);
}
.editor-content :deep(blockquote p:last-child) { margin-bottom: 0; }
.editor-content :deep(pre) {
  overflow-x: auto;
  margin: 18px 0;
  padding: 14px;
  border: 1px solid var(--van-border-color);
  border-radius: 6px;
  background: var(--van-background);
  font-size: 13px;
  line-height: 1.6;
}
.editor-content :deep(code) {
  padding: 1px 4px;
  border-radius: 3px;
  background: var(--van-background);
  font-family: "SFMono-Regular", Consolas, monospace;
  font-size: 0.88em;
}
.editor-content :deep(pre code) { padding: 0; background: transparent; }
.editor-content :deep(a) { color: var(--van-primary-color); }
.editor-content :deep(img) { max-width: 100%; height: auto; border-radius: 6px; }
.editor-content :deep(hr) { margin: 28px 0; border: 0; border-top: 1px solid var(--van-border-color); }
.editor-content :deep(table) { width: 100%; margin: 18px 0; border-collapse: collapse; font-size: 13px; }
.editor-content :deep(th),
.editor-content :deep(td) { padding: 8px; border: 1px solid var(--van-border-color); text-align: left; }
.editor-content :deep(th) { background: var(--van-background); }
.editor-content :deep(p:last-child) { margin-bottom: 0; }

.empty-hint {
  text-align: center;
  color: var(--van-text-color-2);
  margin-top: 72px;
  font-size: 13px;
}

.edit-btn {
  position: fixed;
  right: 18px;
  bottom: calc(20px + env(safe-area-inset-bottom, 0px));
  z-index: 5;
  display: flex;
  width: 46px;
  height: 46px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: 8px;
  background: var(--van-primary-color);
  box-shadow: 0 9px 24px color-mix(in srgb, var(--van-primary-color) 26%, transparent);
  color: white;
}

.edit-btn:active {
  transform: scale(0.96);
}

.edit-btn .van-icon {
  font-size: 21px;
}

.editor-workbench {
  width: min(100%, 680px);
  min-height: calc(100vh - 128px - env(safe-area-inset-top, 0px));
  margin: 0 auto;
  background: var(--van-background-2);
}

.edit-actions {
  position: sticky;
  bottom: 0;
  z-index: 5;
  display: flex;
  gap: 10px;
  padding: 10px 14px calc(10px + env(safe-area-inset-bottom, 0px));
  border-top: 1px solid var(--van-border-color);
  background: color-mix(in srgb, var(--van-background-2) 95%, transparent);
  backdrop-filter: blur(18px);
}

.edit-actions .van-button {
  height: 42px;
  flex: 1;
  border-radius: 7px;
}

.cancel-button {
  background: var(--van-background-2);
  color: var(--van-text-color-2);
}
</style>
