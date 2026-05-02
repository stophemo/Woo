/**
 * 工作空间 Store：通过后端接口管理目录树、文稿列表与当前文稿内容。
 *
 * 关键设计：
 * - currentDocumentData 存储当前选中文稿的完整对象；切换文稿时重新赋值（引用变化）
 *   以便编辑器响应；本地编辑时仅原地修改 content 字段（引用不变），避免打断用户输入。
 * - 列表项不携带 content，保证目录切换的加载速度。
 * - 编辑后的 content 以 800ms 防抖写回后端。
 */
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { FolderNode } from '../types/folder'
import type { Document } from '../types/document'
import * as folderApi from '../services/folderApi'
import * as documentApi from '../services/documentApi'
import type { FolderTreeNodeDTO } from '../services/folderApi'
import type { DocumentDTO } from '../services/documentApi'

function mapFolder(node: FolderTreeNodeDTO): FolderNode {
  return {
    id: node.id,
    name: node.name,
    parentId: node.parentId,
    isExpanded: false,
    children: (node.children || []).map(mapFolder)
  }
}

function mapDocument(dto: DocumentDTO): Document {
  return {
    id: dto.id,
    title: dto.title,
    content: dto.content ?? '',
    folderId: dto.folderId,
    createdAt: dto.createTime,
    updatedAt: dto.updateTime
  }
}

