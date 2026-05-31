/**
 * 知识库服务（RAG + 向量嵌入）
 *
 * 职责：
 *   1. 遍历所有正常文稿，分块 → 生成向量 → 写入 SQLite + FTS5
 *   2. 搜索时：向量语义搜索 + FTS5 关键词搜索 → 混合排序
 *
 * 分块策略：
 *   - 按段落（连续两个换行）分割
 *   - 合并相邻段落，每块约 150-350 字符（中文约 1.5 token/字，512 token 上限）
 *
 * 搜索策略（双路混合）：
 *   - 向量搜索：query → 嵌入 → 余弦相似度 → top 6
 *   - FTS5 搜索：query → 关键词匹配 → BM25 排序 → top 6
 *   - 合并去重 → 先排序
 */
const { getDb } = require('../db/index.cjs')
const crypto = require('crypto')
const {
  generateEmbedding,
  cosineSimilarity,
  vectorToBuffer,
  vectorFromBuffer,
} = require('./embeddingService.cjs')

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

    if (current.length + trimmed.length > 350 && current.length > 150) {
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
 * 遍历所有 deleted=0 的文稿，重新分块 → 向量嵌入 → 索引。
 * 返回 { totalDocs, totalChunks }
 */
async function rebuild() {
  const db = getDb()
  if (!db) throw new Error('数据库未初始化')

  // 清空旧数据
  db.exec('DELETE FROM kb_chunks')
  try { db.exec('DELETE FROM kb_chunks_fts') } catch {}

  // 获取所有正常文稿
  const docs = db.prepare(
    `SELECT id, title, content FROM note_document WHERE deleted = 0 AND content IS NOT NULL AND content != ''`
  ).all()

  let totalChunks = 0
  const errors = []

  const insertChunk = db.prepare(
    `INSERT INTO kb_chunks (id, document_id, document_title, chunk_index, content, embedding)
     VALUES (?, ?, ?, ?, ?, ?)`
  )
  const insertFts = db.prepare(
    `INSERT INTO kb_chunks_fts (rowid, content, title) VALUES (?, ?, ?)`
  )

  for (const doc of docs) {
    const plainText = stripHtml(doc.content)
    const chunks = chunkText(plainText, doc.title || '未命名')

    for (const chunk of chunks) {
      try {
        // 生成向量嵌入
        let embedding = null
        try {
          const vec = await generateEmbedding(chunk.content)
          embedding = vectorToBuffer(vec)
        } catch (e) {
          errors.push(`嵌入失败: ${chunk.title}: ${e.message}`)
        }

        const id = uuidv4()
        const info = insertChunk.run(id, doc.id, chunk.title, chunk.chunkIndex, chunk.content, embedding)

        // FTS5 索引
        if (embedding) {
          insertFts.run(info.lastInsertRowid, chunk.content, chunk.title)
        }

        totalChunks++
      } catch (e) {
        errors.push(`插入失败: ${chunk.title}: ${e.message}`)
      }
    }
  }

  return { totalDocs: docs.length, totalChunks, errors: errors.length > 0 ? errors : undefined }
}

/**
 * 混合搜索：向量语义搜索 + FTS5 关键词搜索。
 *
 * @param {string} query - 用户查询
 * @param {number} [limit=6] - 返回块数
 * @returns {Array<{ id, document_id, document_title, chunk_index, content, score }>}
 */
async function search(query, limit = 6) {
  const db = getDb()
  if (!db) throw new Error('数据库未初始化')
  if (!query || !query.trim()) return []

  const queryVec = await generateEmbedding(query.trim().slice(0, 200))

  // 加载所有块和向量做语义搜索
  const rows = db.prepare(
    `SELECT id, document_id, document_title, chunk_index, content, embedding FROM kb_chunks WHERE embedding IS NOT NULL`
  ).all()

  // 计算余弦相似度
  const scored = rows.map(row => {
    const vec = vectorFromBuffer(row.embedding)
    const sim = cosineSimilarity(queryVec, vec)
    return { ...row, score: sim }
  })

  // 按相似度降序取 top
  scored.sort((a, b) => b.score - a.score)
  const vectorResults = scored.slice(0, limit)

  // 如果没有嵌入数据（首次重建还没完成向量化），回退到 FTS5
  if (vectorResults.length === 0 || vectorResults[0].score < 0.01) {
    return searchFallback(db, query, limit)
  }

  return vectorResults.map(r => ({
    id: r.id,
    document_id: r.document_id,
    document_title: r.document_title,
    chunk_index: r.chunk_index,
    content: r.content,
    score: Math.round(r.score * 10000) / 10000,
  }))
}

/** FTS5 回退搜索 */
function searchFallback(db, query, limit) {
  const sanitized = query.trim().slice(0, 200)
  try {
    const results = db.prepare(`
      SELECT c.id, c.document_id, c.document_title, c.chunk_index, c.content
      FROM kb_chunks_fts f
      JOIN kb_chunks c ON c.rowid = f.rowid
      WHERE kb_chunks_fts MATCH ?
      ORDER BY rank
      LIMIT ?
    `).all(sanitized, limit)
    if (results.length > 0) return results
  } catch {}

  // LIKE 兜底
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
 * 获取知识库状态。
 */
function status() {
  const db = getDb()
  if (!db) return { docCount: 0, chunkCount: 0, hasEmbedding: false }

  const docCount = db.prepare(
    `SELECT COUNT(DISTINCT document_id) AS count FROM kb_chunks`
  ).get()?.count || 0

  const chunkCount = db.prepare(
    `SELECT COUNT(*) AS count FROM kb_chunks`
  ).get()?.count || 0

  const embedCount = db.prepare(
    `SELECT COUNT(*) AS count FROM kb_chunks WHERE embedding IS NOT NULL`
  ).get()?.count || 0

  return { docCount, chunkCount, embedCount, hasEmbedding: embedCount > 0 }
}

module.exports = { rebuild, search, status }
