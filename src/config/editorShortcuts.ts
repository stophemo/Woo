import { computed, ref } from 'vue'
import { isMac, shortcutDisplay } from './shortcutUtils'

/** 可配置的编辑器命令。快捷键格式统一为 Mod/Shift/Alt + key。 */
export type EditorShortcutAction =
  | 'undo' | 'redo' | 'save'
  | 'h1' | 'h2' | 'h3' | 'h4' | 'h5' | 'h6' | 'paragraph'
  | 'bold' | 'italic' | 'underline' | 'strikethrough' | 'highlight' | 'clear'
  | 'ul' | 'ol' | 'task' | 'quote' | 'code' | 'codeblock'
  | 'link' | 'image' | 'table' | 'hr'

export const EDITOR_SHORTCUT_LABELS: Record<EditorShortcutAction, string> = {
  undo: '撤销', redo: '重做', save: '保存',
  h1: '一级标题', h2: '二级标题', h3: '三级标题', h4: '四级标题', h5: '五级标题', h6: '六级标题', paragraph: '正文',
  bold: '粗体', italic: '斜体', underline: '下划线', strikethrough: '删除线', highlight: '高亮', clear: '清除格式',
  ul: '无序列表', ol: '有序列表', task: '任务列表', quote: '引用', code: '行内代码', codeblock: '代码块',
  link: '链接', image: '图片', table: '表格', hr: '分割线',
}

/** 参考 Typora 的默认编辑习惯，同时避开 Woo 的窗口级快捷键。 */
export const DEFAULT_EDITOR_SHORTCUTS: Record<EditorShortcutAction, string> = {
  undo: 'Mod+Z', redo: 'Mod+Shift+Z', save: 'Mod+S',
  h1: 'Mod+1', h2: 'Mod+2', h3: 'Mod+3', h4: 'Mod+4', h5: 'Mod+5', h6: 'Mod+6', paragraph: 'Mod+0',
  bold: 'Mod+B', italic: 'Mod+I', underline: 'Mod+U', strikethrough: 'Alt+Shift+5', highlight: 'Mod+Shift+H', clear: 'Mod+\\',
  ul: 'Mod+Shift+]', ol: 'Mod+Shift+[', task: 'Mod+Shift+T', quote: 'Mod+Shift+Q', code: 'Mod+Shift+E', codeblock: 'Mod+Shift+K',
  link: 'Mod+K', image: 'Mod+Shift+I', table: 'Mod+T', hr: 'Mod+Shift+-',
}

const STORAGE_KEY = 'woo:editor-shortcuts:v1'
const shortcuts = ref<Record<EditorShortcutAction, string>>(loadShortcuts())
export const editorShortcuts = computed(() => shortcuts.value)

function loadShortcuts(): Record<EditorShortcutAction, string> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return { ...DEFAULT_EDITOR_SHORTCUTS }
    const parsed = JSON.parse(raw) as Partial<Record<EditorShortcutAction, string>>
    return { ...DEFAULT_EDITOR_SHORTCUTS, ...parsed }
  } catch {
    return { ...DEFAULT_EDITOR_SHORTCUTS }
  }
}

function persist() {
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(shortcuts.value)) } catch { /* storage unavailable */ }
}

export function getEditorShortcut(action: EditorShortcutAction): string {
  return shortcuts.value[action]
}

export function setEditorShortcut(action: EditorShortcutAction, shortcut: string): void {
  shortcuts.value = { ...shortcuts.value, [action]: shortcut }
  persist()
  window.dispatchEvent(new CustomEvent('woo-shortcuts-changed'))
}

export function resetEditorShortcuts(): void {
  shortcuts.value = { ...DEFAULT_EDITOR_SHORTCUTS }
  persist()
  window.dispatchEvent(new CustomEvent('woo-shortcuts-changed'))
}

export function shortcutForDisplay(shortcut: string): string {
  return shortcutDisplay(shortcut)
}

/** 把 KeyboardEvent 转成可持久化的标准表示。 */
export function shortcutFromKeyboardEvent(event: KeyboardEvent): string | null {
  if (['Control', 'Meta', 'Alt', 'Shift'].includes(event.key)) return null
  const parts: string[] = []
  if (isMac ? event.metaKey : event.ctrlKey) parts.push('Mod')
  if (event.shiftKey) parts.push('Shift')
  if (event.altKey) parts.push('Alt')
  if (!parts.length) return null

  let key = event.key
  const baseKeyByCode: Record<string, string> = {
    Minus: '-', Equal: '=', BracketLeft: '[', BracketRight: ']', Backslash: '\\',
    Semicolon: ';', Quote: "'", Comma: ',', Period: '.', Slash: '/', Backquote: '`',
  }
  if (baseKeyByCode[event.code]) key = baseKeyByCode[event.code]
  if (/^Digit\d$/.test(event.code)) key = event.code.slice(-1)
  if (key === ' ') key = 'Space'
  if (key === 'Esc') key = 'Escape'
  if (key.length === 1 && /[a-z]/i.test(key)) key = key.toUpperCase()
  return `${parts.join('+')}+${key}`
}

function normalizeKey(key: string): string {
  if (key === ' ') return 'Space'
  if (key === 'Esc') return 'Escape'
  return key.length === 1 ? key.toUpperCase() : key
}

function eventKeyMatches(event: KeyboardEvent, key: string): boolean {
  if (normalizeKey(event.key) === normalizeKey(key)) return true
  const codeByKey: Record<string, string> = {
    '0': 'Digit0', '1': 'Digit1', '2': 'Digit2', '3': 'Digit3', '4': 'Digit4',
    '5': 'Digit5', '6': 'Digit6', '7': 'Digit7', '8': 'Digit8', '9': 'Digit9',
    '-': 'Minus', '=': 'Equal', '[': 'BracketLeft', ']': 'BracketRight', '\\': 'Backslash',
  }
  return key.length === 1 && event.code === codeByKey[key]
}

/** 在捕获阶段匹配快捷键，确保自定义键能覆盖 Tiptap 的默认 keymap。 */
export function matchesShortcut(shortcut: string, event: KeyboardEvent): boolean {
  const tokens = shortcut.split('+').map(token => token.trim()).filter(Boolean)
  const key = tokens.pop()
  if (!key || !eventKeyMatches(event, key)) return false
  const requiredMod = tokens.includes('Mod')
  const requiredCtrl = tokens.includes('Ctrl')
  const requiredMeta = tokens.includes('Meta')
  const requiredAlt = tokens.includes('Alt')
  const requiredShift = tokens.includes('Shift')
  const expectedCtrl = requiredCtrl || (requiredMod && !isMac)
  const expectedMeta = requiredMeta || (requiredMod && isMac)
  if (expectedCtrl !== event.ctrlKey || expectedMeta !== event.metaKey) return false
  if (requiredAlt !== event.altKey || requiredShift !== event.shiftKey) return false
  return true
}

export function shortcutConflicts(action: EditorShortcutAction, value: string): EditorShortcutAction | null {
  for (const candidate of Object.keys(shortcuts.value) as EditorShortcutAction[]) {
    if (candidate !== action && shortcuts.value[candidate].toLowerCase() === value.toLowerCase()) return candidate
  }
  return null
}

export function editorShortcutEntries() {
  return (Object.keys(EDITOR_SHORTCUT_LABELS) as EditorShortcutAction[]).map(action => ({
    action,
    label: EDITOR_SHORTCUT_LABELS[action],
    shortcut: getEditorShortcut(action),
    defaultShortcut: DEFAULT_EDITOR_SHORTCUTS[action],
  }))
}
