<script setup lang="ts">
/**
 * 移动端编辑器。
 *
 * 与旧版的关键差异：
 * - 不再有「阅读模式 / 编辑模式」两个状态：进入即可直接写，少一次点击。
 * - 不再有底部「取消 / 保存」按钮：输入即自动保存（900ms 防抖），
 *   顶部显示真实保存状态，离开页面前强制落盘。
 * - 版本提交沿用桌面端策略（30s 冷却），内容去重由后端 hash 完成。
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { showToast, showConfirmDialog } from 'vant'
import { useWorkspaceStore } from '../../src/stores/workspace'
import { useLockStore } from '../../src/stores/lock'
import MobileRichEditor from '../components/MobileRichEditor.vue'
import FolderPicker from '../components/FolderPicker.vue'

const route = useRoute()
const router = useRouter()
const store = useWorkspaceStore()
const lockStore = useLockStore()

const noteId = route.params.id as string
const isDraft = noteId.startsWith('draft_')

const loading = ref(true)
const html = ref('')
const saveState = ref<'idle' | 'pending' | 'saving' | 'saved' | 'error'>('idle')
const showMore = ref(false)

const showFolderPicker = ref(false)

const document = computed(() => store.currentDocument)
const isLocked = computed(() => !!document.value?.isLocked)
const savedLabel = computed(() => {
  switch (saveState.value) {
    case 'pending': return '未保存'
    case 'saving': return '保存中…'
    case 'saved': return '已保存'
    case 'error': return '保存失败'
    default: return isDraft ? '本地草稿' : '自动保存'
  }
})

/**
 * 把编辑器 HTML 拆成“行”：块级元素结尾补换行。
 *
 * 注意：ProseMirror 的 `textContent` 不包含块级分隔符，段落之间会直接相连
 * （"第一行第二行"），旧实现直接按 `\n` 拆分，多段落文稿的实时标题会变成
 * 两段拼接后的前 20 个字符。
 */
function htmlToLines(source: string): string[] {
  const div = window.document.createElement('div')
  div.innerHTML = source
  div.querySelectorAll('br').forEach((node) => node.replaceWith('\n'))
  div
    .querySelectorAll('p,div,h1,h2,h3,h4,h5,h6,li,blockquote,pre,tr,table')
    .forEach((node) => node.append('\n'))
  return (div.textContent || '').split('\n')
}

/** 标题由正文首行推导（与后端 extract_first_line_as_title 保持一致） */
const liveTitle = computed(() => {
  const source = html.value
  if (!source.trim()) return '无标题'
  const line = htmlToLines(source)
    .map((item) => item.trim())
    .find((item) => item.length > 0)
  if (!line) return '无标题'
  return line.length > 20 ? `${line.slice(0, 20)}…` : line
})

// ------- 保存 -------
const SAVE_DELAY_MS = 900
const VERSION_COOLDOWN_MS = 30_000

let saveTimer: ReturnType<typeof setTimeout> | null = null
let lastVersionAt = 0
let dirty = false

function onEditorUpdate(value: string) {
  html.value = value
  dirty = true
  saveState.value = 'pending'
  // store 内部还有一层 800ms 防抖写库，这里只负责状态机与版本节流。
  store.updateDocumentContent(noteId, value)
  if (saveTimer) clearTimeout(saveTimer)
  saveTimer = setTimeout(() => { void persist() }, SAVE_DELAY_MS)
}

async function persist() {
  if (saveTimer) {
    clearTimeout(saveTimer)
    saveTimer = null
  }
  if (!dirty) return
  saveState.value = 'saving'
  try {
    await store.flushPendingSave(noteId)
    dirty = false
    saveState.value = 'saved'
    await maybeCommitVersion()
  } catch {
    saveState.value = 'error'
  }
}

async function maybeCommitVersion() {
  if (isDraft) return
  const now = Date.now()
  if (now - lastVersionAt < VERSION_COOLDOWN_MS) return
  try {
    await store.commitDocumentVersion(noteId, 'auto')
    lastVersionAt = Date.now()
  } catch { /* 版本提交失败不影响正文已保存 */ }
}

// ------- 生命周期 -------
onMounted(async () => {
  try {
    await store.selectDocument(noteId)
    html.value = store.currentDocument?.content || ''
    lastVersionAt = Date.now()
  } catch {
    showToast('加载失败')
  } finally {
    loading.value = false
  }
})

// 切换文稿（例如从版本历史恢复后重新进入）时同步正文
watch(() => store.currentDocument?.id, () => {
  if (store.currentDocument?.id === noteId) {
    html.value = store.currentDocument?.content || ''
  }
})

