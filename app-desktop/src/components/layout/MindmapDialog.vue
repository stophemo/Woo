<template>
  <teleport to="body">
    <div class="mm-overlay" :class="{ 'mm-closing': closing }" @click.self="startClose">
      <div class="mm-card">
        <div class="mm-card-header">
          <span class="mm-card-title">思维导图预览</span>
          <div class="mm-card-actions">
            <button class="mm-action-btn" :disabled="!hasData" @click="exportImage" title="导出为图片">
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>
              </svg>
              导出为图片
            </button>
            <button class="mm-close-btn" @click="startClose" title="关闭">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
              </svg>
            </button>
          </div>
        </div>

        <div class="mm-card-body">
          <div v-if="!hasData" class="mm-empty">
            <p>所选列表为空</p>
          </div>
          <div v-else class="mm-canvas">
            <svg ref="svgRef" class="mm-svg"></svg>
          </div>
        </div>
      </div>
    </div>
  </teleport>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onBeforeUnmount, nextTick } from 'vue'
import { Markmap } from 'markmap-view'
import type { INode, IPureNode } from 'markmap-common'
import type { TreeNode } from '../../types/mindmap'

const props = defineProps<{
  data: TreeNode
}>()

const emit = defineEmits<{ close: [] }>()
const closing = ref(false)

function startClose() {
  // 先销毁 markmap，断开 SVG DOM，避免 unmount 时集中清理导致闪屏
  mm?.destroy()
  mm = null
  closing.value = true
  // 动画结束后再 emit close
  const el = document.querySelector('.mm-overlay')
  const handler = () => {
    el?.removeEventListener('transitionend', handler)
    emit('close')
  }
  el?.addEventListener('transitionend', handler)
  setTimeout(() => {
    el?.removeEventListener('transitionend', handler)
    emit('close')
  }, 600)
}

const svgRef = ref<SVGSVGElement | null>(null)
let mm: any = null

// ── 主题感知色板 ──
const lightPalette = [
  '#007AFF', '#34C759', '#FF9F0A', '#FF375F',
  '#5E5CE6', '#64D2FF', '#BF5AF2', '#00C7BE',
]
const darkPalette = [
  '#5AB0FF', '#5CD974', '#FFB340', '#FF6B85',
  '#8B8AFF', '#80DFFF', '#D98AFF', '#5CE0D8',
]

function isDarkMode(): boolean {
  return document.documentElement.getAttribute('data-theme') === 'dark'
}

function pickColor(node: INode): string {
  const dark = isDarkMode()
  const depth = node.state.depth
  if (depth === 0) return dark ? '#d5d0c8' : '#37342f'
  const palette = dark ? darkPalette : lightPalette
  let hash = 0
  for (const ch of node.content) hash = ((hash << 5) - hash) + ch.charCodeAt(0)
  const idx = (Math.abs(hash) + depth) % palette.length
  return palette[idx]
}

/** markmap 的 style 选项：注入 CSS 到 SVG 内，确保文字和线条随主题变化 */
function themeStyle(id: string): string {
  const dark = isDarkMode()
  const textClr = dark ? '#d5d0c8' : '#37342f'
  const dim = dark ? '#6a6560' : '#9a9690'
  return `
#${id} { background: ${dark ? '#2a2926' : '#f0ede9'} !important; }
#${id} .markmap-node { color: ${textClr} !important; fill: ${textClr} !important; }
#${id} .markmap-link { stroke: ${dim} !important; }
`
}

function toMarkmapNode(text: string, children: any[]): IPureNode {
  return {
    content: text || '(空)',
    children: children.map((c) => toMarkmapNode(c.text, c.children)),
  }
}

function countNodes(node: IPureNode): number {
  let count = 1
  for (const c of node.children) count += countNodes(c)
  return count
}

const rootNode = computed<IPureNode>(() => {
  const title = props.data.text || '思维导图'
  return toMarkmapNode(title, props.data.children)
})

const nodeCount = computed(() => countNodes(rootNode.value))
const hasData = computed(() => nodeCount.value > 1)

async function render() {
  if (!svgRef.value) return
  await nextTick()
  if (!mm) {
    mm = Markmap.create(svgRef.value, {
      autoFit: true,
      duration: 0,
      initialExpandLevel: 999,
      maxInitialScale: 1.2,
      pan: true,
      zoom: true,
      scrollForPan: true,
      toggleRecursively: true,
      id: 'mm-canvas',
      color: pickColor,
      style: themeStyle,
      spacingHorizontal: 50,      // 水平间距（Apple 风格宽敞）
      spacingVertical: 6,         // 垂直间距紧凑
      paddingX: 16,               // 节点内边距
      nodeMinHeight: 22,          // 节点最小高度
      maxWidth: 280,              // 节点文本最大宽度
      fitRatio: 0.92,
    }, rootNode.value)
  } else {
    await mm.setData(rootNode.value)
    mm.fit()
  }
}

function applyThemeToSvg() {
  if (!svgRef.value) return
  const isDark = document.documentElement.getAttribute('data-theme') === 'dark'
  const textColor = isDark ? '#d5d0c8' : '#37342f'
  const dimColor = isDark ? '#6a6560' : '#9a9690'
  // 遍历 SVG 中所有 text 元素，强制设置 fill
  const texts = svgRef.value.querySelectorAll<SVGTextElement>('text')
  for (const t of texts) t.setAttribute('fill', textColor)
  // 遍历所有 link 线条
  const links = svgRef.value.querySelectorAll<SVGPathElement>('.markmap-link')
  for (const l of links) l.setAttribute('stroke', dimColor)
}

