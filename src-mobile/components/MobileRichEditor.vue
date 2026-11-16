<template>
  <div
    class="m-editor"
    :class="{ 'is-focused': editorFocused }"
    :style="{ '--m-editor-font-size': `${fontSizePx}px` }"
  >
    <div
      class="m-editor__canvas"
      :style="{ paddingBottom: `${bottomSpace}px` }"
      @click.self="focusEnd"
    >
      <EditorContent :editor="editor" class="m-editor__content" />
    </div>

    <!-- 选中文字后的浮动格式条：不占常驻空间，只在这时出现。 -->
    <BubbleMenu
      v-if="editor"
      :editor="editor"
      :options="bubbleOptions"
      :should-show="shouldShowBubble"
      class="m-bubble"
      @pointerdown.prevent
      @mousedown.prevent
      @contextmenu.prevent
    >
      <button
        type="button"
        class="m-bubble__btn m-bubble__text is-bold"
        :class="{ 'is-active': isActive('bold') }"
        aria-label="粗体"
        @click="runEditorAction('bold')"
      >B</button>
      <button
        type="button"
        class="m-bubble__btn m-bubble__text is-italic"
        :class="{ 'is-active': isActive('italic') }"
        aria-label="斜体"
        @click="runEditorAction('italic')"
      >I</button>
      <button
        type="button"
        class="m-bubble__btn m-bubble__text is-underline"
        :class="{ 'is-active': isActive('underline') }"
        aria-label="下划线"
        @click="runEditorAction('underline')"
      >U</button>
      <button
        type="button"
        class="m-bubble__btn m-bubble__text is-strike"
        :class="{ 'is-active': isActive('strikethrough') }"
        aria-label="删除线"
        @click="runEditorAction('strikethrough')"
      >S</button>
      <span class="m-bubble__divider"></span>
      <button
        type="button"
        class="m-bubble__btn"
        :class="{ 'is-active': isActive('highlight') }"
        aria-label="高亮"
        @click="runEditorAction('highlight')"
      >
        <MIcon name="highlight" :size="18" />
      </button>
      <button
        type="button"
        class="m-bubble__btn"
        :class="{ 'is-active': isActive('link') }"
        aria-label="链接"
        @click="openLinkSheet"
      >
        <MIcon name="link" :size="17" />
      </button>
      <button
        type="button"
        class="m-bubble__btn m-bubble__aa"
        aria-label="更多格式"
        @click="openStyleSheet"
      >Aa</button>
    </BubbleMenu>

    <!-- 键盘上方的常驻入口：一行，拇指可达，不横向滚动。 -->
    <div
      v-if="editorFocused"
      class="m-dock"
      :class="{ 'is-keyboard-open': keyboardInset > 0 }"
      :style="{ bottom: `${keyboardInset}px` }"
    >
      <div v-if="isTableActive" class="m-table-bar">
        <button type="button" @pointerdown.prevent @mousedown.prevent @click="runTableCommand('addRowAfter')">+ 行</button>
        <button type="button" @pointerdown.prevent @mousedown.prevent @click="runTableCommand('deleteRow')">− 行</button>
        <button type="button" @pointerdown.prevent @mousedown.prevent @click="runTableCommand('addColumnAfter')">+ 列</button>
        <button type="button" @pointerdown.prevent @mousedown.prevent @click="runTableCommand('deleteColumn')">− 列</button>
        <button type="button" @pointerdown.prevent @mousedown.prevent @click="runTableCommand('toggleHeaderRow')">表头</button>
        <button type="button" class="is-danger" @pointerdown.prevent @mousedown.prevent @click="runTableCommand('deleteTable')">删除</button>
      </div>

      <div class="m-dock__row" @pointerdown.prevent @mousedown.prevent>
        <button
          type="button"
          class="m-dock__btn"
          :disabled="!canUndo"
          aria-label="撤销"
          @click="runEditorAction('undo')"
        >
          <MIcon name="undo" :size="19" />
        </button>
        <button
          type="button"
          class="m-dock__btn"
          :disabled="!canRedo"
          aria-label="重做"
          @click="runEditorAction('redo')"
        >
          <MIcon name="redo" :size="19" />
        </button>
        <span class="m-dock__divider"></span>
        <button
          type="button"
          class="m-dock__btn m-dock__btn--text"
          aria-label="文字样式"
          @click="openStyleSheet"
        >Aa</button>
        <button
          type="button"
          class="m-dock__btn"
          :class="{ 'is-active': isActive('bulletList') }"
          aria-label="无序列表"
          @click="runEditorAction('ul')"
        >
          <van-icon name="bars" />
        </button>
        <button
          type="button"
          class="m-dock__btn"
          :class="{ 'is-active': isActive('taskList') }"
          aria-label="任务列表"
          @click="runEditorAction('task')"
        >
          <van-icon name="passed" />
        </button>
        <button
          type="button"
          class="m-dock__btn m-dock__btn--accent"
          aria-label="插入内容"
          @click="openInsertSheet"
        >
          <van-icon name="plus" />
        </button>
        <span class="m-dock__spacer"></span>
        <button
          type="button"
          class="m-dock__btn"
          aria-label="收起键盘"
          @click="hideKeyboard"
        >
          <MIcon name="keyboard" :size="20" />
        </button>
      </div>
    </div>

    <!-- 未聚焦时只留一条安静的状态栏，点 Aa 继续排版。 -->
    <div v-else class="m-statusbar" @pointerdown.prevent @mousedown.prevent>
      <span class="m-statusbar__count">{{ wordCount > 0 ? `${wordCount} 字` : '空白文稿' }}</span>
      <button type="button" class="m-statusbar__action" @click="focusAndOpenStyle">Aa 格式</button>
    </div>

    <input ref="imageFileInput" class="m-hidden-input" type="file" accept="image/*" @change="handleImageFile" />

    <!-- ---------------- 插入内容 ---------------- -->
    <van-popup
      v-model:show="showInsertSheet"
      position="bottom"
      round
      close-on-popstate
      class="m-sheet-popup"
      :style="sheetStyle"
      @closed="onSheetClosed"
    >
      <div class="m-sheet">
        <div class="m-sheet__grip"></div>
        <div class="m-sheet__head">
          <h3>插入内容</h3>
          <button type="button" class="m-sheet__close" aria-label="关闭" @click="showInsertSheet = false">
            <van-icon name="cross" />
          </button>
        </div>

        <div class="m-sheet__section">
          <p class="m-sheet__label">标题</p>
          <div class="m-sheet__grid m-sheet__grid--3" @pointerdown.prevent>
            <button type="button" class="m-sheet__item" @click="insertHeading(1)"><span class="m-sheet__h m-sheet__h--1">H1</span><span>一级标题</span></button>
            <button type="button" class="m-sheet__item" @click="insertHeading(2)"><span class="m-sheet__h m-sheet__h--2">H2</span><span>二级标题</span></button>
            <button type="button" class="m-sheet__item" @click="insertHeading(3)"><span class="m-sheet__h m-sheet__h--3">H3</span><span>三级标题</span></button>
          </div>
        </div>

        <div class="m-sheet__section">
          <p class="m-sheet__label">列表与段落</p>
          <div class="m-sheet__grid m-sheet__grid--3" @pointerdown.prevent>
            <button type="button" class="m-sheet__item" @click="insertBlock('ul')"><van-icon name="bars" class="m-sheet__icon" /><span>无序列表</span></button>
            <button type="button" class="m-sheet__item" @click="insertBlock('ol')"><span class="m-sheet__ol">1.</span><span>有序列表</span></button>
            <button type="button" class="m-sheet__item" @click="insertBlock('task')"><van-icon name="passed" class="m-sheet__icon" /><span>任务列表</span></button>
            <button type="button" class="m-sheet__item" @click="insertBlock('quote')"><MIcon name="quote" :size="18" class="m-sheet__icon" /><span>引用</span></button>
            <button type="button" class="m-sheet__item" @click="insertBlock('codeblock')"><MIcon name="code" :size="18" class="m-sheet__icon" /><span>代码块</span></button>
            <button type="button" class="m-sheet__item" @click="insertBlock('hr')"><MIcon name="divider" :size="18" class="m-sheet__icon" /><span>分割线</span></button>
          </div>
        </div>

        <div class="m-sheet__section">
          <p class="m-sheet__label">插入</p>
          <div class="m-sheet__grid m-sheet__grid--3" @pointerdown.prevent>
            <button type="button" class="m-sheet__item" @click="openImageSheet"><van-icon name="photo-o" class="m-sheet__icon" /><span>图片</span></button>
            <button type="button" class="m-sheet__item" @click="openTableSheet"><MIcon name="table" :size="18" class="m-sheet__icon" /><span>表格</span></button>
            <button type="button" class="m-sheet__item" @click="openLinkSheet"><MIcon name="link" :size="17" class="m-sheet__icon" /><span>链接</span></button>
          </div>
        </div>
      </div>
    </van-popup>

    <!-- ---------------- 文字样式 ---------------- -->
    <van-popup
      v-model:show="showStyleSheet"
      position="bottom"
      round
      close-on-popstate
      class="m-sheet-popup"
      :style="sheetStyle"
      @closed="onSheetClosed"
    >
      <div class="m-sheet">
        <div class="m-sheet__grip"></div>
        <div class="m-sheet__head">
          <h3>文字样式</h3>
          <button type="button" class="m-sheet__close" aria-label="关闭" @click="showStyleSheet = false">
            <van-icon name="cross" />
          </button>
        </div>

        <div class="m-sheet__section">
          <p class="m-sheet__label">段落</p>
          <div class="m-style-row">
            <button
              v-for="option in blockOptions"
              :key="option.id"
              type="button"
              class="m-style-chip"
              :class="{ 'is-active': activeBlock === option.id }"
              @pointerdown.prevent
              @click="applyBlock(option.id)"
            >{{ option.label }}</button>
          </div>
        </div>

        <div class="m-sheet__section">
          <p class="m-sheet__label">强调</p>
          <div class="m-mark-row" @pointerdown.prevent>
            <button type="button" class="m-mark-chip" :class="{ 'is-active': isActive('bold') }" @click="runEditorAction('bold')"><span class="is-bold">B</span><span>粗体</span></button>
            <button type="button" class="m-mark-chip" :class="{ 'is-active': isActive('italic') }" @click="runEditorAction('italic')"><span class="is-italic">I</span><span>斜体</span></button>
            <button type="button" class="m-mark-chip" :class="{ 'is-active': isActive('underline') }" @click="runEditorAction('underline')"><span class="is-underline">U</span><span>下划线</span></button>
            <button type="button" class="m-mark-chip" :class="{ 'is-active': isActive('strikethrough') }" @click="runEditorAction('strikethrough')"><span class="is-strike">S</span><span>删除线</span></button>
            <button type="button" class="m-mark-chip" :class="{ 'is-active': isActive('highlight') }" @click="runEditorAction('highlight')"><MIcon name="highlight" :size="17" /><span>高亮</span></button>
            <button type="button" class="m-mark-chip" @click="runEditorAction('clear')"><span class="m-mark-chip__clear">T</span><span>清除</span></button>
          </div>
        </div>

        <div class="m-sheet__section">
          <p class="m-sheet__label">字体</p>
          <div class="m-font-grid">
            <button
              v-for="option in editorFontOptions"
              :key="option.id"
              type="button"
              class="m-font-chip"
              :class="{ 'is-active': editorFontId === option.id }"
              @pointerdown.prevent
              @click="setEditorFontId(option.id)"
            >
              <span class="m-font-chip__preview" :style="{ fontFamily: option.stack }">{{ option.preview }}</span>
              <span class="m-font-chip__meta">
                <span>{{ option.label }}</span>
                <small>{{ option.hint }}</small>
              </span>
            </button>
          </div>
        </div>

        <div class="m-sheet__section">
          <p class="m-sheet__label">字号</p>
          <div class="m-segment" @pointerdown.prevent>
            <button
              v-for="option in fontSizeOptions"
              :key="option.id"
              type="button"
              class="m-segment__item"
              :class="{ 'is-active': fontSizeId === option.id }"
              @click="setFontSize(option.id)"
            >{{ option.label }}</button>
          </div>
        </div>

        <div class="m-sheet__section" v-if="activeLinkHref">
          <p class="m-sheet__label">当前链接</p>
          <div class="m-link-row">
            <span class="m-link-row__url">{{ activeLinkHref }}</span>
            <div class="m-link-row__actions" @pointerdown.prevent>
              <button type="button" @click="openActiveLink">打开</button>
              <button type="button" @click="openLinkSheet">编辑</button>
              <button type="button" class="is-danger" @click="removeLink">移除</button>
            </div>
          </div>
        </div>
      </div>
    </van-popup>

    <!-- ---------------- 插入图片 ---------------- -->
    <van-popup
      v-model:show="showImageSheet"
      position="bottom"
      round
      close-on-popstate
      class="m-sheet-popup"
      :style="sheetStyle"
      @closed="onSheetClosed"
    >
      <div class="m-sheet">
        <div class="m-sheet__grip"></div>
        <div class="m-sheet__head">
          <h3>插入图片</h3>
          <button type="button" class="m-sheet__close" aria-label="关闭" @click="showImageSheet = false">
            <van-icon name="cross" />
          </button>
        </div>
        <div class="m-sheet__section">
          <button type="button" class="m-pick-file" @click="pickImageFile">
            <van-icon name="photo-o" />
            <span>从相册选择</span>
            <span class="m-pick-file__hint">自动压缩，节省同步流量</span>
          </button>
        </div>
        <div class="m-sheet__section">
          <p class="m-sheet__label">或输入图片地址</p>
          <input v-model="imageSource" class="m-input" type="url" inputmode="url" placeholder="https://…" autocomplete="off" spellcheck="false" />
        </div>
        <div class="m-sheet__actions">
          <span class="m-sheet__spacer"></span>
          <button type="button" class="m-btn" @click="showImageSheet = false">取消</button>
          <button type="button" class="m-btn m-btn--primary" :disabled="!imageSource.trim()" @click="confirmImage">插入</button>
        </div>
      </div>
    </van-popup>

    <!-- ---------------- 插入表格 ---------------- -->
    <van-popup
      v-model:show="showTableSheet"
      position="bottom"
      round
      close-on-popstate
      class="m-sheet-popup"
      :style="sheetStyle"
      @closed="onSheetClosed"
    >
      <div class="m-sheet">
        <div class="m-sheet__grip"></div>
        <div class="m-sheet__head">
          <h3>插入表格</h3>
          <button type="button" class="m-sheet__close" aria-label="关闭" @click="showTableSheet = false">
            <van-icon name="cross" />
          </button>
        </div>
        <div class="m-sheet__section m-sheet__grid m-sheet__grid--2">
          <label class="m-field">
            <span class="m-field__label">行数</span>
            <input v-model.number="tableRows" class="m-input" type="number" inputmode="numeric" min="1" max="20" />
          </label>
          <label class="m-field">
            <span class="m-field__label">列数</span>
            <input v-model.number="tableCols" class="m-input" type="number" inputmode="numeric" min="1" max="12" />
          </label>
        </div>
        <p class="m-sheet__hint">插入后可用键盘上方的表格工具增删行列。</p>
        <div class="m-sheet__actions">
          <span class="m-sheet__spacer"></span>
          <button type="button" class="m-btn" @click="showTableSheet = false">取消</button>
          <button type="button" class="m-btn m-btn--primary" @click="confirmTable">插入</button>
        </div>
      </div>
    </van-popup>

    <!-- ---------------- 插入链接 ---------------- -->
    <van-popup
      v-model:show="showLinkSheet"
      position="bottom"
      round
      close-on-popstate
      class="m-sheet-popup"
      :style="sheetStyle"
      @closed="onSheetClosed"
    >
      <div class="m-sheet">
        <div class="m-sheet__grip"></div>
        <div class="m-sheet__head">
          <h3>{{ editingLink ? '编辑链接' : '插入链接' }}</h3>
          <button type="button" class="m-sheet__close" aria-label="关闭" @click="showLinkSheet = false">
            <van-icon name="cross" />
          </button>
        </div>
        <div class="m-sheet__section">
          <label class="m-field">
            <span class="m-field__label">文字</span>
            <input v-model="linkText" class="m-input" type="text" placeholder="选中文字时可留空" autocomplete="off" />
          </label>
        </div>
        <div class="m-sheet__section">
          <label class="m-field">
            <span class="m-field__label">地址</span>
            <input v-model="linkUrl" class="m-input" type="url" inputmode="url" placeholder="https://example.com" autocomplete="off" spellcheck="false" />
          </label>
        </div>
        <div class="m-sheet__actions">
          <button v-if="editingLink" type="button" class="m-btn m-btn--danger" @click="removeLink">移除链接</button>
          <span class="m-sheet__spacer"></span>
          <button type="button" class="m-btn" @click="showLinkSheet = false">取消</button>
          <button type="button" class="m-btn m-btn--primary" :disabled="!linkUrl.trim()" @click="confirmLink">保存</button>
        </div>
      </div>
    </van-popup>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { EditorContent, useEditor } from '@tiptap/vue-3'
