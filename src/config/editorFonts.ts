/**
 * 编辑器正文字体。
 *
 * 默认使用内置的「霞鹜文楷」子集，给写作界面一点书卷气；同时提供系统黑体、
 * 宋体、等宽作为选择。设置只作用于正文，不改变应用界面。
 */
import { computed, ref } from 'vue'

export type EditorFontId = 'kai' | 'sans' | 'serif' | 'mono'

export interface EditorFontOption {
  id: EditorFontId
  label: string
  hint: string
  stack: string
  preview: string
}

export const editorFontOptions: EditorFontOption[] = [
  {
    id: 'kai',
    label: '文楷',
    hint: '书卷气',
    preview: '春江潮水',
    stack: '"LXGW WenKai", "Kaiti SC", "STKaiti", "KaiTi", "Noto Serif CJK SC", "Source Han Serif SC", serif',
  },
  {
    id: 'sans',
    label: '黑体',
    hint: '清爽',
    preview: '春江潮水',
    stack: '"MiSans", "HarmonyOS Sans SC", "PingFang SC", "Noto Sans CJK SC", "Source Han Sans SC", "Microsoft YaHei", sans-serif',
  },
  {
    id: 'serif',
    label: '宋体',
    hint: '端正',
    preview: '春江潮水',
    stack: '"Songti SC", "Noto Serif CJK SC", "Source Han Serif SC", "STSong", "SimSun", serif',
  },
  {
    id: 'mono',
    label: '等宽',
    hint: '克制',
    preview: '春江潮水',
    stack: '"SFMono-Regular", "Roboto Mono", "JetBrains Mono", "Noto Sans Mono CJK SC", monospace',
  },
]

const STORAGE_KEY = 'woo:editor-font:v1'
const DEFAULT_FONT_ID: EditorFontId = 'kai'

function loadFontId(): EditorFontId {
  try {
    const saved = localStorage.getItem(STORAGE_KEY)
    if (saved && editorFontOptions.some(option => option.id === saved)) {
      return saved as EditorFontId
    }
  } catch { /* storage unavailable */ }
  return DEFAULT_FONT_ID
}

export const editorFontId = ref<EditorFontId>(loadFontId())

export const editorFontStack = computed(() =>
  editorFontOptions.find(option => option.id === editorFontId.value)?.stack
    ?? editorFontOptions[0].stack,
)

export function applyEditorFont(): void {
  if (typeof document === 'undefined') return
  document.documentElement.style.setProperty('--editor-font-family', editorFontStack.value)
}

export function setEditorFontId(id: EditorFontId): void {
  editorFontId.value = id
  try { localStorage.setItem(STORAGE_KEY, id) } catch { /* ignore */ }
  applyEditorFont()
}

applyEditorFont()


// ============================================================
// 正文字号（移动端；与 Aa 面板共用）
// ============================================================

export type EditorFontSizeId = 'small' | 'standard' | 'large'

export const editorFontSizeOptions = [
  { id: 'small', label: '小', px: 16 },
  { id: 'standard', label: '标准', px: 17.5 },
  { id: 'large', label: '大', px: 19 },
] as const

const SIZE_STORAGE_KEY = 'woo:mobile-editor-font-size:v1'
const DEFAULT_FONT_SIZE_ID: EditorFontSizeId = 'standard'

function loadFontSizeId(): EditorFontSizeId {
  try {
    const saved = localStorage.getItem(SIZE_STORAGE_KEY)
    if (saved === 'small' || saved === 'standard' || saved === 'large') {
      return saved
    }
  } catch { /* storage unavailable */ }
  return DEFAULT_FONT_SIZE_ID
}

export const editorFontSizeId = ref<EditorFontSizeId>(loadFontSizeId())

export const editorFontSizePx = computed(() =>
  editorFontSizeOptions.find(option => option.id === editorFontSizeId.value)?.px ?? 17.5,
)

export function setEditorFontSizeId(id: EditorFontSizeId): void {
  editorFontSizeId.value = id
  try { localStorage.setItem(SIZE_STORAGE_KEY, id) } catch { /* ignore */ }
}