// 页面被系统回收 / 切后台时立即落盘
function onVisibilityChange() {
  if (window.document.visibilityState === 'hidden') void persist()
}

onMounted(() => {
  window.document.addEventListener('visibilitychange', onVisibilityChange)
})

onBeforeUnmount(() => {
  window.document.removeEventListener('visibilitychange', onVisibilityChange)
  // 物理返回键可能直接退栈，这里兜底保证内容与版本都落盘。
  if (dirty) {
    void store.flushPendingSave(noteId).catch(() => {})
    if (!isDraft) void store.commitDocumentVersion(noteId, 'auto').catch(() => {})
  }
})

// ------- 交互 -------
async function goBack() {
  await persist()
  router.back()
}

async function openVersions() {
  await persist()
  router.push(`/versions/${noteId}`)
}

async function onMoreAction(action: { key: string }) {
  showMore.value = false
  if (action.key === 'versions') {
    await openVersions()
  } else if (action.key === 'move') {
    await persist()
    showFolderPicker.value = true
  } else if (action.key === 'share') {
    await shareAsMarkdown()
  } else if (action.key === 'lock') {
    await toggleLock()
  } else if (action.key === 'delete') {
    await confirmDelete()
  }
}

function moreActions(): { name: string; key: string; color?: string }[] {
  const actions: { name: string; key: string; color?: string }[] = [
    { name: '移动到目录', key: 'move' },
  ]
  // 草稿只在本地存储，没有版本历史，避免点进去看到空列表
  if (!isDraft) actions.push({ name: '版本历史', key: 'versions' })
  actions.push(
    { name: '分享为 Markdown', key: 'share' },
    { name: document.value?.isLocked ? '解除锁定' : '加锁保护', key: 'lock' },
    { name: '移入回收站', key: 'delete', color: '#ee0a24' },
  )
  return actions
}

async function shareAsMarkdown() {
  const source = html.value
  if (!source.trim()) {
    showToast('内容为空')
    return
  }
  let text = ''
  try {
    const { default: TurndownService } = await import('turndown')
    const { gfm } = await import('turndown-plugin-gfm')
    const turndown = new TurndownService({ headingStyle: 'atx', codeBlockStyle: 'fenced' })
    turndown.use(gfm)
    text = turndown.turndown(source)
  } catch {
    text = source
  }
  try {
    if (navigator.share) {
      await navigator.share({ title: liveTitle.value, text })
      return
    }
    await navigator.clipboard.writeText(text)
    showToast('已复制到剪贴板')
  } catch (error: any) {
    if (error?.name === 'AbortError') return
    try {
      await navigator.clipboard.writeText(text)
      showToast('已复制到剪贴板')
    } catch {
      showToast('分享失败')
    }
  }
}

async function onFolderPicked(folderId: string) {
  const moved = await store.moveDocument(noteId, folderId)
  if (!moved) {
    showToast(store.error || '移动失败')
    return
  }
  showToast('已移动到目录')
  // 草稿升格为正式文稿后 id 会变，当前路由已失效，回列表重新进入。
  if (isDraft) router.replace('/')
}

async function confirmDelete() {
  try {
    await showConfirmDialog({
      title: '删除文稿',
      message: '「' + (document.value?.title || '无标题') + '」将移入回收站',
    })
    if (saveTimer) {
      clearTimeout(saveTimer)
      saveTimer = null
    }
    dirty = false
    await store.deleteDocument(noteId)
    showToast('已移入回收站')
    router.back()
  } catch { /* 取消 */ }
}

// ------- 加锁 -------
const unlockPwd = ref('')
const unlocking = ref(false)

async function toggleLock() {
  const doc = document.value
  if (!doc) return
  if (doc.isLocked) {
    await store.selectDocument(noteId)
    return
  }
  if (!lockStore.hasPassword) {
    showToast('请先在「我的 → 密码锁」中设置密码')
    return
  }
  await persist()
  await lockStore.lockDocument(noteId)
  await store.selectDocument(noteId)
  showToast('已加锁')
}

async function doUnlock() {
  if (!unlockPwd.value) return
  unlocking.value = true
  try {
    const ok = await lockStore.verify(unlockPwd.value)
    if (!ok) {
      showToast('密码错误')
      return
    }
    await lockStore.unlockDocument(noteId)
    await store.selectDocument(noteId)
    html.value = store.currentDocument?.content || ''
    unlockPwd.value = ''
    showToast('已解锁')
  } finally {
    unlocking.value = false
  }
}
</script>