import { BubbleMenu } from '@tiptap/vue-3/menus'
import type { Editor } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import Placeholder from '@tiptap/extension-placeholder'
import Underline from '@tiptap/extension-underline'
import TaskList from '@tiptap/extension-task-list'
import TaskItem from '@tiptap/extension-task-item'
import { Table, TableRow, TableHeader, TableCell } from '@tiptap/extension-table'
import Highlight from '@tiptap/extension-highlight'
import Typography from '@tiptap/extension-typography'
import Link from '@tiptap/extension-link'
import { Node as TiptapNode, mergeAttributes } from '@tiptap/core'
import { showToast } from 'vant'
import { markdownToEditorHtml } from '../../src/services/markdown'
import { buildAssetUrl } from '../../src/services/assetLink'
import {
  editorFontId,
  editorFontOptions,
  setEditorFontId,
  editorFontSizeId as fontSizeId,
  editorFontSizeOptions as fontSizeOptions,
  editorFontSizePx as fontSizePx,
  setEditorFontSizeId as setFontSize,
} from '../../src/config/editorFonts'
import {
  EDITOR_SHORTCUT_LABELS,
  getEditorShortcut,
  matchesShortcut,
  type EditorShortcutAction,
} from '../../src/config/editorShortcuts'
import MIcon from './MIcon.vue'

