/**
 * 知识库服务（RAG + 向量嵌入）
 *
 * 使用 sqlite-vec (vec0) 存储和搜索向量。
 * 搜索方式：query → 嵌入 → vec0 KNN → top K
 * 无降级，失败即报错。
 */
const { getDb } = require('../db/index.cjs')
const crypto = require('crypto')
const { generateEmbedding } = require('./embeddingService.cjs')

function uuidv4() { return crypto.randomUUID() }

/** 将 Float32Array 转为 JSON 数组字符串（vec0 接受格式） */
function vecToJson(vec) {
  return '[' + Array.from(vec).join(',') + ']'
}

/** 将纯文本按段落分割为 150~350 字的块 */
function chunkText(text, title) {
  if (!text || !text.trim()) return []
  const paragraphs = text.split(/\n\s*\n/).filter(p => p.trim())
  const chunks = []
  let current = ''
  let idx = 0

  for (const para of paragraphs) {
    const trimmed = para.trim()
    if (!trimmed) continue

    // 当前块已够大 → 切分
    if (current.length + trimmed.length > 350 && current.length > 150) {
      chunks.push({ title, content: current.trim(), chunkIndex: idx++ })
      current = trimmed
    } else {
      current += (current ? '\n\n' : '') + trimmed
    }
  }

  // 处理最后一块
  if (current.trim()) {
    // 若最后一块 < 100 字且前面有块，合并到前一块
    if (current.trim().length < 100 && chunks.length > 0) {
      chunks[chunks.length - 1].content += '\n\n' + current.trim()
    } else {
      chunks.push({ title, content: current.trim(), chunkIndex: idx })
    }
  }
  return chunks
}

/** 去除 HTML 标签，保留段落边界 */
function stripHtml(html) {
  if (!html) return ''
  return html
    // 块级标签替换为双换行（确保段落间有空行，chunkText 靠 \n\n 分段落）
    .replace(/<\/(p|div|h[1-6]|li|blockquote|section|article|pre)>/gi, '\n\n')
    .replace(/<(br|hr)\s*\/?>/gi, '\n\n')
    .replace(/<\/(tr|table|thead|tbody|tfoot)>/gi, '\n\n')
    .replace(/<[^>]+>/g, ' ')
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/[ \t]+/g, ' ')
    .replace(/\n{4,}/g, '\n\n\n')
    .trim()
}

/**
 * 重建知识库：分块 → 向量嵌入 → kb_chunks + kb_vectors(vec0)
 */
async function rebuild() {
  const db = getDb()
  if (!db) throw new Error('数据库未初始化')

  // 清空旧数据
  db.exec('DELETE FROM kb_vectors')
  db.exec('DELETE FROM kb_chunks')

  const docs = db.prepare(
    `SELECT id, title, content FROM note_document WHERE deleted = 0 AND content IS NOT NULL AND content != ''`
  ).all()

  let totalChunks = 0
  const errors = []
  let embedSuccess = 0
  let embedFail = 0
  let seqId = 0

  const insertChunk = db.prepare(
    `INSERT INTO kb_chunks (id, document_id, document_title, chunk_index, content)
     VALUES (?, ?, ?, ?, ?)`
  )
  const insertVec = db.prepare(
    `INSERT INTO kb_vectors (id, embedding) VALUES (CAST(? AS INTEGER), ?)`
  )

  for (const doc of docs) {
    const plainText = stripHtml(doc.content)
    const chunks = chunkText(plainText, doc.title || '未命名')

    for (const chunk of chunks) {
      const id = uuidv4()
      seqId++

      // 1. 写入 kb_chunks
      insertChunk.run(id, doc.id, chunk.title, chunk.chunkIndex, chunk.content)

      // 2. 生成向量 → 写入 kb_vectors (vec0)
      try {
        console.log('[KB] 嵌入块:', chunk.title, `(${chunk.content.length}字)`)
        const vec = await generateEmbedding(chunk.content)
        insertVec.run(seqId, vecToJson(vec))
        embedSuccess++
      } catch (e) {
        embedFail++
        const msg = `${chunk.title}: ${e.message || e}`
        console.error('[KB] 嵌入失败:', msg)
        errors.push(msg)
      }

      totalChunks++
    }
  }

  console.log(`[KB] 重建完成: ${totalChunks}块, 嵌入成功${embedSuccess}, 失败${embedFail}`)
  if (errors.length > 0) {
    console.error('[KB] 嵌入失败详情:', errors.join('; '))
  }
  return { totalDocs: docs.length, totalChunks, embedSuccess, embedFail, errors: errors.length > 0 ? errors.slice(0, 5) : undefined }
}

/**
 * 语义搜索：vec0 KNN。
 * 失败直接报错，无降级。
 */
async function search(query, limit = 6) {
  const db = getDb()
  if (!db) throw new Error('数据库未初始化')
  if (!query || !query.trim()) return []

  const queryVec = await generateEmbedding(query.trim().slice(0, 200))
  const vecJson = vecToJson(queryVec)

  // vec0 KNN 搜索，distance 越小越相似
  const vecResults = db.prepare(`
    SELECT v.id, v.distance
    FROM kb_vectors v
    WHERE v.embedding MATCH ?
    AND v.k = ?
    ORDER BY v.distance
  `).all(vecJson, limit)

  if (vecResults.length === 0) return []

  const ids = vecResults.map(r => r.id)
  const placeholders = ids.map(() => '?').join(',')
  const chunks = db.prepare(`
    SELECT rowid, id, document_id, document_title, chunk_index, content
    FROM kb_chunks
    WHERE rowid IN (${placeholders})
  `).all(...ids)

  // 按 vec0 返回的 id 顺序排列
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
