/**
 * 键盘遮挡高度（CSS px）。
 *
 * Android 启用 edge-to-edge 后窗口不会因输入法 resize，WebView 的 visualViewport
 * 也看不到键盘，因此由原生 MainActivity 通过 `woo:ime-inset` 事件与
 * `--woo-ime-inset` 变量把遮挡高度推过来；窗口会正常 resize 的平台用
 * visualViewport 兜底，两者取较大值。
 */
import { onBeforeUnmount, onMounted, ref } from 'vue'

export function useKeyboardInset() {
  const inset = ref(0)
  let nativeInset = 0

  function apply() {
    const viewport = window.visualViewport
    const obscured = viewport
      ? Math.max(0, window.innerHeight - viewport.height - viewport.offsetTop)
      : 0
    inset.value = Math.round(Math.max(nativeInset, obscured > 80 ? obscured : 0))
  }

  function onNativeImeInset(event: Event) {
    const detail = (event as CustomEvent<number>).detail
    nativeInset = typeof detail === 'number' && Number.isFinite(detail) && detail > 0 ? detail : 0
    apply()
  }

  onMounted(() => {
    window.addEventListener('woo:ime-inset', onNativeImeInset)
    window.visualViewport?.addEventListener('resize', apply)
    apply()
  })

  onBeforeUnmount(() => {
    window.removeEventListener('woo:ime-inset', onNativeImeInset)
    window.visualViewport?.removeEventListener('resize', apply)
  })

  return { inset }
}