interface Props {
  modelValue: string
}

const props = defineProps<Props>()
const emit = defineEmits<{
  'update:modelValue': [value: string]
  'shortcut-save': []
}>()

const WooImage = TiptapNode.create({
  name: 'image',
  inline: true,
  group: 'inline',
  draggable: true,
  selectable: true,
  atom: true,
  addAttributes() {
    return {
      src: { default: null },
      alt: { default: null },
      title: { default: null },
    }
  },
  parseHTML() {
    return [{ tag: 'img[src]' }]
  },
  renderHTML({ HTMLAttributes }) {
    return ['img', mergeAttributes(HTMLAttributes)]
  },
})

function normalizeContent(value: string): string {
  const source = value || ''
  if (!source.trim()) return ''
  if (/<(p|h[1-6]|table|ul|ol|blockquote|pre|div|br|hr|img|a|strong|em|code)\b[^>]*>/i.test(source)) {
    return source
  }
  return markdownToEditorHtml(source)
}

// ---------- 键盘遮挡 ----------
// Android 在 edge-to-edge 下窗口不会因输入法 resize，WebView 的 visualViewport
// 也看不到键盘，因此由原生 MainActivity 通过 `woo:ime-inset` 推来遮挡高度（CSS px）。
const keyboardInset = ref(0)
let nativeImeInset = 0

function applyKeyboardInset() {
  const viewport = window.visualViewport
  const obscuredByViewport = viewport
    ? Math.max(0, window.innerHeight - viewport.height - viewport.offsetTop)
    : 0
  // M139+ 的 visualViewport 已经是 WebView 局部坐标，优先使用；
  // 老 WebView 看不到键盘时才退回原生上报值。
  const observed = obscuredByViewport > 80 ? obscuredByViewport : nativeImeInset
  keyboardInset.value = Math.round(observed)
}

function onNativeImeInset(event: Event) {
  const detail = (event as CustomEvent<number>).detail
  nativeImeInset = typeof detail === 'number' && Number.isFinite(detail) && detail > 0 ? detail : 0
  applyKeyboardInset()
}

function syncKeyboardInset() {
  applyKeyboardInset()
}

// ---------- 状态 ----------
const editorStateVersion = ref(0)
const editorFocused = ref(false)
const wordCount = ref(0)
const showInsertSheet = ref(false)
const showStyleSheet = ref(false)
const showImageSheet = ref(false)
const showTableSheet = ref(false)
const showLinkSheet = ref(false)
const imageSource = ref('')
const linkText = ref('')
const linkUrl = ref('')
const tableRows = ref(3)
const tableCols = ref(3)
const imageFileInput = ref<HTMLInputElement | null>(null)
const savedSelection = ref<{ from: number; to: number } | null>(null)
const wasFocusedBeforeSheet = ref(false)

const dockHeight = computed(() => (isTableActive.value ? 100 : 52))
const bottomSpace = computed(() => (editorFocused.value ? dockHeight.value + 12 + keyboardInset.value : 76))

// Vant 弹层默认贴屏幕底；键盘打开时会整个落在 IME 后面。
// 把弹层抬到键盘上方，并限制高度，保证输入框和主按钮始终可见。
const sheetStyle = computed(() => ({
  bottom: `${keyboardInset.value}px`,
  maxHeight: `min(78dvh, calc(100dvh - ${keyboardInset.value + 16}px))`,
}))

const blockOptions = [
  { id: 'paragraph', label: '正文' },
  { id: 'h1', label: 'H1' },
  { id: 'h2', label: 'H2' },
  { id: 'h3', label: 'H3' },
  { id: 'quote', label: '引用' },
] as const
type BlockKind = (typeof blockOptions)[number]['id']

// ---------- 编辑器 ----------
const editor = useEditor({
  content: normalizeContent(props.modelValue),
  extensions: [
    StarterKit.configure({ heading: { levels: [1, 2, 3, 4, 5, 6] }, link: false, underline: false }),
    Placeholder.configure({ placeholder: '开始书写…' }),
    Underline,
    TaskList,
    TaskItem.configure({ nested: true }),
    WooImage,
    Table.configure({ resizable: false, renderWrapper: true }),
    TableRow,
    TableHeader,
    TableCell,
    Highlight.configure({ multicolor: false }),
    Typography,
    Link.configure({
      openOnClick: false,
      autolink: true,
      linkOnPaste: true,
      HTMLAttributes: { rel: 'noopener noreferrer nofollow' },
    }),
  ],
  editorProps: {
    attributes: {
      class: 'mobile-tiptap-editor',
      spellcheck: 'false',
      autocapitalize: 'sentences',
      autocomplete: 'off',
      enterkeyhint: 'enter',
    },
    handlePaste: (view, event) => {
      const file = Array.from(event.clipboardData?.files || []).find(item => item.type.startsWith('image/'))
      if (!file) return false
      event.preventDefault()
      void insertImageFile(file)
      return true
    },
  },
  onUpdate: ({ editor: ed }) => {
    // 每次按键同步回写是有意为之：返回/分享/版本提交都依赖 store 里最新的
    // HTML，做节流会丢掉最后一笔。只把字数统计和标题推导放到防抖里。
    emit('update:modelValue', ed.getHTML())
    scheduleWordCount()
  },
  onTransaction: () => {
    editorStateVersion.value++
    scheduleCaretCheck()
  },
  onFocus: () => {
    editorFocused.value = true
    scheduleCaretCheck()
  },
  onBlur: () => {
    editorFocused.value = false
  },
  onCreate: () => {
    updateWordCount()
  },
})

watch(() => props.modelValue, (value) => {
  const ed = editor.value
  if (!ed) return
  // 中文输入法组合输入期间不要重设内容，否则会把候选词打断。
  if (ed.view.composing) return
  const next = normalizeContent(value)
  if (ed.getHTML() === next) return
  ed.commands.setContent(next, { emitUpdate: false })
  updateWordCount()
})

