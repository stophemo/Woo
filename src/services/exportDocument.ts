/**
 * 文稿导出服务（桌面端）
 *
 * 统一处理 Markdown / TXT / 图片(PNG) 三种格式，供两个入口共用：
 *   - 缩略图右键菜单「导出」（exportDocumentInteractive）
 *   - 顶部「文件 → 导出」子菜单（exportDocumentAs）
 *
 * 流程：生成内容 → dialog:save-document 选路径 → file:write 落盘。
 * 图片以 base64 传给后端，由 Rust 端 file_write 解码写字节。
 */
import TurndownService from 'turndown'
import { tables as gfmTable, strikethrough as gfmStrikethrough } from 'turndown-plugin-gfm'
import html2canvas from 'html2canvas'
import { invoke } from './api'
import { log } from './logger'
import type { Document } from '../types/document'

export type ExportFormat = 'markdown' | 'txt' | 'image'

interface FormatMeta {
  ext: string
  filterName: string
}

const FORMAT_META: Record<ExportFormat, FormatMeta> = {
  markdown: { ext: 'md', filterName: 'Markdown 文件' },
  txt: { ext: 'txt', filterName: '文本文件' },
  image: { ext: 'png', filterName: '图片文件' },
}

// ============ Markdown ============
// 与 EditArea / 原 ThumbnailColumn 中的 turndown 配置保持一致
const turndown = new TurndownService({
  headingStyle: 'atx',       // ## 标题
  codeBlockStyle: 'fenced',  // ``` 代码块
  emDelimiter: '*',          // *斜体*
  bulletListMarker: '-',     // - 无序列表
})
turndown.use(gfmTable)
turndown.use(gfmStrikethrough)
// 任务列表（- [x] / - [ ]）
turndown.addRule('taskList', {
  filter: (node: HTMLElement) =>
    node.nodeName === 'LI' &&
    (node.getAttribute('data-checked') !== null ||
     (!!node.parentElement && node.parentElement.getAttribute('data-type') === 'taskList')),
  replacement: (content: string, node: HTMLElement) => {
    const checked = node.getAttribute('data-checked') === 'true'
    return `- [${checked ? 'x' : ' '}] ${content.trim()}\n`
  },
})

export function htmlToMarkdown(html: string): string {
  return turndown.turndown(html || '')
}

// ============ 纯文本 ============
export function htmlToPlainText(html: string): string {
  const tmp = document.createElement('div')
  tmp.innerHTML = html || ''
  // <br> → 换行；块级元素末尾补换行，避免所有文字挤成一行
  tmp.querySelectorAll('br').forEach((br) => br.replaceWith('\n'))
  tmp
    .querySelectorAll('p,div,h1,h2,h3,h4,h5,h6,li,tr,blockquote,pre')
    .forEach((el) => el.append('\n'))
  const text = tmp.textContent || ''
  return text.replace(/\n{3,}/g, '\n\n').trim()
}

// ============ 渲染为 canvas（图片导出用）============
async function renderHtmlToCanvas(html: string): Promise<HTMLCanvasElement> {
  const container = document.createElement('div')
  container.style.cssText = [
    'position:absolute',
    'left:-99999px',
    'top:0',
    'width:794px',            // A4 宽度 @96dpi
    'padding:40px',
    'box-sizing:border-box',
    'background:#ffffff',
    'color:#1a1a1a',
    'font-family:-apple-system,"Microsoft YaHei",sans-serif',
    'font-size:15px',
    'line-height:1.7',
  ].join(';')
  container.innerHTML = html || '<p></p>'
  document.body.appendChild(container)
  try {
    // 按内容尺寸回退 scale：避免超长文稿在 scale:2 下生成超出 webview
    // canvas 单边上限的空白/截断画布（WebKit 上限较低，保守取 8192）
    const MAX_DIM = 8192
    const longest = Math.max(container.scrollHeight, container.scrollWidth) || 1
    const scale = Math.min(2, Math.max(1, MAX_DIM / longest))
    return await html2canvas(container, {
      scale,
      backgroundColor: '#ffffff',
      useCORS: true,
      logging: false,
    })
  } finally {
    document.body.removeChild(container)
  }
}

