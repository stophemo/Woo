<template>
  <teleport to="body">
    <transition name="mm-fade">
      <div class="mm-overlay" @click.self="$emit('close')">
        <div class="mm-card">
          <div class="mm-card-header">
            <span class="mm-card-title">思维导图预览</span>
            <div class="mm-card-actions">
              <button class="mm-action-btn" :disabled="!hasData" @click="exportPNG" title="导出 PNG">
                <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>
                </svg>
                导出 PNG
              </button>
              <button class="mm-action-btn" :disabled="!hasData" @click="exportSVG" title="导出 SVG">
                <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/>
                </svg>
                导出 SVG
              </button>
              <button class="mm-close-btn" @click="$emit('close')" title="关闭">
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
    </transition>
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

defineEmits<{ close: [] }>()

const svgRef = ref<SVGSVGElement | null>(null)
let mm: Markmap | null = null

// ── Apple 风格色板 ──
const colorPalette = [
  '#007AFF', // 蓝色
  '#34C759', // 绿色
  '#FF9F0A', // 橙色
  '#FF375F', // 玫红
  '#5E5CE6', // 紫色
  '#64D2FF', // 天蓝
  '#BF5AF2', // 洋红
  '#00C7BE', // 青绿
]

function pickColor(node: INode): string {
  const depth = node.state.depth
  if (depth === 0) return '#1d1d1f'
  // 根据内容和深度算出色板索引，保证同节点颜色一致
  let hash = 0
  for (const ch of node.content) hash = ((hash << 5) - hash) + ch.charCodeAt(0)
  const idx = (Math.abs(hash) + depth) % colorPalette.length
  return colorPalette[idx]
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
      duration: 0,                // 无动画，即刻渲染
      initialExpandLevel: 999,    // 全部展开
      maxInitialScale: 1.2,
      pan: true,
      zoom: true,
      scrollForPan: true,
      toggleRecursively: true,
      color: pickColor,
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

onMounted(() => {
  if (hasData.value) void render()
})

onBeforeUnmount(() => {
  mm?.destroy()
  mm = null
})

async function exportPNG() {
  if (!svgRef.value || !mm) return
  const svgData = new XMLSerializer().serializeToString(svgRef.value)
  const svgBlob = new Blob([svgData], { type: 'image/svg+xml;charset=utf-8' })
  const url = URL.createObjectURL(svgBlob)
  const img = new Image()
  img.onload = () => {
    const canvas = document.createElement('canvas')
    const rect = svgRef.value!.getBoundingClientRect()
    const scale = 2
    canvas.width = rect.width * scale || 1200
    canvas.height = rect.height * scale || 800
    const ctx = canvas.getContext('2d')!
    ctx.scale(scale, scale)
    ctx.fillStyle = '#ffffff'
    ctx.fillRect(0, 0, canvas.width, canvas.height)
    ctx.drawImage(img, 0, 0)
    canvas.toBlob((blob) => {
      if (!blob) return
      const a = document.createElement('a')
      a.href = URL.createObjectURL(blob)
      a.download = 'mindmap.png'
      a.click()
      URL.revokeObjectURL(a.href)
    }, 'image/png')
    URL.revokeObjectURL(url)
  }
  img.src = url
}

function exportSVG() {
  if (!svgRef.value) return
  const svgData = new XMLSerializer().serializeToString(svgRef.value)
  const blob = new Blob([svgData], { type: 'image/svg+xml;charset=utf-8' })
  const a = document.createElement('a')
  a.href = URL.createObjectURL(blob)
  a.download = 'mindmap.svg'
  a.click()
  URL.revokeObjectURL(a.href)
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
  background: var(--card-bg, #ffffff);
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
  background: var(--card-body-bg, #fafafa);
}

.mm-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted, #999);
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

/* ── 弹窗淡入动画 ── */
.mm-fade-enter-active {
  transition: opacity 0.2s ease;
}
.mm-fade-enter-active .mm-card {
  transition: transform 0.2s ease, opacity 0.2s ease;
}
.mm-fade-leave-active {
  transition: opacity 0.15s ease;
}
.mm-fade-enter-from {
  opacity: 0;
}
.mm-fade-enter-from .mm-card {
  opacity: 0;
  transform: scale(0.97);
}
.mm-fade-leave-to {
  opacity: 0;
}

/* Dark theme adjustments */
:root[data-theme="dark"] .mm-overlay {
  background: rgba(0, 0, 0, 0.5);
}
</style>

<!-- 非 scoped 样式，用于 markmap SVG 内部元素 -->
<style>
.mm-svg text {
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', sans-serif;
  font-size: 13px;
}

.mm-svg .markmap-node circle {
  stroke-width: 2;
}

.mm-svg .markmap-link {
  stroke-width: 1.5;
}
</style>