let wordCountTimer: ReturnType<typeof setTimeout> | null = null

function scheduleWordCount() {
  if (wordCountTimer) clearTimeout(wordCountTimer)
  wordCountTimer = setTimeout(() => {
    wordCountTimer = null
    updateWordCount()
  }, 260)
}

function updateWordCount() {
  const text = editor.value?.getText() || ''
  const cjk = (text.match(/[㐀-鿿豈-﫿]/g) || []).length
  const words = text
    .replace(/[㐀-鿿豈-﫿]/g, ' ')
    .trim()
    .split(/\s+/)
    .filter(Boolean).length
  wordCount.value = cjk + words
}

// ---------- 派生状态 ----------
const isTableActive = computed(() => {
  void editorStateVersion.value
  return editor.value?.isActive('table') === true
})

const canUndo = computed(() => {
  void editorStateVersion.value
  return editor.value?.can().undo() === true
})

const canRedo = computed(() => {
  void editorStateVersion.value
  return editor.value?.can().redo() === true
})

const activeBlock = computed<BlockKind>(() => {
  void editorStateVersion.value
  const ed = editor.value
  if (!ed) return 'paragraph'
  if (ed.isActive('heading', { level: 1 })) return 'h1'
  if (ed.isActive('heading', { level: 2 })) return 'h2'
  if (ed.isActive('heading', { level: 3 })) return 'h3'
  if (ed.isActive('blockquote')) return 'quote'
  return 'paragraph'
})

const activeLinkHref = computed(() => {
  void editorStateVersion.value
  const href = editor.value?.getAttributes('link')?.href
  return typeof href === 'string' ? href : ''
})

const editingLink = computed(() => !!activeLinkHref.value)

function isActive(name: string, attrs?: Record<string, unknown>): boolean {
  void editorStateVersion.value
  return editor.value?.isActive(name, attrs) === true
}

const bubbleOptions = {
  // 放在选区下方，避开 Android 长按选择时自带的操作菜单。
  placement: 'bottom' as const,
  offset: 12,
  flip: true,
  shift: { padding: 10 },
  hide: { strategy: 'referenceHidden' as const },
}

function shouldShowBubble({ editor: ed, from, to }: { editor: Editor; from: number; to: number }) {
  return from !== to && ed.isEditable && !ed.view.composing
}

// ---------- 选区记忆 ----------
function hasOpenSheet(): boolean {
  return showInsertSheet.value
    || showStyleSheet.value
    || showImageSheet.value
    || showTableSheet.value
    || showLinkSheet.value
}

function rememberSelection() {
  const ed = editor.value
  if (!ed) return
  savedSelection.value = { from: ed.state.selection.from, to: ed.state.selection.to }
}

function restoreSelection() {
  const ed = editor.value
  const selection = savedSelection.value
  if (!ed || !selection) return
  // 只有在面板打开时才恢复记忆的选区；否则工具条上的普通操作会被一个
  // 已经过期的 savedSelection 拽回旧光标位置。
  if (!hasOpenSheet()) return
  const max = ed.state.doc.content.size
  const from = Math.min(selection.from, max)
  const to = Math.min(selection.to, max)
  try { ed.commands.setTextSelection({ from, to }) } catch { /* node selection 等边界情况 */ }
}

function focusEnd() {
  editor.value?.chain().focus('end').run()
}

// ---------- 光标可见性 ----------
// fixed 工具坞和键盘都不会改变 WebView 的布局视口，浏览器的自动滚动不知道
// 工具坞的存在。这里在光标落到“可视底边 - 工具坞高度”之下时主动补一点滚动。
let caretFrame = 0

function scheduleCaretCheck() {
  if (caretFrame) cancelAnimationFrame(caretFrame)
  caretFrame = requestAnimationFrame(() => {
    caretFrame = 0
    ensureCaretVisible()
  })
}

function ensureCaretVisible() {
  const ed = editor.value
  if (!ed || keyboardInset.value <= 0 || ed.view.composing) return
  try {
    const stage = document.querySelector('.edit-stage') as HTMLElement | null
    const scroller = stage ?? (document.scrollingElement as HTMLElement | null)
    if (!scroller) return
    const position = ed.state.selection.to || ed.state.selection.from
    const coords = ed.view.coordsAtPos(position)
    const stageBottom = stage
      ? stage.getBoundingClientRect().bottom
      : window.innerHeight
    const visibleBottom = Math.min(stageBottom, window.innerHeight - keyboardInset.value)
    const dockTop = visibleBottom - dockHeight.value - 8
    if (coords.bottom > dockTop - 10) {
      const delta = coords.bottom - dockTop + 46
      // 只做小幅向下补偿；异常坐标不许把滚动条拽到顶部。
      if (delta > 0 && delta < 700) {
        scroller.scrollBy({ top: delta, behavior: 'auto' })
      }
    }
  } catch {
    // 选区在节点边界等位置拿不到坐标时忽略，下一次事务会再试。
  }
}

watch(keyboardInset, (value) => {
  if (value > 0) scheduleCaretCheck()
})

function hideKeyboard() {
  editor.value?.commands.blur()
  const active = document.activeElement
  if (active instanceof HTMLElement) active.blur()
}

// ---------- 面板 ----------
function markSheetOpen() {
  wasFocusedBeforeSheet.value = editorFocused.value
  rememberSelection()
}

function closeAllSheets() {
  showInsertSheet.value = false
  showStyleSheet.value = false
  showImageSheet.value = false
  showTableSheet.value = false
  showLinkSheet.value = false
}

function onSheetClosed() {
  // 从「插入」切到「图片/表格/链接」时旧弹层也会触发 closed，
  // 此时不能抢焦点重开键盘，等最后一个面板关闭再说。
  if (hasOpenSheet()) return
  if (wasFocusedBeforeSheet.value) {
    editor.value?.commands.focus()
  }
  wasFocusedBeforeSheet.value = false
}

function openInsertSheet() {
  markSheetOpen()
  showInsertSheet.value = true
}

function openStyleSheet() {
  markSheetOpen()
  showStyleSheet.value = true
}

function openImageSheet() {
  if (!hasOpenSheet()) markSheetOpen()
  // 选图/选表格不需要键盘，先收起让面板完整可见；点输入框时键盘会再弹出。
  hideKeyboard()
  imageSource.value = ''
  showInsertSheet.value = false
  showStyleSheet.value = false
  showImageSheet.value = true
}

function openTableSheet() {
  if (!hasOpenSheet()) markSheetOpen()
  hideKeyboard()
  tableRows.value = 3
  tableCols.value = 3
  showInsertSheet.value = false
  showStyleSheet.value = false
  showTableSheet.value = true
}

function openLinkSheet() {
  const ed = editor.value
  if (!ed) return
  if (!hasOpenSheet()) markSheetOpen()
  const attrs = ed.getAttributes('link')
  linkUrl.value = typeof attrs.href === 'string' ? attrs.href : ''
  linkText.value = ed.state.selection.empty
    ? ''
    : ed.state.doc.textBetween(ed.state.selection.from, ed.state.selection.to, ' ')
  showInsertSheet.value = false
  showStyleSheet.value = false
  showLinkSheet.value = true
}

function focusAndOpenStyle() {
  const ed = editor.value
  if (!ed) return
  rememberSelection()
  wasFocusedBeforeSheet.value = true
  ed.commands.focus()
  showStyleSheet.value = true
}

// ---------- 命令 ----------
function applyBlock(kind: BlockKind) {
  const ed = editor.value
  if (!ed || ed.view.composing) return
  restoreSelection()
  const chain = ed.chain().focus()
  if (kind === 'paragraph') chain.setParagraph().run()
  else if (kind === 'quote') chain.toggleBlockquote().run()
  else chain.toggleHeading({ level: Number(kind.slice(1)) as 1 | 2 | 3 }).run()
  showStyleSheet.value = false
  savedSelection.value = null
}

function insertHeading(level: 1 | 2 | 3) {
  const ed = editor.value
  if (!ed || ed.view.composing) return
  restoreSelection()
  ed.chain().focus().toggleHeading({ level }).run()
  closeAllSheets()
  savedSelection.value = null
}

