/**
 * 知识库服务（RAG + 向量嵌入）
 *
 * 使用 sqlite-vec (vec0) 存储和搜索向量。
 *
 * 搜索策略：
 *   - 向量搜索：query → 嵌入 → vec0 MATCH → top K
 *   - 回退：FTS5 关键词 → LIKE 模糊
 */
const { getDb } = require('../db/index.cjs')
const crypto = require('crypto')
const { generateEmbedding } = require('./embeddingService.cjs')

function uuidv4() { return crypto.randomUUID() }

/** 将 Float32Array 转为 JSON 数组字符串（vec0 接受格式） */
function vecToJson(vec) {
  return '[' + Array.from(vec).join(',') + ']'
}

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

/** 去除 HTML 标签 */
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
 * 重建知识库：分块 → 向量嵌入 → kb_chunks + kb_vectors(vec0) + FTS5
 */
async function rebuild() {
  const db = getDb()
  if (!db) throw new Error('数据库未初始化')

  // 清空旧数据
  db.exec('DELETE FROM kb_vectors')
  db.exec('DELETE FROM kb_chunks')
  try { db.exec('DELETE FROM kb_chunks_fts') } catch {}

  const docs = db.prepare(
    `SELECT id, title, content FROM note_document WHERE deleted = 0 AND content IS NOT NULL AND content != ''`
  ).all()

  let totalChunks = 0

  const insertChunk = db.prepare(
    `INSERT INTO kb_chunks (id, document_id, document_title, chunk_index, content)
     VALUES (?, ?, ?, ?, ?)`
  )
  const insertVec = db.prepare(
    `INSERT INTO kb_vectors (id, embedding) VALUES (?, ?)`
  )
  const insertFts = db.prepare(
    `INSERT INTO kb_chunks_fts (rowid, content, title) VALUES (?, ?, ?)`
  )

  for (const doc of docs) {
    const plainText = stripHtml(doc.content)
    const chunks = chunkText(plainText, doc.title || '未命名')

    for (const chunk of chunks) {
      const id = uuidv4()

      // 1. 写入 kb_chunks
      const info = insertChunk.run(id, doc.id, chunk.title, chunk.chunkIndex, chunk.content)
      const rowid = info.lastInsertRowid

      // 2. 生成向量 → 写入 kb_vectors (vec0)
      try {
        const vec = await generateEmbedding(chunk.content)
        insertVec.run(rowid, vecToJson(vec))
      } catch (e) {
        console.warn('[KB] 嵌入失败:', chunk.title, e.message)
      }

      // 3. 写入 FTS5
      insertFts.run(rowid, chunk.content, chunk.title)

      totalChunks++
    }
  }

  return { totalDocs: docs.length, totalChunks }
}

/**
 * 混合搜索：vec0 语义搜索 → FTS5 回退。
 */
async function search(query, limit = 6) {
  const db = getDb()
  if (!db) throw new Error('数据库未初始化')
  if (!query || !query.trim()) return []

  // 1. 向量搜索（主路径）
  try {
    const queryVec = await generateEmbedding(query.trim().slice(0, 200))
    const vecJson = vecToJson(queryVec)

    // vec0 的 KNN 搜索，返回 distance（L2，归一化后等价于余弦距离: 0=最相似）
    const vecResults = db.prepare(`
      SELECT v.id, v.distance
      FROM kb_vectors v
      WHERE v.embedding MATCH ?
      AND v.k = ?
      ORDER BY v.distance
    `).all(vecJson, limit)

    if (vecResults.length > 0) {
      const ids = vecResults.map(r => r.id)
      const placeholders = ids.map(() => '?').join(',')
      const chunks = db.prepare(`
        SELECT rowid, id, document_id, document_title, chunk_index, content
        FROM kb_chunks
        WHERE rowid IN (${placeholders})
      `).all(...ids)

      // 按 vec0 返回的 id(rowid) 顺序排列
      const chunkMap = {}
      for (const c of chunks) chunkMap[c.rowid] = c
      return ids.map(rowid => {
        const c = chunkMap[rowid]
        return c ? {
          id: c.id,
          document_id: c.document_id,
          document_title: c.document_title,
          chunk_index: c.chunk_index,
          content: c.content,
        } : null
      }).filter(Boolean)
    }
  } catch (e) {
    console.warn('[KB] 向量搜索失败，回退 FTS5:', e.message)
  }

  // 2. 回退：FTS5
  return searchFallback(db, query.trim().slice(0, 200), limit)
}

/** FTS5 回退搜索 */
function searchFallback(db, query, limit) {
  try {
    const results = db.prepare(`
      SELECT c.id, c.document_id, c.document_title, c.chunk_index, c.content
      FROM kb_chunks_fts f
      JOIN kb_chunks c ON c.rowid = f.rowid
      WHERE kb_chunks_fts MATCH ?
      ORDER BY rank
      LIMIT ?
    `).all(query, limit)
    if (results.length > 0) return results
  } catch {}

  const likePattern = `%${query}%`
  return db.prepare(`
    SELECT id, document_id, document_title, chunk_index, content
    FROM kb_chunks WHERE content LIKE ?
    ORDER BY chunk_index LIMIT ?
  `).all(likePattern, limit)
}

/** 知识库状态 */
function status() {
  const db = getDb()
  if (!db) return { docCount: 0, chunkCount: 0, embedCount: 0 }

  const docCount = db.prepare(
    `SELECT COUNT(DISTINCT document_id) AS count FROM kb_chunks`
  ).get()?.count || 0

  const chunkCount = db.prepare(
    `SELECT COUNT(*) AS count FROM kb_chunks`
  ).get()?.count || 0

  const embedCount = db.prepare(
    `SELECT COUNT(*) AS count FROM kb_vectors`
  ).get()?.count || 0

  return { docCount, chunkCount, embedCount, hasEmbedding: embedCount > 0 }
}

module.exports = { rebuild, search, status }
