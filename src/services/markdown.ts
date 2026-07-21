import { marked } from 'marked'
import TurndownService from 'turndown'
import { tables as gfmTable, strikethrough as gfmStrikethrough } from 'turndown-plugin-gfm'

const MARKDOWN_EXTENSIONS = new Set(['md', 'markdown', 'mdown', 'mkd'])

const turndown = new TurndownService({
  headingStyle: 'atx',
  bulletListMarker: '-',
  codeBlockStyle: 'fenced',
  emDelimiter: '*',
  strongDelimiter: '**',
})

turndown.use([gfmTable, gfmStrikethrough])
turndown.addRule('taskList', {
  filter: (node: HTMLElement) =>
    node.nodeName === 'LI' &&
    (node.getAttribute('data-checked') !== null ||
      node.parentElement?.getAttribute('data-type') === 'taskList'),
  replacement: (content: string, node: HTMLElement) => {
    const checked = node.getAttribute('data-checked') === 'true'
    return `- [${checked ? 'x' : ' '}] ${content.trim()}\n`
  },
})

export function isMarkdownFilePath(filePath: string): boolean {
  const extension = filePath.split('.').pop()?.toLowerCase()
  return extension !== undefined && MARKDOWN_EXTENSIONS.has(extension)
}

function normalizeTaskListHtml(html: string): string {
  if (!html.includes('type="checkbox"')) return html

  const template = document.createElement('template')
  template.innerHTML = html

  template.content.querySelectorAll('ul').forEach((list) => {
    const items = Array.from(list.children).filter(
      (child): child is HTMLLIElement => child instanceof HTMLLIElement
    )
    const hasTaskItem = items.some((item) =>
      item.firstElementChild?.matches('input[type="checkbox"]')
    )
    if (!hasTaskItem) return

    list.setAttribute('data-type', 'taskList')
    items.forEach((item) => {
      const checkbox = item.firstElementChild
      const checked = checkbox?.matches('input[type="checkbox"]') === true
        && (checkbox as HTMLInputElement).checked

      item.setAttribute('data-type', 'taskItem')
      item.setAttribute('data-checked', checked ? 'true' : 'false')
      if (checkbox?.matches('input[type="checkbox"]')) checkbox.remove()
      if (item.firstChild?.nodeType === Node.TEXT_NODE && item.firstChild.textContent) {
        item.firstChild.textContent = item.firstChild.textContent.replace(/^\s+/, '')
      }
    })
  })

  return template.innerHTML
}

export function markdownToEditorHtml(markdown: string): string {
  const source = markdown.replace(/^\uFEFF/, '')
  // 保留 Markdown 的软换行，避免加载后编辑再保存时丢失原文换行。
  const html = marked.parse(source, { gfm: true, breaks: true }) as string
  return normalizeTaskListHtml(html)
}

export function editorHtmlToMarkdown(html: string): string {
  return turndown.turndown(html || '')
}

export function plainTextToEditorHtml(text: string): string {
  const escaped = text
    .replace(/^\uFEFF/, '')
    .replace(/\r\n?/g, '\n')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
  return escaped ? `<p>${escaped.replace(/\n/g, '<br>')}</p>` : ''
}

const BLOCK_TEXT_ELEMENTS = new Set([
  'ADDRESS', 'ARTICLE', 'ASIDE', 'BLOCKQUOTE', 'DIV', 'FIGCAPTION', 'FIGURE',
  'FOOTER', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6', 'HEADER', 'LI', 'MAIN',
  'NAV', 'P', 'PRE', 'SECTION', 'TR',
])

function extractPlainText(node: Node): string {
  if (node.nodeType === Node.TEXT_NODE) return node.textContent || ''
  if (!(node instanceof HTMLElement) && !(node instanceof DocumentFragment)) return ''
  if (node instanceof HTMLBRElement) return '\n'

  let text = Array.from(node.childNodes).map(extractPlainText).join('')
  if (node instanceof HTMLTableCellElement && node.nextElementSibling) text += '\t'
  if (node instanceof HTMLElement && BLOCK_TEXT_ELEMENTS.has(node.tagName) && !text.endsWith('\n')) {
    text += '\n'
  }
  return text
}

export function editorHtmlToPlainText(html: string): string {
  const template = document.createElement('template')
  template.innerHTML = html || ''
  return extractPlainText(template.content)
    .replace(/\u00a0/g, ' ')
    .replace(/\t+\n/g, '\n')
    .replace(/\n+$/, '')
}