function insertBlock(action: 'ul' | 'ol' | 'task' | 'quote' | 'codeblock' | 'hr') {
  runEditorAction(action)
  closeAllSheets()
}

function selectCurrentBlockIfEmpty(ed: Editor) {
  const { selection } = ed.state
  if (!selection.empty) return
  const { $from } = selection
  const start = $from.start()
  const end = $from.end()
  if (end > start) {
    ed.commands.setTextSelection({ from: start, to: end })
  }
}

function runEditorAction(action: EditorShortcutAction) {
  const ed = editor.value
  if (!ed || ed.view.composing) return
  restoreSelection()
  // Aa 面板里往往只有一个收起的光标：mark 作用在空选区上没有任何可见效果。
  // 这时自动选中当前段落，让粗体/斜体/高亮等立即生效。
  if (['bold', 'italic', 'underline', 'strikethrough', 'highlight', 'code'].includes(action)) {
    selectCurrentBlockIfEmpty(ed)
  }
  switch (action) {
    case 'undo': ed.chain().focus().undo().run(); break
    case 'redo': ed.chain().focus().redo().run(); break
    case 'save': emit('shortcut-save'); break
    case 'bold': ed.chain().focus().toggleBold().run(); break
    case 'italic': ed.chain().focus().toggleItalic().run(); break
    case 'underline': ed.chain().focus().toggleUnderline().run(); break
    case 'strikethrough': ed.chain().focus().toggleStrike().run(); break
    case 'highlight': ed.chain().focus().toggleHighlight().run(); break
    case 'clear': ed.chain().focus().clearNodes().unsetAllMarks().run(); break
    case 'ul': ed.chain().focus().toggleBulletList().run(); break
    case 'ol': ed.chain().focus().toggleOrderedList().run(); break
    case 'task': ed.chain().focus().toggleTaskList().run(); break
    case 'quote': ed.chain().focus().toggleBlockquote().run(); break
    case 'code': ed.chain().focus().toggleCode().run(); break
    case 'codeblock': ed.chain().focus().toggleCodeBlock().run(); break
    case 'hr': ed.chain().focus().setHorizontalRule().run(); break
    case 'image': openImageSheet(); break
    case 'table': openTableSheet(); break
    case 'link': openLinkSheet(); break
    case 'h1': insertHeading(1); break
    case 'h2': insertHeading(2); break
    case 'h3': insertHeading(3); break
    case 'h4': ed.chain().focus().toggleHeading({ level: 4 }).run(); break
    case 'h5': ed.chain().focus().toggleHeading({ level: 5 }).run(); break
    case 'h6': ed.chain().focus().toggleHeading({ level: 6 }).run(); break
    case 'paragraph': ed.chain().focus().setParagraph().run(); break
  }
  if (!['image', 'table', 'link'].includes(action)) {
    showStyleSheet.value = false
  }
}

// ---------- 图片 ----------
function pickImageFile() {
  imageFileInput.value?.click()
}

async function handleImageFile(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  if (!file.type.startsWith('image/')) {
    showToast('请选择图片文件')
    return
  }
  await insertImageFile(file)
}

function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => (typeof reader.result === 'string' ? resolve(reader.result) : reject(new Error('读取失败')))
    reader.onerror = () => reject(reader.error || new Error('读取失败'))
    reader.readAsDataURL(file)
  })
}

/** 压到 1600px 以内再编码，避免一张照片就把同步流量吃光。 */
async function compressImage(file: File): Promise<string> {
  if (file.size <= 380 * 1024) return readFileAsDataUrl(file)
  try {
    const bitmap = await createImageBitmap(file)
    const maxSide = 1600
    const scale = Math.min(1, maxSide / Math.max(bitmap.width, bitmap.height))
    let width = Math.max(1, Math.round(bitmap.width * scale))
    let height = Math.max(1, Math.round(bitmap.height * scale))
    let quality = 0.82
    let dataUrl = ''
    for (let attempt = 0; attempt < 2; attempt++) {
      const canvas = document.createElement('canvas')
      canvas.width = width
      canvas.height = height
      const context = canvas.getContext('2d')
      if (!context) break
      context.drawImage(bitmap, 0, 0, width, height)
      dataUrl = canvas.toDataURL('image/webp', quality)
      if (!dataUrl.startsWith('data:image/webp')) {
        dataUrl = canvas.toDataURL('image/jpeg', quality)
      }
      if (dataUrl.length <= 1_400_000) break
      width = Math.max(1, Math.round(width * 0.8))
      height = Math.max(1, Math.round(height * 0.8))
      quality = 0.72
    }
    bitmap.close?.()
    if (dataUrl) {
      if (dataUrl.length > 2_400_000) {
        throw new Error('图片压缩后仍超过 2MB，请换一张')
      }
      return dataUrl
    }
  } catch {
    // 部分 WebView 不支持 createImageBitmap：直接塞原图会把同步流量和
    // 编辑卡顿放大一个数量级，超过 2MB 就拒绝，而不是静默回退。
    const fallback = await readFileAsDataUrl(file)
    if (fallback.length > 2_400_000) {
      throw new Error('图片压缩失败，且原图过大')
    }
    return fallback
  }
  return readFileAsDataUrl(file)
}

async function insertImageFile(file: File) {
  if (file.size > 25 * 1024 * 1024) {
    showToast('图片超过 25MB，暂不支持插入')
    return
  }
  try {
    const dataUrl = await compressImage(file)
    insertImageNode(dataUrl, file.name)
  } catch (error: any) {
    showToast(error?.message || '读取图片失败')
  }
}

function insertImageNode(src: string, alt = '') {
  const ed = editor.value
  if (!ed || !src || ed.view.composing) return
  restoreSelection()
  ed.chain().focus().insertContent({ type: 'image', attrs: { src, alt } }).run()
  savedSelection.value = null
  closeAllSheets()
}

function confirmImage() {
  const source = imageSource.value.trim()
  if (!source) return
  const src = buildAssetUrl(source)
  const alt = src.split('/').pop()?.split('?')[0] || ''
  insertImageNode(src, alt)
}

// ---------- 表格 ----------
function confirmTable() {
  const ed = editor.value
  if (!ed || ed.view.composing) return
  const rows = Math.max(1, Math.min(20, Number(tableRows.value) || 3))
  const cols = Math.max(1, Math.min(12, Number(tableCols.value) || 3))
  restoreSelection()
  ed.chain().focus().insertTable({ rows, cols, withHeaderRow: true }).run()
  savedSelection.value = null
  showTableSheet.value = false
}

function runTableCommand(command: string) {
  const ed = editor.value
  if (!ed) return
  const chain = ed.chain().focus() as unknown as Record<string, () => { run: () => boolean }>
  if (typeof chain[command] !== 'function') return
  chain[command]().run()
}

