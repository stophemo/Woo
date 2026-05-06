/**
 * 通用工具：ID 生成、SHA-256、HTML 处理。
 */
const crypto = require('crypto')

function newId() {
  // 16 字节随机 ID（十六进制），足够本地使用
  return crypto.randomBytes(12).toString('hex')
}

function sha256(text) {
  return crypto.createHash('sha256').update(text || '', 'utf8').digest('hex')
}

function stripHtml(content) {
  if (!content) return ''
  return String(content)
    .replace(/<\s*br\s*\/?\s*>/gi, ' ')
    .replace(/<\/\s*(p|div|h[1-6]|li|blockquote|pre)\s*>/gi, ' ')
    .replace(/<[^>]+>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/\s+/g, ' ')
    .trim()
}

function stripHtmlKeepLines(content) {
  if (!content) return ''
  return String(content)
    .replace(/<\s*br\s*\/?\s*>/gi, '\n')
    .replace(/<\/\s*(p|div|h[1-6]|li|blockquote|pre)\s*>/gi, '\n')
    .replace(/<[^>]+>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
}

function nowStr() {
  const d = new Date()
  const pad = (n) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ` +
         `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

module.exports = { newId, sha256, stripHtml, stripHtmlKeepLines, nowStr }
