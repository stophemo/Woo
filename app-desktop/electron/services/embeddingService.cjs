/**
 * 本地嵌入服务。
 *
 * 使用 @xenova/transformers 在 Electron 主进程加载并运行嵌入模型，
 * 将文本转为向量。完全离线运行，无需外部 API。
 *
 * 模型：Xenova/bge-small-zh-v1.5（中文优化，约 33MB）
 * 向量维度：512
 */

const MODEL_NAME = 'Xenova/bge-small-zh-v1.5'

let embedder = null
let loading = false
let loadPromise = null
let pipeline = null

/** 懒加载 @xenova/transformers（ESM 模块，必须动态 import） */
async function ensureTransformers() {
  if (pipeline) return pipeline
  const tf = await import('@xenova/transformers')
  pipeline = tf.pipeline
  return pipeline
}

/** 获取或初始化嵌入模型（懒加载+单例） */
async function getEmbedder() {
  if (embedder) return embedder
  if (loadPromise) return loadPromise

  loading = true
  loadPromise = (async () => {
    try {
      const fn = await ensureTransformers()
      // pipeline('feature-extraction', modelName) 返回一个函数，输入文本输出向量
      embedder = await fn('feature-extraction', MODEL_NAME, {
        quantized: true, // 量化版本，缩小体积加快速度
      })
      return embedder
    } finally {
      loading = false
      loadPromise = null
    }
  })()

  return loadPromise
}

/**
 * 将文本转为 float32 向量。
 * @param {string} text - 输入文本
 * @returns {Promise<Float32Array>} 512 维向量
 */
async function generateEmbedding(text) {
  const fn = await getEmbedder()
  if (!fn) throw new Error('嵌入模型加载失败')

  // bge 系列模型需要添加 prompt 前缀以获得更好的嵌入质量
  const input = `为这个句子生成表示以用于检索相关文章：${text}`

  const result = await fn(input, {
    pooling: 'mean',
    normalize: true,
  })

  // result.data 是 Float32Array
  return result.data
}

/**
 * 计算两个向量的余弦相似度。
 * @param {Float32Array} a
 * @param {Float32Array} b
 * @returns {number} 0~1 之间的相似度
 */
function cosineSimilarity(a, b) {
  if (a.length !== b.length) return 0
  let dot = 0
  let normA = 0
  let normB = 0
  for (let i = 0; i < a.length; i++) {
    dot += a[i] * b[i]
    normA += a[i] * a[i]
    normB += b[i] * b[i]
  }
  if (normA === 0 || normB === 0) return 0
  return dot / (Math.sqrt(normA) * Math.sqrt(normB))
}

/**
 * 将 Float32Array 转为 Buffer 以便存入 SQLite BLOB。
 */
function vectorToBuffer(vec) {
  return Buffer.from(vec.buffer)
}

/**
 * 从 SQLite BLOB 读出 Float32Array。
 */
function vectorFromBuffer(buffer) {
  return new Float32Array(buffer.buffer, buffer.byteOffset, buffer.byteLength / 4)
}

/**
 * 加载状态。
 */
function getStatus() {
  if (embedder) return 'ready'
  if (loading) return 'loading'
  return 'idle'
}

module.exports = {
  generateEmbedding,
  cosineSimilarity,
  vectorToBuffer,
  vectorFromBuffer,
  getStatus,
}