// ---------- 链接 ----------
function normalizeLinkUrl(raw: string): string {
  const url = raw.trim()
  if (!url) return ''
  if (/^(?:https?:\/\/|mailto:|tel:|#)/i.test(url)) return url
  if (/^[\w.-]+\.[a-z]{2,}(\/|$)/i.test(url)) return `https://${url}`
  return `#heading:${url}`
}

function confirmLink() {
  const ed = editor.value
  const url = normalizeLinkUrl(linkUrl.value)
  if (!ed || !url || ed.view.composing) return
  restoreSelection()
  const selected = !ed.state.selection.empty
  if (selected) {
    ed.chain().focus().extendMarkRange('link').setLink({ href: url }).run()
  } else {
    ed.chain()
      .focus()
      .insertContent({ type: 'text', text: linkText.value.trim() || '链接' })
      .extendMarkRange('link')
      .setLink({ href: url })
      .run()
  }
  savedSelection.value = null
  showLinkSheet.value = false
}

function removeLink() {
  const ed = editor.value
  if (!ed || ed.view.composing) return
  restoreSelection()
  ed.chain().focus().extendMarkRange('link').unsetLink().run()
  savedSelection.value = null
  showLinkSheet.value = false
  showStyleSheet.value = false
}

async function openActiveLink() {
  const url = activeLinkHref.value
  if (!/^https?:\/\//i.test(url)) {
    showToast('该链接不是网页地址')
    return
  }
  try {
    const { openUrl } = await import('@tauri-apps/plugin-opener')
    await openUrl(url)
  } catch {
    window.open(url, '_blank', 'noopener')
  }
}

// ---------- 快捷键 ----------
const shortcutActions = Object.keys(EDITOR_SHORTCUT_LABELS) as EditorShortcutAction[]

function handleConfiguredShortcut(event: KeyboardEvent) {
  const ed = editor.value
  if (!ed || !ed.isFocused || event.isComposing || event.keyCode === 229) return
  const action = shortcutActions.find(candidate => matchesShortcut(getEditorShortcut(candidate), event))
  if (!action) return
  event.preventDefault()
  event.stopImmediatePropagation()
  runEditorAction(action)
}

// ---------- 生命周期 ----------
function attachViewportListeners() {
  window.addEventListener('woo:ime-inset', onNativeImeInset)
  const viewport = window.visualViewport
  viewport?.addEventListener('resize', syncKeyboardInset)
  viewport?.addEventListener('scroll', syncKeyboardInset)
  syncKeyboardInset()
}

function detachViewportListeners() {
  window.removeEventListener('woo:ime-inset', onNativeImeInset)
  const viewport = window.visualViewport
  viewport?.removeEventListener('resize', syncKeyboardInset)
  viewport?.removeEventListener('scroll', syncKeyboardInset)
}

onMounted(() => {
  window.addEventListener('keydown', handleConfiguredShortcut, true)
  attachViewportListeners()
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleConfiguredShortcut, true)
  detachViewportListeners()
  if (caretFrame) cancelAnimationFrame(caretFrame)
  if (wordCountTimer) clearTimeout(wordCountTimer)
})
</script>

<style scoped>
.m-editor {
  position: relative;
  min-height: 100%;
  background: var(--w-canvas);
}

/* ---------- 纸面 ---------- */
.m-editor__canvas {
  min-height: 100%;
  padding: 18px clamp(18px, 5.2vw, 30px) 76px;
  transition: padding-bottom 0.16s ease;
}

.m-editor__content {
  min-height: 100%;
}

.m-editor__content :deep(.mobile-tiptap-editor) {
  max-width: 40rem;
  min-height: calc(100dvh - 190px);
  margin: 0 auto;
  outline: none;
  color: var(--w-ink);
  font-family: var(--editor-font-family, "LXGW WenKai", "PingFang SC", serif);
  font-size: var(--m-editor-font-size, 17.5px);
  font-weight: 400;
  letter-spacing: 0.012em;
  line-height: 1.85;
  overflow-wrap: break-word;
  caret-color: var(--w-accent);
  -webkit-tap-highlight-color: transparent;
}

.m-editor__content :deep(.mobile-tiptap-editor ::selection) {
  background: color-mix(in srgb, var(--w-accent) 22%, transparent);
}

.m-editor__content :deep(.mobile-tiptap-editor p) {
  margin: 0 0 0.95em;
}

.m-editor__content :deep(.mobile-tiptap-editor p.is-editor-empty:first-child::before) {
  content: attr(data-placeholder);
  float: left;
  height: 0;
  color: var(--w-ink-3);
  pointer-events: none;
}

/* 标题只用字号、字重和留白区分，不再插入 H1/H2 标签。 */
.m-editor__content :deep(.mobile-tiptap-editor h1),
.m-editor__content :deep(.mobile-tiptap-editor h2),
.m-editor__content :deep(.mobile-tiptap-editor h3),
.m-editor__content :deep(.mobile-tiptap-editor h4),
.m-editor__content :deep(.mobile-tiptap-editor h5),
.m-editor__content :deep(.mobile-tiptap-editor h6) {
  color: var(--w-ink);
  font-weight: 650;
  letter-spacing: -0.012em;
  line-height: 1.38;
}

.m-editor__content :deep(.mobile-tiptap-editor h1) {
  margin: 0.1em 0 0.62em;
  font-size: 1.48em;
  line-height: 1.32;
}

.m-editor__content :deep(.mobile-tiptap-editor h2) {
  margin: 1.55em 0 0.52em;
  font-size: 1.18em;
}

.m-editor__content :deep(.mobile-tiptap-editor h3) {
  margin: 1.32em 0 0.42em;
  font-size: 1.06em;
}

.m-editor__content :deep(.mobile-tiptap-editor h4),
.m-editor__content :deep(.mobile-tiptap-editor h5),
.m-editor__content :deep(.mobile-tiptap-editor h6) {
  margin: 1.2em 0 0.35em;
  color: var(--w-ink-2);
  font-size: 1em;
  font-weight: 600;
}

.m-editor__content :deep(.mobile-tiptap-editor strong) {
  font-weight: 650;
}

.m-editor__content :deep(.mobile-tiptap-editor ul),
.m-editor__content :deep(.mobile-tiptap-editor ol) {
  margin: 0.2em 0 1em;
  padding-left: 1.5em;
}

/* Vant 的全局 reset 把 list-style 清成了 none，编辑区必须显式恢复。 */
.m-editor__content :deep(.mobile-tiptap-editor ul:not([data-type='taskList'])) {
  list-style: disc outside;
}

.m-editor__content :deep(.mobile-tiptap-editor ol) {
  list-style: decimal outside;
}

.m-editor__content :deep(.mobile-tiptap-editor li) {
  margin: 0.18em 0;
  padding-left: 0.12em;
}

.m-editor__content :deep(.mobile-tiptap-editor li p) {
  margin: 0 0 0.25em;
}

.m-editor__content :deep(.mobile-tiptap-editor li::marker) {
  color: var(--w-ink-3);
  font-variant-numeric: tabular-nums;
}

.m-editor__content :deep(.mobile-tiptap-editor ol > li::marker) {
  color: var(--w-accent);
  font-weight: 600;
}

.m-editor__content :deep(.mobile-tiptap-editor blockquote) {
  margin: 1.25em 0;
  padding: 0.15em 0 0.15em 1em;
  border-left: 2px solid color-mix(in srgb, var(--w-accent) 48%, var(--w-line));
  color: var(--w-ink-2);
}

.m-editor__content :deep(.mobile-tiptap-editor blockquote p:last-child) {
  margin-bottom: 0;
}

.m-editor__content :deep(.mobile-tiptap-editor code) {
  padding: 0.08em 0.32em;
  border-radius: 5px;
  background: var(--w-rise);
  color: var(--w-ink);
  font-family: "SFMono-Regular", "Roboto Mono", Consolas, monospace;
  font-size: 0.86em;
  overflow-wrap: anywhere;
}

.m-editor__content :deep(.mobile-tiptap-editor pre) {
  overflow-x: auto;
  margin: 1.2em 0;
  padding: 0.9em 1em;
  border-radius: 12px;
  background: var(--w-rise);
  box-shadow: inset 0 0 0 1px var(--w-line);
  font-size: 0.8em;
  line-height: 1.7;
  tab-size: 2;
  -webkit-overflow-scrolling: touch;
}

.m-editor__content :deep(.mobile-tiptap-editor pre code) {
  padding: 0;
  background: transparent;
  font-size: inherit;
}

.m-editor__content :deep(.mobile-tiptap-editor a) {
  color: var(--w-accent);
  text-decoration: underline;
  text-decoration-thickness: 1px;
  text-underline-offset: 2px;
  overflow-wrap: anywhere;
}

.m-editor__content :deep(.mobile-tiptap-editor hr) {
  margin: 2.1em 0;
  border: 0;
  border-top: 1px solid var(--w-line);
}

.m-editor__content :deep(.mobile-tiptap-editor img) {
  display: block;
  max-width: 100%;
  height: auto;
  margin: 1.2em auto;
  border-radius: 10px;
  background: var(--w-rise);
  box-shadow: 0 0 0 1px var(--w-line);
}

.m-editor__content :deep(.mobile-tiptap-editor mark) {
  padding: 0 0.12em;
  border-radius: 3px;
  background: color-mix(in srgb, var(--w-accent) 20%, transparent);
  color: inherit;
}

/* ---------- 表格 ---------- */
.m-editor__content :deep(.mobile-tiptap-editor .tableWrapper) {
  overflow-x: auto;
  margin: 1.2em 0;
  -webkit-overflow-scrolling: touch;
}

.m-editor__content :deep(.mobile-tiptap-editor table) {
  width: 100%;
  min-width: 460px;
  border-collapse: collapse;
  font-size: 0.86em;
  line-height: 1.62;
}

.m-editor__content :deep(.mobile-tiptap-editor th),
.m-editor__content :deep(.mobile-tiptap-editor td) {
  min-width: 86px;
  padding: 0.55em 0.65em;
  border: 1px solid var(--w-line);
  text-align: left;
  vertical-align: top;
  overflow-wrap: anywhere;
}

.m-editor__content :deep(.mobile-tiptap-editor th) {
  background: var(--w-rise);
  color: var(--w-ink-2);
  font-weight: 600;
}

.m-editor__content :deep(.mobile-tiptap-editor .selectedCell) {
  background: color-mix(in srgb, var(--w-accent) 9%, transparent);
}

/* ---------- 任务列表 ---------- */
.m-editor__content :deep(.mobile-tiptap-editor ul[data-type='taskList']) {
  padding-left: 0;
  list-style: none;
}

.m-editor__content :deep(.mobile-tiptap-editor ul[data-type='taskList'] li) {
  display: flex;
  align-items: flex-start;
  gap: 0.55em;
  padding-left: 0;
}

.m-editor__content :deep(.mobile-tiptap-editor ul[data-type='taskList'] li > label) {
  display: flex;
  width: 1.8em;
  height: 1.8em;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  margin: 0 0 0 -0.4em;
  padding: 0;
}

.m-editor__content :deep(.mobile-tiptap-editor ul[data-type='taskList'] li > div) {
  min-width: 0;
  flex: 1;
}

.m-editor__content :deep(.mobile-tiptap-editor ul[data-type='taskList'] input) {
  width: 19px;
  height: 19px;
  margin: 0;
  accent-color: var(--w-accent);
}

.m-editor__content :deep(.mobile-tiptap-editor ul[data-type='taskList'] li[data-checked='true'] > div) {
  color: var(--w-ink-3);
  text-decoration: line-through;
  text-decoration-color: color-mix(in srgb, var(--w-ink-3) 60%, transparent);
}

/* ---------- 选中浮动条 ---------- */
.m-bubble {
  z-index: 40;
  user-select: none;
  -webkit-user-select: none;
  -webkit-touch-callout: none;
  display: flex;
  max-width: calc(100vw - 16px);
  align-items: center;
  gap: 1px;
  padding: 4px;
  border: 1px solid var(--w-line);
  border-radius: 12px;
  background: var(--w-paper);
  box-shadow: 0 10px 28px color-mix(in srgb, var(--w-ink) 16%, transparent), 0 2px 6px color-mix(in srgb, var(--w-ink) 8%, transparent);
}

.m-bubble__btn {
  display: inline-flex;
  width: 36px;
  height: 36px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--w-ink-2);
  font-size: 17px;
}

