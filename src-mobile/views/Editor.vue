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
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'
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
const loadError = ref(false)
const html = ref('')
const saveState = ref<'idle' | 'pending' | 'saving' | 'saved' | 'error'>('idle')
const showMore = ref(false)
const showToc = ref(false)

const showFolderPicker = ref(false)

interface TocHeading {
  level: number
  text: string
}

const tocHeadings = computed<TocHeading[]>(() => {
  if (!html.value) return []
  const container = window.document.createElement('div')
  container.innerHTML = html.value
  return Array.from(container.querySelectorAll('h1,h2,h3,h4,h5,h6')).map((node) => ({
    level: Number(node.tagName.slice(1)),
    text: (node.textContent || '').trim() || '（空标题）'
  }))
})

function jumpToHeading(index: number) {
  showToc.value = false
  void nextTick(() => {
    const root = window.document.querySelector('.mobile-tiptap-editor')
    const heading = root?.querySelectorAll('h1,h2,h3,h4,h5,h6')[index]
    heading?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  })
}

const document = computed(() => store.currentDocument)
const isLocked = computed(() => !!document.value?.isLocked)
const savedLabel = computed(() => {
  switch (saveState.value) {
    case 'pending': return '编辑中'
    case 'saving': return '保存中'
    case 'saved': return '已保存'
    case 'error': return '保存失败'
    default: return isDraft ? '本地草稿' : ''
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
function deriveLiveTitle(source: string): string {
  if (!source.trim()) return '无标题'
  const line = htmlToLines(source)
    .map((item) => item.trim())
    .find((item) => item.length > 0)
  if (!line) return '无标题'
  return line.length > 20 ? `${line.slice(0, 20)}…` : line
}

// 标题解析要遍历整篇 HTML，按键时不必逐字执行；节流到 220ms 后仍然足够实时。
const liveTitle = ref(deriveLiveTitle(''))
let titleTimer: ReturnType<typeof setTimeout> | null = null
watch(html, (value) => {
  if (titleTimer) clearTimeout(titleTimer)
  titleTimer = setTimeout(() => {
    titleTimer = null
    liveTitle.value = deriveLiveTitle(value)
  }, 220)
}, { immediate: true })

// ------- 保存 -------
const SAVE_DELAY_MS = 900
const VERSION_COOLDOWN_MS = 30_000

let saveTimer: ReturnType<typeof setTimeout> | null = null
let savedResetTimer: ReturnType<typeof setTimeout> | null = null
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

function markSaved() {
  saveState.value = 'saved'
  if (savedResetTimer) clearTimeout(savedResetTimer)
  savedResetTimer = setTimeout(() => {
    if (saveState.value === 'saved') saveState.value = 'idle'
  }, 1600)
}

function onSaveStateTap() {
  // 保存失败后点状态文字即可重试；dirty 在失败时保持 true。
  if (saveState.value === 'error') void persist()
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
    markSaved()
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

// Android 物理返回键走 WebView 的历史后退，不会经过 goBack()；路由守卫是
// 唯一能同时覆盖「顶栏返回」和「物理返回」的落盘点。
onBeforeRouteLeave(async () => {
  try {
    await persist()
  } catch {
    // 落盘失败也允许离开：内容仍在内存与本地存储里，下次进入会继续保存。
  }
  return true
})

// ------- 生命周期 -------
async function loadDocument() {
  loading.value = true
  loadError.value = false
  try {
    await store.selectDocument(noteId)
    html.value = store.currentDocument?.content || ''
    lastVersionAt = Date.now()
  } catch {
    // 加载失败时不能渲染空编辑器：用户一输入就会把原正文覆盖成空稿。
    loadError.value = true
  } finally {
    loading.value = false
  }
}

onMounted(loadDocument)

// 切换文稿（例如从版本历史恢复后重新进入）时同步正文
watch(() => store.currentDocument?.id, () => {
  if (store.currentDocument?.id === noteId) {
    html.value = store.currentDocument?.content || ''
  }
})

// 同步把云端更新合并进当前文稿时，store 会替换 currentDocumentData；
// 只监听 id 会让编辑器继续显示旧正文，用户一输入就会反向覆盖刚拉下来的版本。
watch(() => store.currentDocument?.content, (content) => {
  if (store.currentDocument?.id !== noteId) return
  if (typeof content !== 'string') return
  if (content === html.value) return
  // 本地还有未落盘的修改时，以本地为准，避免同步刷新打断输入。
  if (dirty) return
  html.value = content
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
  if (savedResetTimer) clearTimeout(savedResetTimer)
  if (titleTimer) clearTimeout(titleTimer)
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
    <header class="edit-top">
      <button type="button" class="edit-top__btn" aria-label="返回" @click="goBack">
        <van-icon name="arrow-left" />
      </button>
      <span class="edit-top__spacer"></span>
      <span
        v-if="savedLabel"
        class="edit-top__state"
        :class="`is-${saveState}`"
        :role="saveState === 'error' ? 'button' : undefined"
        @click="onSaveStateTap"
      >
        <span class="edit-top__dot"></span>{{ savedLabel }}
      </span>
      <button
        type="button"
        class="edit-top__btn"
        aria-label="目录"
        :disabled="tocHeadings.length === 0"
        @click="showToc = true"
      >
        <van-icon name="bars" />
      </button>
      <button type="button" class="edit-top__btn" aria-label="更多" @click="showMore = true">
        <van-icon name="ellipsis" />
      </button>
    </header>

    <van-loading v-if="loading" class="edit-loading" />

    <div v-else-if="loadError" class="locked-box">
      <span class="locked-icon"><van-icon name="info-o" /></span>
      <h2 class="locked-title">文稿加载失败</h2>
      <p class="locked-copy">为避免覆盖原稿，编辑已暂停。请检查后重试。</p>
      <button type="button" class="locked-submit" @click="loadDocument">重新加载</button>
    </div>

    <div v-else-if="isLocked" class="locked-box">
      <span class="locked-icon"><van-icon name="lock" /></span>
      <h2 class="locked-title">此文稿已加锁</h2>
      <p class="locked-copy">输入密码后即可阅读和编辑</p>
      <input
        v-model="unlockPwd"
        type="password"
        class="locked-input"
        placeholder="密码锁密码"
        @keyup.enter="doUnlock"
      />
      <button type="button" class="locked-submit" :disabled="unlocking" @click="doUnlock">
        {{ unlocking ? '校验中…' : '解锁' }}
      </button>
    </div>

    <div v-else class="edit-stage">
      <MobileRichEditor :model-value="html" @update:model-value="onEditorUpdate" />
    </div>

    <FolderPicker v-model:show="showFolderPicker" @select="onFolderPicked" />

    <van-popup
      v-model:show="showToc"
      position="right"
      class="toc-popup"
      :style="{ width: '78%', height: '100%' }"
      close-on-popstate
    >
      <div class="toc-panel">
        <div class="toc-head">
          <strong>目录</strong>
          <button type="button" class="edit-top__btn" aria-label="关闭目录" @click="showToc = false">
            <van-icon name="cross" />
          </button>
        </div>
        <div v-if="tocHeadings.length" class="toc-list">
          <button
            v-for="(heading, index) in tocHeadings"
            :key="`${index}-${heading.text}`"
            type="button"
            class="toc-item"
            :class="`is-h${heading.level}`"
            :style="{ paddingLeft: `${14 + (heading.level - 1) * 14}px` }"
            @click="jumpToHeading(index)"
          >
            {{ heading.text }}
          </button>
        </div>
        <p v-else class="toc-empty">这篇文稿还没有标题</p>
      </div>
    </van-popup>

    <van-action-sheet
      v-model:show="showMore"
      :actions="moreActions()"
      cancel-text="取消"
      close-on-click-action
      close-on-popstate
      @select="onMoreAction"
    />
  </div>
</template>

<style scoped>
.edit-page {
  display: flex;
  height: 100vh;
  height: 100dvh;
  overflow: hidden;
  flex-direction: column;
  background: var(--w-canvas);
}

/* 顶部只留「返回 / 保存状态 / 更多」：正文首行就是标题，
 * 这里再显示一次会和正文的大标题重复，也容易显得像一条贴顶的 tab。 */
.edit-top {
  position: relative;
  z-index: 35;
  display: flex;
  flex: 0 0 auto;
  height: 50px;
  align-items: center;
  gap: 4px;
  padding: 8px 10px 2px;
  background: var(--w-canvas);
}

.edit-top__spacer {
  min-width: 0;
  flex: 1;
}

.edit-top__btn {
  display: inline-flex;
  width: 38px;
  height: 38px;
  flex: 0 0 38px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: 10px;
  background: transparent;
  color: var(--w-ink-2);
  font-size: 19px;
}

.edit-top__btn:active {
  background: var(--w-rise);
  color: var(--w-accent);
}

.edit-top__state {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 4px;
  padding-right: 2px;
  color: var(--w-ink-3);
  font-size: 10.5px;
  letter-spacing: 0.02em;
  white-space: nowrap;
}

.edit-top__dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: currentColor;
}

.edit-top__state.is-pending,
.edit-top__state.is-saving {
  color: var(--w-accent);
}

.edit-top__state.is-saved {
  color: var(--w-ink-3);
}

.edit-top__state.is-error {
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
  overflow-y: auto;
  overscroll-behavior: contain;
  -webkit-overflow-scrolling: touch;
}

/* ---------- 锁定态 ---------- */
.locked-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  width: min(100%, 420px);
  margin: 0 auto;
  padding: 72px 26px 32px;
}

.locked-icon {
  color: var(--w-accent);
  font-size: 26px;
}

.locked-title {
  margin: 16px 0 0;
  color: var(--w-ink);
  font-size: 17px;
  font-weight: 600;
}

.locked-copy {
  margin: 6px 0 22px;
  color: var(--w-ink-2);
  font-size: 12.5px;
}

.locked-input {
  width: 100%;
  height: 44px;
  padding: 0 14px;
  border: 1px solid var(--w-line-strong);
  border-radius: 10px;
  background: var(--w-canvas);
  color: var(--w-ink);
  font-size: 15px;
  outline: none;
}

.locked-input:focus {
  border-color: var(--w-accent);
}

.locked-submit {
  width: 100%;
  height: 44px;
  margin-top: 12px;
  border: 0;
  border-radius: 10px;
  background: var(--w-accent);
  color: #fff;
  font-size: 15px;
  font-weight: 600;
}

.locked-submit:disabled {
  opacity: 0.6;
}

.toc-popup {
  background: var(--w-canvas);
}

.toc-panel {
  display: flex;
  height: 100%;
  flex-direction: column;
  padding: max(12px, env(safe-area-inset-top)) 0 max(16px, env(safe-area-inset-bottom));
}

.toc-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 12px 10px;
  border-bottom: 1px solid var(--w-line);
  font-size: 16px;
}

.toc-list {
  overflow-y: auto;
  padding: 8px 0;
  -webkit-overflow-scrolling: touch;
}

.toc-item {
  display: block;
  width: 100%;
  padding-top: 10px;
  padding-right: 14px;
  padding-bottom: 10px;
  border: 0;
  background: transparent;
  color: var(--w-ink-2);
  font-size: 14px;
  line-height: 1.45;
  text-align: left;
  overflow-wrap: anywhere;
}

.toc-item.is-h1 { color: var(--w-ink); font-size: 15px; font-weight: 600; }
.toc-item.is-h2 { color: var(--w-ink); font-size: 14px; font-weight: 500; }
.toc-item.is-h4,
.toc-item.is-h5,
.toc-item.is-h6 { font-size: 13px; }

.toc-item:active {
  background: var(--w-rise);
  color: var(--w-accent);
}

.toc-empty {
  margin: 24px 16px;
  color: var(--w-ink-3);
  font-size: 13px;
  text-align: center;
}
</style>