<template>
  <div class="edit-page">
    <van-nav-bar class="edit-nav">
      <template #left>
        <button type="button" class="nav-back" aria-label="返回" @click="goBack">
          <van-icon name="arrow-left" />
        </button>
      </template>
      <template #title>
        <span class="nav-title">{{ liveTitle }}</span>
      </template>
      <template #right>
        <span class="save-chip" :class="`is-${saveState}`">{{ savedLabel }}</span>
        <button type="button" class="nav-action" aria-label="更多" @click="showMore = true">
          <van-icon name="ellipsis" />
        </button>
      </template>
    </van-nav-bar>

    <van-loading v-if="loading" class="edit-loading" />

    <div v-else-if="isLocked" class="locked-box">
      <span class="locked-icon"><van-icon name="lock" /></span>
      <h2 class="locked-title">此文稿已加锁</h2>
      <p class="locked-copy">输入密码后即可阅读和编辑</p>
      <van-field v-model="unlockPwd" type="password" placeholder="密码锁密码" class="locked-field" @keyup.enter="doUnlock" />
      <van-button type="primary" block class="locked-submit" :loading="unlocking" @click="doUnlock">解锁</van-button>
    </div>

    <div v-else class="edit-stage">
      <MobileRichEditor :model-value="html" @update:model-value="onEditorUpdate" />
    </div>

    <FolderPicker v-model:show="showFolderPicker" @select="onFolderPicked" />

    <van-action-sheet
      v-model:show="showMore"
      :actions="moreActions()"
      cancel-text="取消"
      close-on-click-action
      @select="onMoreAction"
    />
  </div>
</template>

<style scoped>
.edit-page {
  display: flex;
  min-height: 100vh;
  min-height: 100dvh;
  flex-direction: column;
  background: var(--van-background-2);
}

.edit-nav {
  position: sticky;
  top: 0;
  z-index: 6;
  backdrop-filter: blur(16px);
}

.edit-nav :deep(.van-nav-bar__content) {
  height: 52px;
}

.nav-back,
.nav-action {
  display: inline-flex;
  width: 34px;
  height: 34px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: 9px;
  background: transparent;
  color: var(--van-text-color-2);
  font-size: 19px;
}

.nav-back:active,
.nav-action:active {
  background: color-mix(in srgb, var(--van-primary-color) 12%, transparent);
  color: var(--van-primary-color);
}

.nav-title {
  display: block;
  overflow: hidden;
  color: var(--van-text-color);
  font-size: 15px;
  font-weight: 650;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.save-chip {
  margin-right: 2px;
  padding: 2px 8px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--van-text-color-2) 10%, transparent);
  color: var(--van-text-color-3, var(--van-text-color-2));
  font-size: 11px;
  line-height: 1.6;
  white-space: nowrap;
}

.save-chip.is-saved {
  background: color-mix(in srgb, #18a058 12%, transparent);
  color: #18a058;
}

.save-chip.is-saving {
  background: color-mix(in srgb, var(--van-primary-color) 12%, transparent);
  color: var(--van-primary-color);
}

.save-chip.is-error {
  background: color-mix(in srgb, var(--van-danger-color) 12%, transparent);
  color: var(--van-danger-color);
}

.edit-loading {
  display: flex;
  flex: 1;
  justify-content: center;
  padding: 64px 0;
}

.edit-stage {
  min-height: 0;
  flex: 1;
}

/* ---------- 锁定态 ---------- */
.locked-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  width: min(100%, 420px);
  margin: 0 auto;
  padding: 64px 24px calc(32px + env(safe-area-inset-bottom, 0px));
}

.locked-icon {
  display: flex;
  width: 54px;
  height: 54px;
  align-items: center;
  justify-content: center;
  border: 1px solid color-mix(in srgb, #d3a32c 30%, var(--van-border-color));
  border-radius: 14px;
  background: color-mix(in srgb, #d3a32c 10%, var(--van-background-2));
  color: #d3a32c;
  font-size: 24px;
}

.locked-title {
  margin: 18px 0 0;
  color: var(--van-text-color);
  font-size: 18px;
  font-weight: 650;
}

.locked-copy {
  margin: 6px 0 20px;
  color: var(--van-text-color-2);
  font-size: 12px;
}

.locked-field {
  width: 100%;
  border: 1px solid var(--van-border-color);
  border-radius: 10px;
  background: var(--van-background-2);
}

.locked-submit {
  height: 44px;
  margin-top: 12px;
  border-radius: 10px;
}
</style>