.m-bubble__btn:active {
  background: var(--w-rise);
}

.m-bubble__btn.is-active {
  color: var(--w-accent);
}

.m-bubble__text {
  font-family: Georgia, "Times New Roman", serif;
  font-weight: 700;
}

.m-bubble__text.is-italic { font-style: italic; }
.m-bubble__text.is-underline { text-decoration: underline; }
.m-bubble__text.is-strike { text-decoration: line-through; }
.m-bubble__text.is-bold { font-weight: 800; }

.m-bubble__aa {
  font-family: inherit;
  font-size: 12.5px;
  font-weight: 700;
  letter-spacing: 0.01em;
}

.m-bubble__divider {
  width: 1px;
  height: 18px;
  margin: 0 4px;
  background: var(--w-line);
}

/* ---------- 键盘上方工具条 ---------- */
.m-dock {
  position: fixed;
  right: 0;
  bottom: 0;
  left: 0;
  z-index: 30;
  border-top: 1px solid var(--w-line);
  background: color-mix(in srgb, var(--w-paper) 96%, transparent);
  backdrop-filter: blur(20px);
}

.m-dock.is-keyboard-open {
  padding-bottom: 0;
  /* 键盘动画期间 blur 在低端 Android 上会掉帧，改用接近不透明的纯色。 */
  background: var(--w-paper);
  backdrop-filter: none;
}

.m-dock__row {
  display: flex;
  height: 52px;
  align-items: center;
  gap: 2px;
  padding: 0 6px;
  touch-action: manipulation;
  user-select: none;
  -webkit-user-select: none;
  -webkit-touch-callout: none;
}

.m-dock__btn {
  display: inline-flex;
  width: 40px;
  height: 42px;
  flex: 0 0 40px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: 10px;
  background: transparent;
  color: var(--w-ink-2);
  font-size: 20px;
}

.m-dock__btn:active {
  background: var(--w-rise);
}

.m-dock__btn.is-active {
  color: var(--w-accent);
}

.m-dock__btn:disabled {
  opacity: 0.28;
}

.m-dock__btn--text {
  font-family: inherit;
  font-size: 14px;
  font-weight: 700;
  letter-spacing: 0.01em;
}

.m-dock__btn--accent {
  color: var(--w-accent);
}

.m-dock__divider {
  width: 1px;
  height: 18px;
  margin: 0 4px;
  background: var(--w-line);
}

.m-dock__spacer {
  flex: 1;
}

/* 360dp 及以下也能完整放下 7 个入口；大屏再放大触控目标。 */
@media (min-width: 380px) {
  .m-dock__btn {
    width: 44px;
    height: 44px;
    flex-basis: 44px;
  }
}

.m-bubble__btn,
.m-dock__btn,
.m-table-bar button,
.m-style-chip,
.m-mark-chip,
.m-segment__item,
.m-sheet__item {
  touch-action: manipulation;
  user-select: none;
  -webkit-user-select: none;
  -webkit-touch-callout: none;
}

/* ---------- 表格工具 ---------- */
.m-table-bar {
  display: flex;
  gap: 6px;
  overflow-x: auto;
  touch-action: pan-x;
  padding: 7px 12px;
  border-bottom: 1px solid var(--w-line);
  scrollbar-width: none;
  -webkit-overflow-scrolling: touch;
}

.m-table-bar::-webkit-scrollbar { display: none; }

.m-table-bar button {
  flex: 0 0 auto;
  min-height: 30px;
  padding: 3px 10px;
  border: 1px solid var(--w-line-strong);
  border-radius: 8px;
  background: var(--w-canvas);
  color: var(--w-ink-2);
  font-size: 12px;
  white-space: nowrap;
}

.m-table-bar button.is-danger {
  color: var(--van-danger-color);
}

/* ---------- 未聚焦状态栏 ---------- */
.m-statusbar {
  position: fixed;
  right: 0;
  bottom: 0;
  left: 0;
  z-index: 30;
  display: flex;
  user-select: none;
  -webkit-user-select: none;
  -webkit-touch-callout: none;
  height: 42px;
  align-items: center;
  justify-content: space-between;
  padding: 0 18px;
  border-top: 1px solid var(--w-line);
  background: color-mix(in srgb, var(--w-paper) 94%, transparent);
  backdrop-filter: blur(20px);
}

.m-statusbar__count {
  color: var(--w-ink-3);
  font-size: 11.5px;
  letter-spacing: 0.03em;
  font-variant-numeric: tabular-nums;
}

.m-statusbar__action {
  padding: 6px 0;
  border: 0;
  background: transparent;
  color: var(--w-accent);
  font-size: 12.5px;
  font-weight: 600;
}

/* ---------- 面板 ---------- */
/* 旧 WebView 不认识 dvh 时，sheetStyle 的 max-height 会失效，用 vh 兜底。 */
.m-sheet-popup {
  max-height: 78vh;
}

.m-hidden-input {
  position: absolute;
  width: 1px;
  height: 1px;
  opacity: 0;
  pointer-events: none;
}

