/**
 * 知识库服务（RAG）
 *
 * 职责：
 *   1. 遍历所有正常文稿，分块后写入 kb_chunks 表并更新 FTS5 索引
 *   2. 根据用户查询在 FTS5 中检索相关块
 *
 * 分块策略：
 *   - 按段落（连续两个换行）分割
 *   - 合并相邻段落，每块约 400-800 字符
 *   - 保留文档标题作为上下文
 */
const { getDb } = require('../db/index.cjs')
const crypto = require('crypto')

function uuidv4() { return crypto.randomUUID() }

/** 将 HTML 纯文本分割为块 */
function chunkText(text, title) {
  if (!text || !text.trim()) return []
  const paragraphs = text.split(/\n\s*\n/).filter(p => p.trim())
  const chunks = []
  let current = ''
  let idx = 0

  for (const para of paragraphs) {
    const trimmed = para.trim()
    if (!trimmed) continue

    if (current.length + trimmed.length > 800 && current.length > 300) {
      chunks.push({ title, content: current.trim(), chunkIndex: idx++ })
      current = trimmed
    } else {
      current += (current ? '\n\n' : '') + trimmed
    }
  }

  if (current.trim()) {
    chunks.push({ title, content: current.trim(), chunkIndex: idx })
  }

  return chunks
}

/** 去除 HTML 标签，提取纯文本 */
function stripHtml(html) {
  if (!html) return ''
  return html
    .replace(/<[^>]+>/g, ' ')
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/\s+/g, ' ')
    .trim()
}

/**
 * 重建完整知识库。
 * 遍历所有 deleted=0 的文稿，重新分块索引。
 * 返回 { totalDocs, totalChunks }
 */
function rebuild() {
  const db = getDb()
  if (!db) throw new Error('数据库未初始化')

  // 清空旧数据
  db.exec('DELETE FROM kb_chunks_fts')
  db.exec('DELETE FROM kb_chunks')

  // 获取所有正常文稿
  const docs = db.prepare(
    `SELECT id, title, content FROM note_document WHERE deleted = 0 AND content IS NOT NULL AND content != ''`
  ).all()

  let totalChunks = 0

  const insertChunk = db.prepare(
    `INSERT INTO kb_chunks (id, document_id, document_title, chunk_index, content)
     VALUES (?, ?, ?, ?, ?)`
  )
  const insertFts = db.prepare(
    `INSERT INTO kb_chunks_fts (rowid, content, title) VALUES (?, ?, ?)`
  )

  const trx = db.transaction(() => {
    for (const doc of docs) {
      const plainText = stripHtml(doc.content)
      const chunks = chunkText(plainText, doc.title || '未命名')

      for (const chunk of chunks) {
        const id = uuidv4()
        const info = insertChunk.run(id, doc.id, chunk.title, chunk.chunkIndex, chunk.content)
        // FTS5 content-sync: 插入 FTS 索引（rowid 与主表 rowid 一致）
        insertFts.run(info.lastInsertRowid, chunk.content, chunk.title)
        totalChunks++
      }
    }
  })

  trx()

  return { totalDocs: docs.length, totalChunks }
}

/**
 * 在知识库中搜索与查询最相关的块。
 *
 * @param {string} query - 用户查询文本
 * @param {number} [limit=8] - 最多返回块数
 * @returns {Array<{ id, document_id, document_title, chunk_index, content }>}
 */
function search(query, limit = 8) {
  const db = getDb()
  if (!db) throw new Error('数据库未初始化')

  if (!query || !query.trim()) return []

  // 对查询做简单预处理：去除非关键词字符
  const sanitized = query.trim().slice(0, 200)

  // FTS5 搜索（按 BM25 排序）
  try {
    const results = db.prepare(`
      SELECT c.id, c.document_id, c.document_title, c.chunk_index, c.content,
             rank AS score
      FROM kb_chunks_fts f
      JOIN kb_chunks c ON c.rowid = f.rowid
      WHERE kb_chunks_fts MATCH ?
      ORDER BY rank
      LIMIT ?
    `).all(sanitized, limit)

    // 如果 FTS5 有结果，直接返回
    if (results.length > 0) return results
  } catch {
    // FTS5 可能对某些查询抛出语法错误（如标点），回退到 LIKE
  }

  // 回退：LIKE 模糊搜索
  const likePattern = `%${sanitized}%`
  return db.prepare(`
    SELECT id, document_id, document_title, chunk_index, content
    FROM kb_chunks
    WHERE content LIKE ?
    ORDER BY chunk_index
    LIMIT ?
  `).all(likePattern, limit)
}

/**
 * 获取知识库状态（文档数和块数）。
 */
function status() {
  const db = getDb()
  if (!db) return { docCount: 0, chunkCount: 0 }

  const docCount = db.prepare(
    `SELECT COUNT(DISTINCT document_id) AS count FROM kb_chunks`
  ).get()?.count || 0

  const chunkCount = db.prepare(
    `SELECT COUNT(*) AS count FROM kb_chunks`
  ).get()?.count || 0

  return { docCount, chunkCount }
}

module.exports = { rebuild, search, status }
