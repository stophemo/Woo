/**
 * 版本服务（本地版）。无用户隔离。
 * - 写入 + 历史合并（24h 内保留；24h~7d 相邻 ≤100 字合并；>7d 相邻 ≤1000 字合并）
 * - 去重（SHA-256）
 * - 回滚（写回正文并追加 restore 版本）
 */
const { getDb } = require('../db/index.cjs')
const documentService = require('./documentService.cjs')
const { newId, sha256, stripHtml, nowStr } = require('./utils.cjs')

const PREVIEW_LIMIT = 80
const RECENT_HOURS = 24
const MID_DAYS = 7
const MID_DIFF = 100
const OLD_DIFF = 1000
const EDIT_DISTANCE_MAX_LEN = 10000

function findLatest(documentId) {
  const db = getDb()
  return db.prepare(`SELECT * FROM note_document_version WHERE document_id = ?
                     ORDER BY version_no DESC LIMIT 1`).get(documentId)
}

function saveSnapshot(doc, changeType, operatorId) {
  if (!doc) return null
  const content = doc.content == null ? '' : doc.content
  const hash = sha256(content)
  const latest = findLatest(doc.id)
  if (latest && latest.content_hash === hash) return null
  const nextNo = latest ? latest.version_no + 1 : 1
  const id = newId()
  const db = getDb()
  db.prepare(`INSERT INTO note_document_version
              (id, document_id, version_no, title, content, content_hash, change_type, operator_id)
              VALUES (?, ?, ?, ?, ?, ?, ?, ?)`)
    .run(id, doc.id, nextNo, doc.title, content, hash, changeType || 'auto', operatorId || null)
  return db.prepare('SELECT * FROM note_document_version WHERE id = ?').get(id)
}

function saveAndCompact(doc, changeType, operatorId) {
  const saved = saveSnapshot(doc, changeType, operatorId)
  if (saved) compactHistory(doc.id)
  return saved
}

function compactHistory(documentId) {
  const db = getDb()
  const rows = db.prepare(`SELECT * FROM note_document_version WHERE document_id = ?
                           ORDER BY version_no ASC`).all(documentId)
  if (rows.length < 2) return
  const now = Date.now()
  let prev = null
  const del = db.prepare('DELETE FROM note_document_version WHERE id = ?')
  const trx = db.transaction(() => {
    for (const curr of rows) {
      if (!prev) { prev = curr; continue }
      const threshold = thresholdFor(prev.create_time, now)
      if (threshold > 0) {
        const diff = textDiff(prev.content, curr.content)
        if (diff <= threshold) {
          del.run(prev.id)
          prev = curr
          continue
        }
      }
      prev = curr
    }
  })
  trx()
}

function thresholdFor(createTimeStr, nowMs) {
  if (!createTimeStr) return 0
  const t = Date.parse(createTimeStr.replace(' ', 'T'))
  if (Number.isNaN(t)) return 0
  if (t > nowMs - RECENT_HOURS * 3600_000) return 0
  if (t > nowMs - MID_DAYS * 24 * 3600_000) return MID_DIFF
  return OLD_DIFF
}

function listVersions(documentId) {
  documentService.verifyExists(documentId)
  const db = getDb()
  const rows = db.prepare(`SELECT * FROM note_document_version WHERE document_id = ?
                           ORDER BY version_no DESC`).all(documentId)
  return rows.map(r => ({
    id: r.id,
    documentId: r.document_id,
    versionNo: r.version_no,
    title: r.title,
    changeType: r.change_type,
    operatorId: r.operator_id,
    createTime: r.create_time,
    preview: buildPreview(r.content)
  }))
}

function getVersion(documentId, versionNo) {
  documentService.verifyExists(documentId)
  const db = getDb()
  const r = db.prepare(`SELECT * FROM note_document_version WHERE document_id = ? AND version_no = ?`)
    .get(documentId, Number(versionNo))
  if (!r) throw new Error('版本不存在')
  return {
    id: r.id,
    documentId: r.document_id,
    versionNo: r.version_no,
    title: r.title,
    content: r.content || '',
    contentHash: r.content_hash,
    changeType: r.change_type,
    operatorId: r.operator_id,
    createTime: r.create_time,
    preview: buildPreview(r.content)
  }
}

function saveManual(documentId) {
  const doc = documentService.verifyExists(documentId)
  saveAndCompact(doc, 'manual', 'local')
}

function commit(documentId, changeType) {
  const normalized = String(changeType || 'auto').toLowerCase() === 'manual' ? 'manual' : 'auto'
  const doc = documentService.verifyExists(documentId)
  saveAndCompact(doc, normalized, 'local')
}

function restore(documentId, versionNo) {
  const doc = documentService.verifyExists(documentId)
  const target = getVersion(documentId, versionNo)
  const db = getDb()
  db.prepare('UPDATE note_document SET content = ?, title = ?, update_time = ? WHERE id = ?')
    .run(target.content, target.title, nowStr(), doc.id)
  // 写回后重新取最新 doc 再存快照
  const fresh = db.prepare('SELECT * FROM note_document WHERE id = ?').get(doc.id)
  const saved = saveAndCompact(fresh, 'restore', 'local')
  if (!saved) return target
  return {
    id: saved.id,
    documentId: saved.document_id,
    versionNo: saved.version_no,
    title: saved.title,
    content: saved.content || '',
    contentHash: saved.content_hash,
    changeType: saved.change_type,
    operatorId: saved.operator_id,
    createTime: saved.create_time,
    preview: buildPreview(saved.content)
  }
}

function buildPreview(content) {
  const plain = stripHtml(content)
  return plain.length > PREVIEW_LIMIT ? plain.substring(0, PREVIEW_LIMIT) + '…' : plain
}

// —— 文本差异：先剥离 HTML，然后做编辑距离；超长退化为长度差 ——
function textDiff(a, b) {
  const ta = stripHtml(a)
  const tb = stripHtml(b)
  if (ta === tb) return 0
  if (ta.length > EDIT_DISTANCE_MAX_LEN || tb.length > EDIT_DISTANCE_MAX_LEN) {
    return Math.abs(ta.length - tb.length)
  }
  return levenshtein(ta, tb)
}

function levenshtein(a, b) {
  if (a.length < b.length) { const t = a; a = b; b = t }
  const m = a.length, n = b.length
  const dp = new Array(n + 1)
  for (let j = 0; j <= n; j++) dp[j] = j
  for (let i = 1; i <= m; i++) {
    let prev = dp[0]
    dp[0] = i
    for (let j = 1; j <= n; j++) {
      const temp = dp[j]
      const cost = a.charCodeAt(i - 1) === b.charCodeAt(j - 1) ? 0 : 1
      dp[j] = Math.min(dp[j] + 1, dp[j - 1] + 1, prev + cost)
      prev = temp
    }
  }
  return dp[n]
}

module.exports = { listVersions, getVersion, saveManual, commit, restore }
