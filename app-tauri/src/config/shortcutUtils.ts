/**
 * 快捷键工具：统一处理跨平台快捷键差异。
 *
 * macOS 使用 ⌘（Meta），Windows/Linux 使用 Ctrl。
 */
export const isMac = navigator.platform.includes('Mac')
export const modLabel = isMac ? '⌘' : 'Ctrl+'
export const altLabel = isMac ? '⌥' : 'Alt+'
export const shiftLabel = isMac ? '⇧' : 'Shift+'

/**
 * 判断键盘事件是否为"修饰键按下"。
 * macOS 检测 metaKey（⌘），其他平台检测 ctrlKey。
 */
export function isModKey(event: KeyboardEvent): boolean {
  return isMac ? event.metaKey : event.ctrlKey
}

/**
 * 将通用快捷键（使用 Mod 前缀）转为平台对应的显示文本。
 *
 * @example
 *   shortcutDisplay('Mod+B')     // macOS → '⌘B'      Windows → 'Ctrl+B'
 *   shortcutDisplay('Mod+Shift+F') // macOS → '⌘⇧F'   Windows → 'Ctrl+Shift+F'
 *   shortcutDisplay('Shift+Alt+1') // macOS → '⇧⌥1'  Windows → 'Shift+Alt+1'
 */
export function shortcutDisplay(shortcut: string): string {
  let result = shortcut
    .replace(/Mod\+/g, modLabel)
    .replace(/Ctrl\+/g, isMac ? '⌘' : 'Ctrl+')
    .replace(/Shift\+/g, shiftLabel)
    .replace(/Alt\+/g, altLabel)
  return result
}