.m-sheet {
  max-height: inherit;
  overflow-y: auto;
  padding: 10px 18px 20px;
  background: var(--w-canvas);
  color: var(--w-ink);
  overscroll-behavior: contain;
  -webkit-overflow-scrolling: touch;
}

.m-sheet__grip {
  width: 36px;
  height: 4px;
  margin: 0 auto 12px;
  border-radius: 999px;
  background: var(--w-line-strong);
}

.m-sheet__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding-bottom: 4px;
}

.m-sheet__head h3 {
  margin: 0;
  color: var(--w-ink);
  font-size: 16px;
  font-weight: 650;
  letter-spacing: -0.01em;
}

.m-sheet__close {
  display: inline-flex;
  width: 32px;
  height: 32px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--w-ink-3);
  font-size: 17px;
}

.m-sheet__close:active {
  background: var(--w-rise);
}

.m-sheet__section {
  margin-top: 18px;
}

.m-sheet__label {
  margin: 0 0 8px;
  color: var(--w-ink-3);
  font-size: 10.5px;
  font-weight: 650;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.m-sheet__grid {
  display: grid;
  gap: 8px;
}

.m-sheet__grid--2 { grid-template-columns: repeat(2, minmax(0, 1fr)); }
.m-sheet__grid--3 { grid-template-columns: repeat(3, minmax(0, 1fr)); }

.m-sheet__item {
  display: flex;
  min-height: 62px;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 5px;
  padding: 8px 4px;
  border: 0;
  border-radius: 12px;
  background: var(--w-paper);
  box-shadow: inset 0 0 0 1px var(--w-line);
  color: var(--w-ink-2);
  font-size: 12px;
}

.m-sheet__item:active {
  background: color-mix(in srgb, var(--w-accent) 7%, var(--w-paper));
  color: var(--w-accent);
}

.m-sheet__icon {
  color: var(--w-accent);
  font-size: 19px;
}

.m-sheet__h {
  font-family: Georgia, "Times New Roman", serif;
  color: var(--w-ink);
  font-weight: 700;
}

.m-sheet__h--1 { font-size: 22px; }
.m-sheet__h--2 { font-size: 19px; }
.m-sheet__h--3 { font-size: 17px; }

.m-sheet__ol {
  color: var(--w-accent);
  font-size: 17px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.m-sheet__hint {
  margin: 12px 0 0;
  color: var(--w-ink-3);
  font-size: 11.5px;
  line-height: 1.55;
}

.m-sheet__actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 20px;
}

.m-sheet__spacer { flex: 1; }

/* 段落样式分段 */
.m-style-row {
  display: flex;
  gap: 6px;
  overflow-x: auto;
  scrollbar-width: none;
}

.m-style-row::-webkit-scrollbar { display: none; }

.m-style-chip {
  min-height: 38px;
  flex: 1 0 auto;
  padding: 0 14px;
  border: 1px solid var(--w-line);
  border-radius: 9px;
  background: var(--w-paper);
  color: var(--w-ink-2);
  font-size: 12.5px;
  white-space: nowrap;
}

.m-style-chip.is-active {
  border-color: color-mix(in srgb, var(--w-accent) 42%, var(--w-line));
  background: color-mix(in srgb, var(--w-accent) 7%, var(--w-paper));
  color: var(--w-accent);
  font-weight: 600;
}

/* 行内强调 */
.m-mark-row {
  display: grid;
  gap: 7px;
  grid-template-columns: repeat(6, minmax(0, 1fr));
}

.m-mark-chip {
  display: flex;
  min-height: 56px;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 3px;
  padding: 5px 2px;
  border: 0;
  border-radius: 10px;
  background: var(--w-paper);
  box-shadow: inset 0 0 0 1px var(--w-line);
  color: var(--w-ink-2);
  font-size: 9.5px;
}

.m-mark-chip span:first-child {
  color: var(--w-ink);
  font-size: 16px;
}

.m-mark-chip.is-active {
  background: color-mix(in srgb, var(--w-accent) 7%, var(--w-paper));
  color: var(--w-accent);
}

.m-mark-chip.is-active span:first-child {
  color: var(--w-accent);
}

.m-mark-chip__clear {
  font-family: Georgia, serif;
  font-weight: 700;
}

.is-bold { font-weight: 800; }
.is-italic { font-style: italic; }
.is-underline { text-decoration: underline; }
.is-strike { text-decoration: line-through; }

/* 字体 */
.m-font-grid {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.m-font-chip {
  display: flex;
  min-width: 0;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  padding: 10px 12px;
  border: 1px solid var(--w-line);
  border-radius: 12px;
  background: var(--w-paper);
  color: var(--w-ink);
  text-align: left;
}

.m-font-chip.is-active {
  border-color: color-mix(in srgb, var(--w-accent) 45%, var(--w-line));
  background: color-mix(in srgb, var(--w-accent) 6%, var(--w-paper));
}

.m-font-chip__preview {
  max-width: 100%;
  overflow: hidden;
  font-size: 16px;
  line-height: 1.35;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.m-font-chip__meta {
  display: flex;
  align-items: baseline;
  gap: 6px;
  color: var(--w-ink-2);
  font-size: 12px;
}

.m-font-chip__meta small {
  color: var(--w-ink-3);
  font-size: 10px;
}

.m-font-chip.is-active .m-font-chip__meta {
  color: var(--w-accent);
}

/* 字号 */
.m-segment {
  display: flex;
  overflow: hidden;
  border: 1px solid var(--w-line);
  border-radius: 10px;
  background: var(--w-paper);
}

.m-segment__item {
  min-height: 40px;
  flex: 1;
  border: 0;
  border-right: 1px solid var(--w-line);
  background: transparent;
  color: var(--w-ink-2);
  font-size: 12.5px;
}

.m-segment__item:last-child { border-right: 0; }

.m-segment__item.is-active {
  background: color-mix(in srgb, var(--w-accent) 8%, var(--w-paper));
  color: var(--w-accent);
  font-weight: 650;
}

/* 表单 */
.m-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.m-field__label {
  color: var(--w-ink-3);
  font-size: 11px;
}

.m-input {
  width: 100%;
  height: 44px;
  padding: 0 12px;
  border: 1px solid var(--w-line);
  border-radius: 10px;
  background: var(--w-paper);
  color: var(--w-ink);
  font-size: 16px;
  outline: none;
}

.m-input:focus {
  border-color: color-mix(in srgb, var(--w-accent) 55%, var(--w-line));
}

.m-input::placeholder {
  color: var(--w-ink-3);
}

.m-btn {
  min-height: 38px;
  padding: 0 16px;
  border: 0;
  border-radius: 10px;
  background: var(--w-rise);
  color: var(--w-ink-2);
  font-size: 13.5px;
  font-weight: 600;
}

.m-btn--primary {
  background: var(--w-accent);
  color: #fff;
}

.m-btn--primary:disabled {
  opacity: 0.45;
}

.m-btn--danger {
  background: transparent;
  color: var(--van-danger-color);
  padding-left: 0;
}

/* 选择图片 */
.m-pick-file {
  display: flex;
  width: 100%;
  min-height: 76px;
  align-items: center;
  gap: 12px;
  padding: 14px;
  border: 1px dashed var(--w-line-strong);
  border-radius: 12px;
  background: var(--w-paper);
  color: var(--w-ink);
  font-size: 14px;
  text-align: left;
}

.m-pick-file .van-icon {
  color: var(--w-accent);
  font-size: 24px;
}

.m-pick-file__hint {
  margin-left: auto;
  color: var(--w-ink-3);
  font-size: 10.5px;
}

/* 当前链接 */
.m-link-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-radius: 10px;
  background: var(--w-paper);
  box-shadow: inset 0 0 0 1px var(--w-line);
}

.m-link-row__url {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  color: var(--w-ink-2);
  font-size: 11.5px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.m-link-row__actions {
  display: flex;
  flex: 0 0 auto;
  gap: 10px;
}

.m-link-row__actions button {
  padding: 2px 0;
  border: 0;
  background: transparent;
  color: var(--w-accent);
  font-size: 12px;
  font-weight: 600;
}

.m-link-row__actions button.is-danger {
  color: var(--van-danger-color);
}
</style>
