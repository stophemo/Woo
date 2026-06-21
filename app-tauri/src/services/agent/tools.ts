/**
 * Agent 内置工具定义。
 *
 * 遵循 MCP 风格的 Tool 定义（JSON Schema 描述参数），
 * 由 Agent 引擎通过 function calling 调用。
 *
 * 命名规范：短横线分隔，名字清晰体现工具用途，
 * 避免模型混淆或幻觉工具名。
 */
import type { Tool, ContextProvider } from './types'
import { requestConfirmation } from './confirmation'

/** 由外部提供的上下文句柄 */
let ctx: ContextProvider | null = null

export function setContextProvider(provider: ContextProvider) {
  ctx = provider
}

// ─── 工具：搜索知识库（语义向量） ──────────────────

const searchKbTool: Tool = {
  name: 'search_kb',
  description: '语义搜索知识库，根据含义查找相关文档片段。回答需要引用笔记内容时使用',
  parameters: {
    type: 'object',
    properties: {
      query: { type: 'string', description: '搜索关键词，从用户当前问题提取关键词，用自然语言描述想找的内容' },
      limit: { type: 'number', description: '返回结果数量，默认 5' },
    },
    required: ['query'],
  },
  handler: async (args) => {
    const query = String(args.query || '')
    const limit = typeof args.limit === 'number' ? args.limit : 5
    if (!ctx) return '知识库未就绪'
    try {
      const result = await ctx.searchKb(query, limit)
      return result || '未找到相关文档'
    } catch (e: any) {
      return `搜索失败: ${e.message}`
    }
  },
}

// ─── 工具：搜索文档标题 ─────────────────────────────

const searchDocsTool: Tool = {
  name: 'search_documents',
  description: '按标题模糊搜索笔记文档，返回匹配的文档列表（含标题和最后修改时间）。用于查找用户有哪些文档、文档是否写过',
  parameters: {
    type: 'object',
    properties: {
      query: { type: 'string', description: '文档标题关键字，从用户当前问题中提取关键词' },
    },
    required: ['query'],
  },
  handler: async (args) => {
    const query = String(args.query || '')
    if (!ctx) return '文档服务未就绪'
    try {
      const result = await ctx.searchDocuments(query)
      return result || '未找到匹配的文档'
    } catch (e: any) {
      return `搜索失败: ${e.message}`
    }
  },
}

// ─── 工具：获取文档详情 ─────────────────────────────

const getDocDetailTool: Tool = {
  name: 'get_document_detail',
  description: '根据文档标题获取完整信息（内容、创建时间、最后修改时间）。回答"某篇文档写了什么""最后一次修改是什么时候"等问题时使用。先调用 search_documents 找到标题，再用此工具获取详情',
  parameters: {
    type: 'object',
    properties: {
      title: { type: 'string', description: '文档标题（精确匹配）' },
    },
    required: ['title'],
  },
  handler: async (args) => {
    const title = String(args.title || '')
    if (!ctx) return '文档服务未就绪'
    try {
      const result = await ctx.getDocumentDetail(title)
      return result || `未找到标题为"${title}"的文档`
    } catch (e: any) {
      return `查询失败: ${e.message}`
    }
  },
}

// ─── 工具：读取当前文档 ─────────────────────────────

const readCurrentDocTool: Tool = {
  name: 'read_current_document',
  description: '读取用户当前正在编辑的文档内容。当用户要求对当前文档做修改、续写、分析、润色时使用。注意：此工具只能读取当前打开的文档',
  parameters: {
    type: 'object',
    properties: {},
    required: [],
  },
  handler: async () => {
    if (!ctx) return '文档服务未就绪'
    try {
      const doc = await ctx.getCurrentDoc()
      return doc || '当前没有打开的文档'
    } catch (e: any) {
      return `读取失败: ${e.message}`
    }
  },
}

// ─── 工具：创建笔记 ────────────────────────────────

