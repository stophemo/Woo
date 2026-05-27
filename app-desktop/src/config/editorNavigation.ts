import { ref } from 'vue'

export interface HeadingInfo {
  level: number
  text: string
  pos: number
}

// 模块级共享状态（Vue 组件间通信，不依赖组件层级）
const headings = ref<HeadingInfo[]>([])
let scrollHandler: ((pos: number) => void) | null = null

export function registerScrollHandler(fn: (pos: number) => void) {
  scrollHandler = fn
}

export function scrollToHeading(pos: number) {
  scrollHandler?.(pos)
}

export function useEditorNavigation() {
  return { headings }
}