export const useWorkspaceStore = defineStore('workspace', () => {
  // 草稿箱特殊视图标识：选中此 id 时，folderDocuments 显示本地草稿列表
  const DRAFT_FOLDER_ID = '__drafts__'
  const DRAFT_STORAGE_KEY = 'woo:drafts'

  function loadDrafts(): Document[] {
    try {
      const raw = localStorage.getItem(DRAFT_STORAGE_KEY)
      return raw ? (JSON.parse(raw) as Document[]) : []
    } catch {
      return []
    }
  }

  function persistDrafts(list: Document[]) {
    try {
      localStorage.setItem(DRAFT_STORAGE_KEY, JSON.stringify(list))
    } catch { /* ignore */ }
  }

  function isDraftId(id: string): boolean {
    return id.startsWith('draft_')
  }

  // ============ 状态 ============
  const folders = ref<FolderNode[]>([])
  const folderDocuments = ref<Document[]>([]) // 当前目录下的文稿（不含 content）
  const selectedFolderId = ref<string | null>(null)
  const selectedDocumentId = ref<string | null>(null)
  // 新建目录后需要自动进入重命名编辑状态的目录 id
  const editingFolderId = ref<string | null>(null)
  // 当前选中文稿的完整数据（切换时整体赋值；编辑时原地修改 content）
  const currentDocumentData = ref<Document | null>(null)
  // 本地草稿列表（未关联后端目录的文稿）
  const drafts = ref<Document[]>(loadDrafts())
  const loading = ref(false)
  const error = ref<string>('')

  // ============ 计算属性 ============
  const currentFolderDocuments = computed<Document[]>(() => folderDocuments.value)
  const currentDocument = computed<Document | null>(() => currentDocumentData.value)

  // ============ 启动 & 重置 ============
  async function bootstrap() {
    loading.value = true
    error.value = ''
    try {
      const tree = await folderApi.getTree()
      folders.value = tree.map(mapFolder)
      selectedFolderId.value = null
      selectedDocumentId.value = null
      folderDocuments.value = []
      currentDocumentData.value = null
    } catch (e: any) {
      error.value = e?.message || '加载目录失败'
      throw e
    } finally {
      loading.value = false
    }
  }

  function reset() {
    folders.value = []
    folderDocuments.value = []
    selectedFolderId.value = null
    selectedDocumentId.value = null
    currentDocumentData.value = null
    error.value = ''
  }

  // ============ 目录操作 ============
  function toggleFolder(folder: FolderNode) {
    folder.isExpanded = !folder.isExpanded
  }

  async function selectFolder(folderId: string) {
    selectedFolderId.value = folderId
    try {
      const list = await documentApi.listByFolder(folderId)
      folderDocuments.value = list.map(mapDocument)
      if (folderDocuments.value.length > 0) {
        await selectDocument(folderDocuments.value[0].id)
      } else {
        selectedDocumentId.value = null
        currentDocumentData.value = null
      }
    } catch (e: any) {
      error.value = e?.message || '加载文稿列表失败'
    }
  }

  async function renameFolder(folder: FolderNode, newName: string) {
    const trimmed = newName.trim()
    if (!trimmed || trimmed === folder.name) return
    // 同级目录名不可重复
    const siblings = getSiblings(folder.parentId, folder.id)
    if (siblings.some(f => f.name === trimmed)) {
      error.value = `同级已存在名为“${trimmed}”的目录`
      return
    }
    const oldName = folder.name
    folder.name = trimmed // 乐观更新
    try {
      await folderApi.rename(folder.id, trimmed)
    } catch (e: any) {
      folder.name = oldName
      error.value = e?.message || '重命名失败'
    }
  }

  async function createRootFolder() {
    try {
      const name = generateUniqueName('新建目录', folders.value)
      const id = await folderApi.create({ name, parentId: null })
      folders.value.push({
        id,
        name,
        children: [],
        parentId: null,
        isExpanded: false
      })
      editingFolderId.value = id
    } catch (e: any) {
      error.value = e?.message || '创建目录失败'
    }
  }

  async function createSiblingFolder(target: FolderNode) {
    try {
      const siblings = getSiblings(target.parentId, null)
      const name = generateUniqueName('新建目录', siblings)
      const id = await folderApi.create({ name, parentId: target.parentId })
      const sibling: FolderNode = {
        id,
        name,
        children: [],
        parentId: target.parentId,
        isExpanded: false
      }
      if (target.parentId === null) {
        folders.value.push(sibling)
      } else {
        addFolderToParent(folders.value, target.parentId, sibling)
      }
      editingFolderId.value = id
    } catch (e: any) {
      error.value = e?.message || '创建目录失败'
    }
  }

  async function createChildFolder(parent: FolderNode) {
    try {
      const name = generateUniqueName('新建子目录', parent.children)
      const id = await folderApi.create({ name, parentId: parent.id })
      parent.children.push({
        id,
        name,
        children: [],
        parentId: parent.id,
        isExpanded: false
      })
      parent.isExpanded = true
      editingFolderId.value = id
    } catch (e: any) {
      error.value = e?.message || '创建子目录失败'
    }
  }

  function clearEditingFolder() {
    editingFolderId.value = null
  }

  async function deleteFolder(folder: FolderNode) {
    try {
      await folderApi.remove(folder.id)
      if (folder.parentId === null) {
        const idx = folders.value.findIndex(f => f.id === folder.id)
        if (idx !== -1) folders.value.splice(idx, 1)
      } else {
        removeFolderFromParent(folders.value, folder.id)
      }
      if (selectedFolderId.value === folder.id) {
        selectedFolderId.value = null
        selectedDocumentId.value = null
        folderDocuments.value = []
        currentDocumentData.value = null
      }
    } catch (e: any) {
      error.value = e?.message || '删除目录失败'
    }
  }

  // ============ 文稿操作 ============
  async function selectDocument(documentId: string) {
    selectedDocumentId.value = documentId
    // 草稿：从本地获取，不调后端
    if (isDraftId(documentId)) {
      const draft = drafts.value.find(d => d.id === documentId)
      currentDocumentData.value = draft ? { ...draft } : null
      return
    }
    try {
      const dto = await documentApi.getById(documentId)
      // 整体赋值，引用变化 → 触发编辑器重载内容
      currentDocumentData.value = mapDocument(dto)
    } catch (e: any) {
      error.value = e?.message || '加载文稿内容失败'
      currentDocumentData.value = null
    }
  }

  // 防抖保存 content
  let saveTimer: number | null = null
  // 记录当前调用中的保存请求（用于 flush 等待返回）
  let savingPromise: Promise<void> | null = null
  // 当前 pending 的待保存任务（在 saveTimer 到期前保留）
  let pendingSave: { documentId: string; content: string } | null = null

  // 版本列表刷新信号：每次成功提交版本后自增，供版本面板 watch 实时重载
  const versionRefreshTick = ref(0)
  const lastVersionedDocId = ref<string | null>(null)

  function updateDocumentContent(documentId: string, content: string) {
    // 原地修改当前文稿对象的 content（引用不变，不触发编辑器重新渲染）
    if (currentDocumentData.value && currentDocumentData.value.id === documentId) {
      currentDocumentData.value.content = content
      currentDocumentData.value.updatedAt = new Date().toISOString()
    }
    const meta = folderDocuments.value.find(d => d.id === documentId)
    if (meta) {
      meta.updatedAt = new Date().toISOString()
      meta.content = content
    }

    // 草稿：只写 localStorage，不调后端
    if (isDraftId(documentId)) {
      const draft = drafts.value.find(d => d.id === documentId)
      if (draft) {
        draft.content = content
        draft.updatedAt = new Date().toISOString()
        persistDrafts(drafts.value)
      }
      return
    }

    pendingSave = { documentId, content }
    if (saveTimer !== null) {
      clearTimeout(saveTimer)
    }
    saveTimer = window.setTimeout(() => {
      saveTimer = null
      void runPendingSave()
    }, 800)
  }

  async function runPendingSave(): Promise<void> {
    if (!pendingSave) return
    const { documentId, content } = pendingSave
    pendingSave = null
    savingPromise = (async () => {
      try {
        await documentApi.updateContent(documentId, content)
      } catch (e: any) {
        error.value = e?.message || '保存失败'
      } finally {
        savingPromise = null
      }
    })()
    await savingPromise
  }

  /**
   * 立即写回当前挂起的保存任务并等待完成（用于提交版本前确保后端读到最新内容）。
   */
  async function flushPendingSave(): Promise<void> {
    if (saveTimer !== null) {
      clearTimeout(saveTimer)
      saveTimer = null
      await runPendingSave()
    } else if (savingPromise) {
      await savingPromise
    }
  }

  /**
   * 提交一个文稿版本：先 flush 未完成的正文保存，再调用后端 commit 接口。
   * 草稿（未入库的文稿）不产生版本。
   */
  async function commitDocumentVersion(documentId: string, changeType: 'auto' | 'manual' = 'auto'): Promise<void> {
    if (isDraftId(documentId)) return
    try {
      await flushPendingSave()
      const { commitVersion } = await import('../services/versionApi')
      await commitVersion(documentId, changeType)
      // 通知监听者（版本面板）：可能有新版本产生
      lastVersionedDocId.value = documentId
      versionRefreshTick.value++
    } catch (e: any) {
      error.value = e?.message || '版本保存失败'
    }
  }

  // 根据基础标题与当前列表生成不重复的默认文稿标题
  function generateUniqueDocTitle(base: string, list: Document[]): string {
    const used = new Set(list.map(d => d.title))
    if (!used.has(base)) return base
    let i = 2
    while (used.has(`${base} ${i}`)) i++
    return `${base} ${i}`
  }

  async function createDocument(folderId: string, title?: string) {
    const finalTitle = title || generateUniqueDocTitle('新建文稿', folderDocuments.value)
    try {
      const dto = await documentApi.create({ title: finalTitle, folderId })
      const doc = mapDocument(dto)
      folderDocuments.value.unshift(doc)
      selectedDocumentId.value = doc.id
      currentDocumentData.value = { ...doc } // 独立引用，避免列表项与编辑项指向同一对象
      return doc
    } catch (e: any) {
      error.value = e?.message || '创建文稿失败'
      return null
    }
  }

  // 打开草稿箱视图：folderDocuments 切为本地草稿列表
  function openDraftBox() {
    selectedFolderId.value = DRAFT_FOLDER_ID
    folderDocuments.value = drafts.value.map(d => ({ ...d }))
    if (folderDocuments.value.length > 0) {
      selectDocument(folderDocuments.value[0].id)
    } else {
      selectedDocumentId.value = null
      currentDocumentData.value = null
    }
  }

  // 本地创建一篇草稿，加入草稿列表并选中进入编辑
  function createDraft(title?: string): Document {
    const now = new Date().toISOString()
    const finalTitle = title || generateUniqueDocTitle('新建文稿', drafts.value)
    const draft: Document = {
      id: `draft_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
      title: finalTitle,
      content: '',
      folderId: DRAFT_FOLDER_ID,
      createdAt: now,
      updatedAt: now
    }
    drafts.value.unshift(draft)
    persistDrafts(drafts.value)
    // 确保切到草稿箱视图并同步列表
    selectedFolderId.value = DRAFT_FOLDER_ID
    folderDocuments.value = drafts.value.map(d => ({ ...d }))
    selectedDocumentId.value = draft.id
    currentDocumentData.value = { ...draft }
    return draft
  }

  // 统一入口：有选中真实目录 → 后端创建；否则归入草稿箱
  async function createNewDocument() {
    const fid = selectedFolderId.value
    if (fid && fid !== DRAFT_FOLDER_ID) {
      await createDocument(fid)
    } else {
      createDraft()
    }
  }

  async function renameDocument(documentId: string, title: string) {
    try {
      await documentApi.rename(documentId, title)
      const meta = folderDocuments.value.find(d => d.id === documentId)
      if (meta) meta.title = title
      if (currentDocumentData.value && currentDocumentData.value.id === documentId) {
        currentDocumentData.value.title = title
      }
    } catch (e: any) {
      error.value = e?.message || '重命名文稿失败'
    }
  }

  async function deleteDocument(documentId: string) {
    try {
      await documentApi.remove(documentId)
      const idx = folderDocuments.value.findIndex(d => d.id === documentId)
      if (idx !== -1) folderDocuments.value.splice(idx, 1)
      if (selectedDocumentId.value === documentId) {
        const next = folderDocuments.value[0]
        if (next) {
          await selectDocument(next.id)
        } else {
          selectedDocumentId.value = null
          currentDocumentData.value = null
        }
      }
    } catch (e: any) {
      error.value = e?.message || '删除文稿失败'
    }
  }

  // ============ 辅助 ============
  // 获取指定父节点下的同级目录列表，可选排除某个 id
  function getSiblings(parentId: string | null, excludeId: string | null): FolderNode[] {
    const list = parentId === null ? folders.value : findFolderById(folders.value, parentId)?.children || []
    return excludeId ? list.filter(f => f.id !== excludeId) : [...list]
  }

  function findFolderById(nodes: FolderNode[], id: string): FolderNode | null {
    for (const node of nodes) {
      if (node.id === id) return node
      if (node.children.length > 0) {
        const found = findFolderById(node.children, id)
        if (found) return found
      }
    }
    return null
  }

  // 根据基础名与同级已存在的目录，生成不重复的默认名称：“基础名”、“基础名 2”、“基础名 3”...
  function generateUniqueName(base: string, siblings: FolderNode[]): string {
    const used = new Set(siblings.map(f => f.name))
    if (!used.has(base)) return base
    let i = 2
    while (used.has(`${base} ${i}`)) i++
    return `${base} ${i}`
  }

  function addFolderToParent(nodes: FolderNode[], parentId: string, folder: FolderNode): boolean {
    for (const node of nodes) {
      if (node.id === parentId) {
        node.children.push(folder)
        return true
      }
      if (node.children.length > 0 && addFolderToParent(node.children, parentId, folder)) {
        return true
      }
    }
    return false
  }

  function removeFolderFromParent(nodes: FolderNode[], folderId: string): boolean {
    for (const node of nodes) {
      const idx = node.children.findIndex(f => f.id === folderId)
      if (idx !== -1) {
        node.children.splice(idx, 1)
        return true
      }
      if (node.children.length > 0 && removeFolderFromParent(node.children, folderId)) {
        return true
      }
    }
    return false
  }

  function getDocumentPreview(doc: Document): string {
    const content = doc.content || ''
    if (!content) return ''
    const tmp = document.createElement('div')
    tmp.innerHTML = content
    const text = tmp.textContent || tmp.innerText || ''
    const lines = text.split('\n').filter(l => l.trim())
    const body = lines.slice(1).join(' ').trim()
    return body.length > 80 ? body.substring(0, 80) + '...' : body
  }

  return {
    // 状态
    folders,
    selectedFolderId,
    selectedDocumentId,
    editingFolderId,
    loading,
    error,
    // 计算属性
    currentFolderDocuments,
    currentDocument,
    // 启动 & 重置
    bootstrap,
    reset,
    // 目录操作
    toggleFolder,
    selectFolder,
    renameFolder,
    createRootFolder,
    createSiblingFolder,
    createChildFolder,
    deleteFolder,
    clearEditingFolder,
    // 文稿操作
    selectDocument,
    updateDocumentContent,
    flushPendingSave,
    commitDocumentVersion,
    versionRefreshTick,
    lastVersionedDocId,
    createDocument,
    createNewDocument,
    openDraftBox,
    renameDocument,
    deleteDocument,
    // 辅助
    getDocumentPreview
  }
})