const createNoteTool: Tool = {
  name: 'create_note',
  description: '创建一篇新的笔记。可指定目录名和初始内容。创建成功后返回笔记 ID',
  parameters: {
    type: 'object',
    properties: {
      title: { type: 'string', description: '笔记标题' },
      content: { type: 'string', description: '笔记初始内容，可选。如提供会写入笔记正文（支持 Markdown 格式）' },
      folderName: { type: 'string', description: '目标目录名称，可选。不指定则放入默认目录（首个顶级目录或根目录）' },
    },
    required: ['title'],
  },
  handler: async (args) => {
    const title = String(args.title || '').trim()
    if (!title) return '标题不能为空'
    if (!ctx) return '服务未就绪'
    try {
      let folderId: string | null = null
      const folderName = args.folderName ? String(args.folderName).trim() : undefined
      if (folderName) {
        folderId = await ctx.findFolderByName(folderName)
        if (!folderId) return `未找到名为「${folderName}」的目录，请先创建目录或省略目录参数`
      }
      const noteId = await ctx.createNote(title, folderId || undefined)
      if (!noteId) return '创建笔记失败，请重试'
      // 如有初始内容，写入
      const noteContent = args.content ? String(args.content).trim() : ''
      if (noteContent && noteId) {
        await ctx.updateNote(noteId, noteContent)
      }
      const location = folderName ? `目录「${folderName}」下` : '默认目录'
      return `笔记「${title}」创建成功（${location}），ID: ${noteId}`
    } catch (e: any) {
      return `创建失败: ${e.message}`
    }
  },
}

// ─── 工具：更新笔记（破坏性操作，需确认）────────────

const updateNoteTool: Tool = {
  name: 'update_note',
  description: '更新一篇笔记的完整内容。此操作将替换笔记的全部内容，不可撤销。用户确认后执行',
  parameters: {
    type: 'object',
    properties: {
      title: { type: 'string', description: '要更新的笔记标题' },
      newContent: { type: 'string', description: '新的笔记完整内容（纯文本或 Markdown 格式）' },
    },
    required: ['title', 'newContent'],
  },
  handler: async (args) => {
    const title = String(args.title || '').trim()
    const newContent = String(args.newContent || '')
    if (!title) return '标题不能为空'
    if (!ctx) return '服务未就绪'
    try {
      const note = await ctx.findNoteByTitle(title)
      if (!note) return `未找到标题为「${title}」的笔记`
      const confirmed = await requestConfirmation('update_note', `将修改笔记「${title}」的全部内容`)
      if (!confirmed) return '操作已取消'
      const success = await ctx.updateNote(note.id, newContent)
      return success ? `笔记「${title}」已更新` : '更新笔记失败，请重试'
    } catch (e: any) {
      return `更新失败: ${e.message}`
    }
  },
}

// ─── 工具：删除笔记（破坏性操作，需确认）────────────

const deleteNoteTool: Tool = {
  name: 'delete_note',
  description: '将笔记移入废纸篓（软删除，可从废纸篓恢复）。需要用户确认',
  parameters: {
    type: 'object',
    properties: {
      title: { type: 'string', description: '要删除的笔记标题' },
    },
    required: ['title'],
  },
  handler: async (args) => {
    const title = String(args.title || '').trim()
    if (!title) return '标题不能为空'
    if (!ctx) return '服务未就绪'
    try {
      const note = await ctx.findNoteByTitle(title)
      if (!note) return `未找到标题为「${title}」的笔记`
      const confirmed = await requestConfirmation('delete_note', `将删除笔记「${title}」（可恢复）`)
      if (!confirmed) return '操作已取消'
      const success = await ctx.deleteNote(note.id)
      return success ? `笔记「${title}」已移入废纸篓` : '删除失败，请重试'
    } catch (e: any) {
      return `删除失败: ${e.message}`
    }
  },
}

// ─── 工具：创建目录 ────────────────────────────────

const createFolderTool: Tool = {
  name: 'create_folder',
  description: '创建一个新的目录（笔记本/文件夹）。可指定父目录名创建子目录',
  parameters: {
    type: 'object',
    properties: {
      name: { type: 'string', description: '目录名称' },
      parentName: { type: 'string', description: '父目录名称，可选。不指定则创建顶级目录' },
    },
    required: ['name'],
  },
  handler: async (args) => {
    const name = String(args.name || '').trim()
    if (!name) return '目录名不能为空'
    if (!ctx) return '服务未就绪'
    try {
      let parentId: string | null = null
      const parentName = args.parentName ? String(args.parentName).trim() : undefined
      if (parentName) {
        parentId = await ctx.findFolderByName(parentName)
        if (!parentId) return `未找到名为「${parentName}」的父目录`
      }
      const folderId = await ctx.createFolder(name, parentId || undefined)
      if (!folderId) return '创建目录失败，可能已存在同名目录'
      const location = parentName ? `目录「${parentName}」下` : '顶级目录'
      return `目录「${name}」创建成功（${location}），ID: ${folderId}`
    } catch (e: any) {
      return `创建目录失败: ${e.message}`
    }
  },
}

