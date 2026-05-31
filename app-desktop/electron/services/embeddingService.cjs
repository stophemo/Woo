/**
 * 本地嵌入服务。
 *
 * 使用 @xenova/transformers 在 Electron 主进程加载并运行嵌入模型，
 * 将文本转为向量。模型文件打包在项目 resources/embeddings/ 下，
 * 完全离线运行，无需外部 API。
 *
 * 模型：bge-small-zh-v1.5（中文优化，约 23MB，512 维）
 */

const path = require('path')
const fs = require('fs')

/** 本地模型目录路径（支持 dev 和生产环境） */
function getLocalModelPath() {
  // 生产环境：process.resourcesPath 指向 app.asar 同级 resources 目录
  if (process.resourcesPath) {
    const prodPath = path.join(process.resourcesPath, 'embeddings')
    if (fs.existsSync(prodPath)) return prodPath
  }
  // 开发环境：相对于本文件路径
  return path.join(__dirname, '..', '..', 'resources', 'embeddings')
}

const MODEL_PATH = getLocalModelPath()

let embedder = null
let loading = false
let loadPromise = null
let pipeline = null

/** 懒加载 @xenova/transformers（ESM 模块，必须动态 import） */
async function ensureTransformers() {
  if (pipeline) return pipeline

  // 必须在 import 之前设置 env，因为 import 时 env 就已经被初始化了
  // 但我们可以在 import 后立即修改 env 属性
  const tf = await import('@xenova/transformers')

  // 配置：仅使用本地文件，禁止远程下载
  tf.env.allowRemoteModels = false
  // localModelPath 设为空字符串，使绝对路径不会被重复拼接
  // （库默认将 localModelPath + modelName 拼接来定位文件）
  tf.env.localModelPath = ''

  pipeline = tf.pipeline
  return pipeline
}

/** 获取或初始化嵌入模型（懒加载+单例，从本地加载） */
async function getEmbedder() {
  if (embedder) return embedder
  if (loadPromise) return loadPromise

  loading = true
  console.log(`[Embed] 加载本地嵌入模型 (${MODEL_PATH})...`)
  loadPromise = (async () => {
    try {
      const fn = await ensureTransformers()
      const start = Date.now()
      // 传入本地绝对路径作为模型名，库会直接读取该路径下的文件
      embedder = await fn('feature-extraction', MODEL_PATH, {
        quantized: true,
      })
      console.log(`[Embed] 模型加载完成 (${Date.now() - start}ms)`)
      return embedder
    } catch (err) {
      console.error('[Embed] 模型加载失败:', err.message)
      throw err
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

  if (!result || !result.data) {
    throw new Error('嵌入模型返回空结果')
  }
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