/** 从 dataURL 中截取纯 base64 段（去掉 data:...;base64, 前缀） */
function dataUrlToBase64(dataUrl: string): string {
  const marker = 'base64,'
  const i = dataUrl.indexOf(marker)
  // 正常渲染必带 base64 前缀；走到这里说明产出了非法/空 dataURL
  // （多因内容过高、canvas 超出 webview 尺寸上限）——显式报错，避免静默写出损坏文件
  if (i < 0) throw new Error('导出内容过大，画布渲染失败')
  return dataUrl.slice(i + marker.length)
}

async function htmlToPngBase64(html: string): Promise<string> {
  const canvas = await renderHtmlToCanvas(html)
  return dataUrlToBase64(canvas.toDataURL('image/png'))
}

// ============ 内容生成 ============
async function buildPayload(
  html: string,
  format: ExportFormat,
): Promise<{ data: string; isBase64: boolean }> {
  switch (format) {
    case 'markdown':
      return { data: htmlToMarkdown(html), isBase64: false }
    case 'txt':
      return { data: htmlToPlainText(html), isBase64: false }
    case 'image':
      return { data: await htmlToPngBase64(html), isBase64: true }
  }
}

function detectFormat(filePath: string): ExportFormat {
  const lower = filePath.toLowerCase()
  if (lower.endsWith('.md') || lower.endsWith('.markdown')) return 'markdown'
  if (lower.endsWith('.txt')) return 'txt'
  return 'image' // png / jpg / webp 等按图片处理
}

function toast(message: string, type: 'success' | 'error' = 'success') {
  window.dispatchEvent(new CustomEvent('sync:toast', { detail: { message, type } }))
}

/** 校验文档可导出，返回 { title, html }；不可导出返回 null 并提示 */
function resolveDoc(doc: Document | null | undefined): { title: string; html: string } | null {
  if (!doc) {
    toast('请先打开一篇文稿', 'error')
    return null
  }
  // 加锁短路：右键菜单传入的列表项携带明文 content（仅 currentDocument 会脱敏），
  // 必须在此拦截，否则加锁文稿会绕过密码锁被导出为明文。
  if (doc.isLocked) {
    toast('已加锁文稿无法导出，请先解锁', 'error')
    return null
  }
  const html = doc.content || ''
  if (!html.trim()) {
    toast('文稿内容为空', 'error')
    return null
  }
  return { title: doc.title || '未命名文稿', html }
}

async function writeExport(filePath: string, format: ExportFormat, html: string): Promise<void> {
  try {
    const { data, isBase64 } = await buildPayload(html, format)
    await invoke('file:write', { filePath, data, isBase64 })
    toast('导出成功')
  } catch (e: any) {
    log.app.error('[Export] 导出失败:', e?.message || e)
    toast(`导出失败: ${e?.message || e}`, 'error')
  }
}

/** 指定格式导出（顶部「文件 → 导出」用）。 */
export async function exportDocumentAs(
  doc: Document | null | undefined,
  format: ExportFormat,
): Promise<void> {
  const resolved = resolveDoc(doc)
  if (!resolved) return
  const meta = FORMAT_META[format]
  const result = await invoke<{ filePath: string }>('dialog:save-document', {
    defaultName: `${resolved.title}.${meta.ext}`,
    filters: [{ name: meta.filterName, extensions: [meta.ext] }],
  })
  if (!result?.filePath) return
  await writeExport(result.filePath, format, resolved.html)
}

/** 交互式导出：单个保存框列出全部格式，按所选扩展名分发（右键菜单用）。 */
export async function exportDocumentInteractive(doc: Document | null | undefined): Promise<void> {
  const resolved = resolveDoc(doc)
  if (!resolved) return
  const result = await invoke<{ filePath: string }>('dialog:save-document', {
    defaultName: `${resolved.title}.md`,
    filters: [
      { name: 'Markdown 文件', extensions: ['md'] },
      { name: '文本文件', extensions: ['txt'] },
      { name: '图片文件', extensions: ['png'] },
    ],
  })
  if (!result?.filePath) return
  await writeExport(result.filePath, detectFormat(result.filePath), resolved.html)
}