// ─── 工具：重命名目录 ──────────────────────────────

const renameFolderTool: Tool = {
  name: 'rename_folder',
  description: '重命名一个目录',
  parameters: {
    type: 'object',
    properties: {
      name: { type: 'string', description: '当前目录名称' },
      newName: { type: 'string', description: '新的目录名称' },
    },
    required: ['name', 'newName'],
  },
  handler: async (args) => {
    const name = String(args.name || '').trim()
    const newName = String(args.newName || '').trim()
    if (!name || !newName) return '目录名不能为空'
    if (!ctx) return '服务未就绪'
    try {
      const folderId = await ctx.findFolderByName(name)
      if (!folderId) return `未找到名为「${name}」的目录`
      const success = await ctx.renameFolder(folderId, newName)
      return success ? `目录「${name}」已重命名为「${newName}」` : '重命名失败'
    } catch (e: any) {
      return `重命名失败: ${e.message}`
    }
  },
}

// ─── 工具：删除目录（破坏性操作，需确认）────────────

const deleteFolderTool: Tool = {
  name: 'delete_folder',
  description: '删除一个目录及其包含的所有笔记（移入废纸篓，可恢复）。需要用户确认',
  parameters: {
    type: 'object',
    properties: {
      name: { type: 'string', description: '要删除的目录名称' },
    },
    required: ['name'],
  },
  handler: async (args) => {
    const name = String(args.name || '').trim()
    if (!name) return '目录名不能为空'
    if (!ctx) return '服务未就绪'
    try {
      const folderId = await ctx.findFolderByName(name)
      if (!folderId) return `未找到名为「${name}」的目录`
      const confirmed = await requestConfirmation('delete_folder', `将删除目录「${name}」及其所有笔记（可恢复）`)
      if (!confirmed) return '操作已取消'
      const success = await ctx.deleteFolder(folderId)
      return success ? `目录「${name}」及其笔记已移入废纸篓` : '删除失败，请重试'
    } catch (e: any) {
      return `删除失败: ${e.message}`
    }
  },
}

// ─── 工具：列出目录 ────────────────────────────────

const listFoldersTool: Tool = {
  name: 'list_folders',
  description: '列出所有目录（笔记本/文件夹）的层级结构。在需要知道有哪些目录或要选择目录时使用',
  parameters: {
    type: 'object',
    properties: {},
    required: [],
  },
  handler: async () => {
    if (!ctx) return '服务未就绪'
    try {
      return await ctx.listFolders()
    } catch (e: any) {
      return `获取目录列表失败: ${e.message}`
    }
  },
}

// ─── 工具：列出最近笔记 ─────────────────────────────

const listRecentNotesTool: Tool = {
  name: 'list_recent_notes',
  description: '列出最近修改的笔记列表（含标题、所在目录和最后修改时间）。用于快速了解用户最近在写什么',
  parameters: {
    type: 'object',
    properties: {
      limit: { type: 'number', description: '返回笔记数量上限，默认 10' },
    },
    required: [],
  },
  handler: async (args) => {
    const limit = typeof args.limit === 'number' ? args.limit : 10
    if (!ctx) return '服务未就绪'
    try {
      return await ctx.listRecentNotes(limit)
    } catch (e: any) {
      return `获取笔记列表失败: ${e.message}`
    }
  },
}

/** 所有可用工具 */
export const BUILTIN_TOOLS: Tool[] = [
  searchKbTool,
  searchDocsTool,
  getDocDetailTool,
  readCurrentDocTool,
  createNoteTool,
  updateNoteTool,
  deleteNoteTool,
  createFolderTool,
  renameFolderTool,
  deleteFolderTool,
  listFoldersTool,
  listRecentNotesTool,
]
