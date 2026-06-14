/**
 * 通用工具函数。
 * 供 electron/services 下的服务文件使用。
 */
const crypto = require('crypto')

/**
 * 生成唯一 ID（UUID v4）
 */
function newId() {
  return crypto.randomUUID()
}

/**
 * 返回当前北京时间字符串 (UTC+8)
 * 格式：YYYY-MM-DDTHH:MM:SS（北京时间，与 schema update_time 默认值一致）
 */
function nowStr() {
  // sv-SE locale 输出 "yyyy-MM-dd HH:mm:ss"，替换空格为 T 即为 target 格式
  return new Date().toLocaleString('sv-SE', { timeZone: 'Asia/Shanghai' }).replace(' ', 'T')
}

/**
 * SHA-256 哈希
 * @param {string} content
 * @returns {string} hex string
 */
function sha256(content) {
  return crypto.createHash('sha256').update(String(content || '')).digest('hex')
}

/**
 * 去除 HTML 标签，保留文本
 * @param {string} html
 * @returns {string}
 */
function stripHtml(html) {
  if (!html) return ''
  return String(html).replace(/<[^>]*>/g, '').replace(/&nbsp;/g, ' ').replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').trim()
}

/**
 * 去除 HTML 标签，但保留换行符（将块级标签替换为 \n）
 * @param {string} html
 * @returns {string}
 */
function stripHtmlKeepLines(html) {
  if (!html) return ''
  // 先替换块级标签为换行，再去除其余标签
  let text = String(html)
    .replace(/<\/?(div|p|br|h[1-6]|li|tr|blockquote|section|article|header|footer)[^>]*>/gi, '\n')
    .replace(/<[^>]*>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
  // 合并连续换行
  text = text.replace(/\n{3,}/g, '\n\n')
  return text.trim()
}

module.exports = { newId, nowStr, sha256, stripHtml, stripHtmlKeepLines }