onMounted(() => {
  if (hasData.value) void render()
  // 渲染完成后强制应用主题色
  setTimeout(applyThemeToSvg, 50)
})

onBeforeUnmount(() => {
  mm?.destroy()
  mm = null
})

async function exportImage() {
  if (!svgRef.value || !mm) return

  const svgData = new XMLSerializer().serializeToString(svgRef.value)
  const wooInvoke = (window as any).woo.invoke as (ch: string, ...a: any[]) => Promise<any>

  const result = await wooInvoke('dialog:save-image', { defaultName: 'mindmap.png' })
  if (result.cancelled) return

  const { filePath, format } = result

  if (format === 'svg') {
    await wooInvoke('file:write', { filePath, data: svgData, isBase64: false })
    return
  }

  // 栅格格式：png / jpg / webp — 通过 canvas 渲染导出
  const mimeMap: Record<string, string> = { png: 'image/png', jpg: 'image/jpeg', webp: 'image/webp' }
  const mimeType = mimeMap[format] || 'image/png'
  const svgBlob = new Blob([svgData], { type: 'image/svg+xml;charset=utf-8' })
  const url = URL.createObjectURL(svgBlob)
  const img = new Image()
  img.onload = async () => {
    const rect = svgRef.value!.getBoundingClientRect()
    const scale = 2
    const canvas = document.createElement('canvas')
    canvas.width = Math.max(rect.width * scale, 1200)
    canvas.height = Math.max(rect.height * scale, 800)
    const ctx = canvas.getContext('2d')!
    ctx.scale(scale, scale)
    // JPG 不支持透明，用白色背景；其余格式沿用主题背景色
    ctx.fillStyle = format === 'jpg' ? '#ffffff' : isDarkMode() ? '#2a2926' : '#f0ede9'
    ctx.fillRect(0, 0, canvas.width, canvas.height)
    ctx.drawImage(img, 0, 0)
    const quality = format === 'jpg' ? 0.92 : format === 'webp' ? 0.9 : undefined
    canvas.toBlob(async (blob) => {
      if (!blob) return
      const buf = await blob.arrayBuffer()
      const base64 = btoa(String.fromCharCode(...new Uint8Array(buf)))
      await wooInvoke('file:write', { filePath, data: base64, isBase64: true })
      URL.revokeObjectURL(url)
    }, mimeType, quality)
  }
  img.onerror = () => { URL.revokeObjectURL(url) }
  img.src = url
}
</script>

<style scoped>
.mm-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  background: rgba(0, 0, 0, 0.25);
  backdrop-filter: blur(6px);
  -webkit-backdrop-filter: blur(6px);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 32px;
}

.mm-card {
  width: 100%;
  max-width: 900px;
  height: 80vh;
  max-height: 700px;
  background: var(--bg-surface);
  border-radius: 16px;
  box-shadow: 0 12px 48px rgba(0, 0, 0, 0.12), 0 2px 8px rgba(0, 0, 0, 0.06);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  transform: scale(1);
  transition: transform 0.2s ease;
}

.mm-card-header {
  display: flex;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-secondary, #e5e5e5);
  flex-shrink: 0;
}

.mm-card-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary, #1d1d1f);
  flex: 1;
}

.mm-card-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.mm-action-btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 6px 14px;
  border: 1px solid var(--border-primary, #d2d2d7);
  border-radius: 8px;
  background: var(--bg-elevated, #f5f5f7);
  color: var(--text-secondary, #6e6e73);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s;
}

.mm-action-btn:hover:not(:disabled) {
  background: var(--accent, #0071e3);
  color: #fff;
  border-color: var(--accent, #0071e3);
}

.mm-action-btn:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

.mm-close-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--text-secondary, #6e6e73);
  cursor: pointer;
  transition: background 0.15s;
  margin-left: 4px;
}

.mm-close-btn:hover {
  background: var(--bg-hover, #e8e8ed);
}

.mm-card-body {
  flex: 1;
  display: flex;
  overflow: hidden;
  background: var(--bg-tertiary);
}

.mm-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  font-size: 14px;
}

.mm-canvas {
  flex: 1;
  overflow: hidden;
  position: relative;
}

.mm-svg {
  display: block;
  width: 100%;
  height: 100%;
}

/* ── 弹窗淡入淡出动画 ── */
.mm-overlay {
  opacity: 1;
  transition: opacity 0.4s ease;
}
.mm-overlay .mm-card {
  opacity: 1;
  transform: scale(1);
  transition: transform 0.4s ease, opacity 0.4s ease;
}
.mm-overlay.mm-closing {
  opacity: 0;
  pointer-events: none;
}
.mm-overlay.mm-closing .mm-card {
  opacity: 0;
  transform: scale(0.97);
}
</style>

<!-- 非 scoped 样式，用于 markmap SVG 内部元素 -->
<style>
.mm-svg {
  background: var(--bg-tertiary) !important;
}
.mm-svg .markmap-node {
  color: var(--text-primary) !important;
  fill: var(--text-primary) !important;
}
.mm-svg .markmap-node text {
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', sans-serif;
  font-size: 14px;
  fill: inherit;
  color: inherit;
}
.mm-svg .markmap-node circle {
  stroke-width: 2;
  fill: var(--accent) !important;
  stroke: var(--accent) !important;
}
.mm-svg .markmap-link {
  stroke: var(--text-muted) !important;
  stroke-width: 1.7;
}
</style>
