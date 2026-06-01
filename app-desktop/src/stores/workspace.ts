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
import * as lockApi from '../services/lockApi'
import type { FolderTreeNodeDTO } from '../services/folderApi'
import type { DocumentDTO } from '../services/documentApi'

function mapFolder(node: FolderTreeNodeDTO): FolderNode {
  return {
    id: node.id,
    name: node.name,
    parentId: node.parentId,
    isExpanded: false,
    children: (node.children || []).map(mapFolder),
    isLocked: node.isLocked
  }
}

function mapDocument(dto: DocumentDTO): Document {
  return {
    id: dto.id,
    title: dto.title,
    content: dto.content ?? '',
    folderId: dto.folderId,
    folderName: dto.folderName,
    createdAt: dto.createTime,
    updatedAt: dto.updateTime,
    isLocked: dto.isLocked
  }
}

export const useWorkspaceStore = defineStore('workspace', () => {
  // 草稿箱特殊视图标识：选中此 id 时，folderDocuments 显示本地草稿列表
  const DRAFT_FOLDER_ID = '__drafts__'
  const TRASH_FOLDER_ID = '__trash__'
  const SEARCH_FOLDER_ID = '__search__'
  const ALL_FOLDER_ID = '__all__'
  const DRAFT_STORAGE_KEY = 'woo:drafts'
  const TRASH_DRAFT_STORAGE_KEY = 'woo:trash_drafts'

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

  function loadTrashDrafts(): Document[] {
    try {
      const raw = localStorage.getItem(TRASH_DRAFT_STORAGE_KEY)
      return raw ? (JSON.parse(raw) as Document[]) : []
    } catch {
      return []
    }
  }

  function persistTrashDrafts(list: Document[]) {
    try {
      localStorage.setItem(TRASH_DRAFT_STORAGE_KEY, JSON.stringify(list))
    } catch { /* ignore */ }
  }

  function isDraftId(id: string): boolean {
    return id.startsWith('draft_')
  }

  // ============ 状态 ============
  const folders = ref<FolderNode[]>([])
  const folderDocuments = ref<Document[]>([]) // 当前目录下的文稿（不含 content）
  const selectedFolderId = ref<string | null>(null)
  const selectedFolderLocked = ref(false)
  const selectedDocumentId = ref<string | null>(null)
  // 新建目录后需要自动进入重命名编辑状态的目录 id
  const editingFolderId = ref<string | null>(null)
  // 当前选中文稿的完整数据（切换时整体赋值；编辑时原地修改 content）
  const currentDocumentData = ref<Document | null>(null)
  // 本地草稿列表（未关联后端目录的文稿）
  const drafts = ref<Document[]>(loadDrafts())
  // 废纸篓中的草稿（已从草稿箱删除但未彻底删除）
  const trashDrafts = ref<Document[]>(loadTrashDrafts())
  const loading = ref(false)
  const error = ref<string>('')

  // ============ 计算属性 ============
  const currentFolderDocuments = computed<Document[]>(() => folderDocuments.value)
  const currentDocument = computed<Document | null>(() => currentDocumentData.value)

  // ============ 上次打开的视图缓存 ============
  const LAST_VIEW_KEY = 'woo:lastView'
  const LAST_DOC_KEY = 'woo:lastDoc'

  function saveLastView(folderId: string | null) {
    if (folderId) {
      localStorage.setItem(LAST_VIEW_KEY, folderId)
    } else {
      localStorage.removeItem(LAST_VIEW_KEY)
    }
  }

  function loadLastView(): string | null {
    try {
      return localStorage.getItem(LAST_VIEW_KEY)
    } catch {
      return null
    }
  }

  function saveLastDoc(docId: string | null) {
    if (docId) {
      localStorage.setItem(LAST_DOC_KEY, docId)
    } else {
      localStorage.removeItem(LAST_DOC_KEY)
    }
  }

  /**
   * 启动后恢复上次打开的视图。
   * 优先恢复上次的目录 + 文档，若无缓存则默认打开"全部"视图。
   */
  async function restoreLastView() {
    const lastFolderId = loadLastView()
    if (lastFolderId) {
      await selectFolder(lastFolderId)
      // 恢复目录后，再尝试恢复上次打开的文档
      const lastDocId = loadLastDoc()
      if (lastDocId && folderDocuments.value.some(d => d.id === lastDocId)) {
        await selectDocument(lastDocId)
      }
    } else {
      await openAllDocuments()
    }
  }

  function loadLastDoc(): string | null {
    try {
      return localStorage.getItem(LAST_DOC_KEY)
    } catch {
      return null
    }
  }

  // ============ 启动 & 重置 ============
  async function bootstrap() {
    loading.value = true
    error.value = ''
    // 同步重置，确保在用户交互之前初始状态已清空
    selectedFolderId.value = null
    selectedDocumentId.value = null
    folderDocuments.value = []
    currentDocumentData.value = null
    try {
      const tree = await folderApi.getTree()
      folders.value = tree.map(mapFolder)
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
    selectedFolderLocked.value = false
    error.value = ''
  }

  // ============ 目录操作 ============
  function toggleFolder(folder: FolderNode) {
    folder.isExpanded = !folder.isExpanded
  }

  async function selectFolder(folderId: string) {
    if (folderId === DRAFT_FOLDER_ID) {
      await openDraftBox()
      return
    }
    if (folderId === TRASH_FOLDER_ID) {
      await openTrashBox()
      return
    }
    if (folderId === ALL_FOLDER_ID) {
      await openAllDocuments()
      return
    }
    selectedFolderId.value = folderId
    saveLastView(folderId) // 缓存最近打开的视图
    try {
      // 隐私保护：检查目录是否被锁定（自身或祖先目录）
      const effectivelyLocked = await lockApi.isFolderEffectivelyLocked(folderId)
      if (selectedFolderId.value !== folderId) return
      if (effectivelyLocked) {
        selectedFolderLocked.value = true
        folderDocuments.value = []
        selectedDocumentId.value = null
        currentDocumentData.value = null
        return
      }
      selectedFolderLocked.value = false
      const list = await documentApi.listByFolder(folderId)
      // 防竞态：如果用户在此期间切换了目录，放弃本次结果
      if (selectedFolderId.value !== folderId) return
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
    saveLastDoc(documentId) // 缓存最近打开的文档
    // 废纸篓视图优先：从当前列表获取（含后端已删文档 + 本地已删草稿）
    if (selectedFolderId.value === TRASH_FOLDER_ID) {
      const trashDoc = folderDocuments.value.find(d => d.id === documentId)
      currentDocumentData.value = trashDoc ? { ...trashDoc } : null
      return
    }
    // 草稿：从本地获取，不调后端
    if (isDraftId(documentId)) {
      const draft = drafts.value.find(d => d.id === documentId)
      currentDocumentData.value = draft ? { ...draft } : null
      return
    }
    try {
      const dto = await documentApi.getById(documentId)
      // 防竞态：如果用户在此期间切换了文稿，放弃本次结果
      if (selectedDocumentId.value !== documentId) return
      const doc = mapDocument(dto)
      // 隐私保护：加锁文稿不保留内容在内存中
      if (doc.isLocked) doc.content = ''
      // 整体赋值，引用变化 → 触发编辑器重载内容
      currentDocumentData.value = doc
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

  /** AI 流式编辑时设置为 true，告知编辑器跳过 onUpdate 防抖 */
  const isExternalStreaming = ref(false)

  /**
   * AI 流式写入内容：每次调用都创建新引用，触发编辑器 watcher 重渲染。
   * 用于模拟打字效果逐字更新编辑器内容。
   */
  function updateContentStream(documentId: string, content: string) {
    if (currentDocumentData.value && currentDocumentData.value.id === documentId) {
      const now = new Date().toISOString()
      currentDocumentData.value = {
        ...currentDocumentData.value,
        content,
        updatedAt: now,
      }
    }
    // 同步更新文档列表中的 meta 数据
    const meta = folderDocuments.value.find(d => d.id === documentId)
    if (meta) {
      meta.updatedAt = new Date().toISOString()
      meta.content = content
    }
  }

  function updateDocumentContent(documentId: string, content: string) {
    if (selectedFolderId.value === TRASH_FOLDER_ID) {
      return
    }
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

  // 打开草稿箱视图：本地草稿 + 后端遗留文档（无目录/目录不存在）
  async function openDraftBox() {
    selectedFolderId.value = DRAFT_FOLDER_ID
    saveLastView(DRAFT_FOLDER_ID)
    selectedFolderLocked.value = false
    const localDrafts = drafts.value.map(d => ({
      ...d,
      folderName: '草稿箱'
    }))
    try {
      const orphans = (await documentApi.listOrphans()).map(d => ({
        ...mapDocument(d),
        folderName: '未分类'
      }))
      folderDocuments.value = [...localDrafts, ...orphans].sort(
        (a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime()
      )
    } catch {
      folderDocuments.value = localDrafts
    }
    await selectFirstDocOrClear()
  }

  async function openTrashBox() {
    selectedFolderId.value = TRASH_FOLDER_ID
    saveLastView(TRASH_FOLDER_ID)
    selectedFolderLocked.value = false
    try {
      const list = await documentApi.listTrash()
      const backendDocs = list.map(d => ({
        ...mapDocument(d),
        folderName: ''
      }))
      const localTrashDrafts = trashDrafts.value.map(d => ({
        ...d,
        folderName: '草稿箱'
      }))
      // 合并后端废纸篓与本地已删草稿，按 update_time 降序
      folderDocuments.value = [...backendDocs, ...localTrashDrafts].sort(
        (a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime()
      )
      await selectFirstDocOrClear()
    } catch (e: any) {
      error.value = e?.message || '加载废纸篓失败'
    }
  }

  async function openSearch(keyword: string) {
    selectedFolderId.value = SEARCH_FOLDER_ID
    selectedFolderLocked.value = false
    const q = keyword.trim()
    if (!q) {
      folderDocuments.value = []
      selectedDocumentId.value = null
      currentDocumentData.value = null
      return
    }
    try {
      const list = await documentApi.search(q)
      folderDocuments.value = list.map(mapDocument)
      await selectFirstDocOrClear()
    } catch (e: any) {
      error.value = e?.message || '搜索文稿失败'
    }
  }

  // 打开全部文稿视图：后端所有文档 + 本地草稿，带来源标记
  async function openAllDocuments() {
    selectedFolderId.value = ALL_FOLDER_ID
    saveLastView(ALL_FOLDER_ID) // 缓存"全部"视图
    selectedFolderLocked.value = false
    try {
      const backendDocs = (await documentApi.listAll()).map(d => ({
        ...mapDocument(d),
        folderName: d.folderName || '未分类'
      }))
      const draftDocs = drafts.value.map(d => ({
        ...d,
        folderName: '草稿箱'
      }))
      // 合并，update_time 降序
      const merged = [...backendDocs, ...draftDocs].sort(
        (a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime()
      )
      folderDocuments.value = merged
      await selectFirstDocOrClear()
    } catch (e: any) {
      error.value = e?.message || '加载文稿失败'
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
    selectedFolderLocked.value = false
    folderDocuments.value = drafts.value.map(d => ({ ...d }))
    selectedDocumentId.value = draft.id
    currentDocumentData.value = { ...draft }
    return draft
  }

  // 统一入口：有选中真实目录 → 后端创建；否则归入草稿箱
  // 后端创建失败（目录不存在等）自动回退为草稿
  async function createNewDocument() {
    const fid = selectedFolderId.value
    if (fid && fid !== DRAFT_FOLDER_ID) {
      const doc = await createDocument(fid)
      if (!doc) {
        createDraft()
      }
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
    if (isDraftId(documentId)) {
      const idx = drafts.value.findIndex(d => d.id === documentId)
      if (idx !== -1) {
        const [deleted] = drafts.value.splice(idx, 1)
        // 已删除草稿移入废纸篓（标记来源以便显示）
        trashDrafts.value.unshift({ ...deleted, folderName: '草稿箱' })
        persistTrashDrafts(trashDrafts.value)
        persistDrafts(drafts.value)
      }
      if (selectedFolderId.value === DRAFT_FOLDER_ID) {
        folderDocuments.value = drafts.value.map(d => ({ ...d }))
      } else {
        const listIdx = folderDocuments.value.findIndex(d => d.id === documentId)
        if (listIdx !== -1) folderDocuments.value.splice(listIdx, 1)
      }
      await removeDocFromListAndAdvance(documentId)
      return
    }
    if (selectedFolderId.value === TRASH_FOLDER_ID) {
      error.value = '请在废纸篓中使用“彻底删除”'
      return
    }
    try {
      await documentApi.remove(documentId)
      const idx = folderDocuments.value.findIndex(d => d.id === documentId)
      if (idx !== -1) folderDocuments.value.splice(idx, 1)
      await removeDocFromListAndAdvance(documentId)
    } catch (e: any) {
      error.value = e?.message || '删除文稿失败'
    }
  }

  async function restoreDocument(documentId: string) {
    // 废纸篓中的草稿恢复到草稿箱
    if (isDraftId(documentId)) {
      const idx = trashDrafts.value.findIndex(d => d.id === documentId)
      if (idx !== -1) {
        const [restored] = trashDrafts.value.splice(idx, 1)
        delete restored.folderName
        drafts.value.unshift(restored)
        persistDrafts(drafts.value)
        persistTrashDrafts(trashDrafts.value)
      }
      const listIdx = folderDocuments.value.findIndex(d => d.id === documentId)
      if (listIdx !== -1) folderDocuments.value.splice(listIdx, 1)
      await removeDocFromListAndAdvance(documentId)
      return
    }
    try {
      await documentApi.restore(documentId)
      const idx = folderDocuments.value.findIndex(d => d.id === documentId)
      if (idx !== -1) folderDocuments.value.splice(idx, 1)
      await removeDocFromListAndAdvance(documentId)
    } catch (e: any) {
      error.value = e?.message || '恢复文稿失败'
    }
  }

  async function hardDeleteDocument(documentId: string) {
    // 废纸篓中的草稿：彻底删除（从 localStorage 移除）
    if (isDraftId(documentId)) {
      const idx = trashDrafts.value.findIndex(d => d.id === documentId)
      if (idx !== -1) {
        trashDrafts.value.splice(idx, 1)
        persistTrashDrafts(trashDrafts.value)
      }
      const listIdx = folderDocuments.value.findIndex(d => d.id === documentId)
      if (listIdx !== -1) folderDocuments.value.splice(listIdx, 1)
      await removeDocFromListAndAdvance(documentId)
      return
    }
    try {
      await documentApi.hardDelete(documentId)
      const idx = folderDocuments.value.findIndex(d => d.id === documentId)
      if (idx !== -1) folderDocuments.value.splice(idx, 1)
      await removeDocFromListAndAdvance(documentId)
    } catch (e: any) {
      error.value = e?.message || '彻底删除失败'
    }
  }

  async function emptyTrash() {
    try {
      await documentApi.emptyTrash()
      // 同时清空废纸篓中的本地草稿
      trashDrafts.value = []
      persistTrashDrafts(trashDrafts.value)
      folderDocuments.value = []
      selectedDocumentId.value = null
      currentDocumentData.value = null
    } catch (e: any) {
      error.value = e?.message || '清空废纸篓失败'
    }
  }

  // ============ 同步刷新（diff 增量） ============

  /**
   * 将新树合并到旧树上，尽量保留对象引用不变（避免 Vue 全量重渲染），
   * 同时保留 isExpanded 状态。返回 { added, removed } 的 id 集合。
   */
  function mergeFolderTree(oldNodes: FolderNode[], newDTOs: FolderTreeNodeDTO[]): { addedIds: Set<string>; removedIds: Set<string> } {
    const oldMap = new Map<string, FolderNode>()
    const walkOld = (list: FolderNode[]) => { for (const n of list) { oldMap.set(n.id, n); walkOld(n.children) } }
    walkOld(oldNodes)

    const newIds = new Set<string>()
    const addedIds = new Set<string>()
    const removedIds = new Set<string>()

    function build(nodes: FolderNode[], dtos: FolderTreeNodeDTO[]) {
      const dtoMap = new Map(dtos.map(d => [d.id, d]))
      // 移除不存在的
      for (let i = nodes.length - 1; i >= 0; i--) {
        if (!dtoMap.has(nodes[i].id)) {
          removedIds.add(nodes[i].id)
          nodes.splice(i, 1)
        }
      }
      // 更新/新增
      let insertIdx = 0
      for (const dto of dtos) {
        newIds.add(dto.id)
        const existing = oldMap.get(dto.id)
        if (existing && nodes.includes(existing)) {
          // 已存在 → 更新 name/isLocked（如有变化），保留 isExpanded，递归 children
          if (existing.name !== dto.name) existing.name = dto.name
          if (existing.isLocked !== dto.isLocked) existing.isLocked = dto.isLocked
          build(existing.children, dto.children || [])
          // 移到正确位置
          const curIdx = nodes.indexOf(existing)
          if (curIdx !== insertIdx) {
            nodes.splice(curIdx, 1)
            nodes.splice(insertIdx, 0, existing)
          }
          insertIdx++
        } else if (existing && !nodes.includes(existing)) {
          // 节点存在于旧树但不在当前层级（可能被移动了）→ 当作新增
          const newNode: FolderNode = {
            id: dto.id,
            name: dto.name,
            parentId: dto.parentId,
            isExpanded: existing.isExpanded,
            children: [],
            isLocked: dto.isLocked
          }
          build(newNode.children, dto.children || [])
          nodes.splice(insertIdx, 0, newNode)
          addedIds.add(dto.id)
          insertIdx++
        } else {
          // 全新节点
          const newNode: FolderNode = {
            id: dto.id,
            name: dto.name,
            parentId: dto.parentId,
            isExpanded: false,
            children: [],
            isLocked: dto.isLocked
          }
          build(newNode.children, dto.children || [])
          nodes.splice(insertIdx, 0, newNode)
          addedIds.add(dto.id)
          insertIdx++
        }
      }
    }
    build(oldNodes, newDTOs)
    return { addedIds, removedIds }
  }

  /**
   * diff 增量同步：对比当前云端数据与本地数据，仅对有差异的部分做更新。
   * 不会清空/重置任何状态，因此不会触发 watcher 误判。
   */
  async function syncRefresh() {
    error.value = ''

    try {
      // 1. 同步目录树（保留展开状态）
      const tree = await folderApi.getTree()
      mergeFolderTree(folders.value, tree)

      // 2. 如果当前在常规目录视图，同步文稿列表
      const fid = selectedFolderId.value
      if (fid && fid !== DRAFT_FOLDER_ID && fid !== TRASH_FOLDER_ID
          && fid !== ALL_FOLDER_ID && fid !== SEARCH_FOLDER_ID) {
        const list = await documentApi.listByFolder(fid)
        const newDocs = list.map(mapDocument)

        // 构建旧 id 集合
        const oldMap = new Map<string, Document>()
        for (const d of folderDocuments.value) oldMap.set(d.id, d)

        // --- 找出删除的 ---
        const newIds = new Set(newDocs.map(d => d.id))
        for (let i = folderDocuments.value.length - 1; i >= 0; i--) {
          if (!newIds.has(folderDocuments.value[i].id)) {
            folderDocuments.value.splice(i, 1)
          }
        }

        // --- 按新顺序重新构建列表，复用旧引用 ---
        const merged: Document[] = []
        for (const nd of newDocs) {
          const old = oldMap.get(nd.id)
          if (old) {
            // 更新可能变化的字段（title、updatedAt），保留对象引用
            if (old.title !== nd.title) old.title = nd.title
            if (old.folderId !== nd.folderId) old.folderId = nd.folderId
            if (old.folderName !== nd.folderName) old.folderName = nd.folderName
            if (old.updatedAt !== nd.updatedAt) old.updatedAt = nd.updatedAt
            if (old.isLocked !== nd.isLocked) old.isLocked = nd.isLocked
            merged.push(old)
          } else {
            merged.push(nd)
          }
        }

        // 用 splice 整体替换，Vue 能识别最小变动
        folderDocuments.value.splice(0, folderDocuments.value.length, ...merged)

        // 3. 冲突检测：当前正在编辑的文稿
        if (selectedDocumentId.value && !isDraftId(selectedDocumentId.value)) {
          const cloudDoc = newDocs.find(d => d.id === selectedDocumentId.value)
          const localDoc = currentDocumentData.value
          if (cloudDoc && localDoc && cloudDoc.updatedAt !== localDoc.updatedAt) {
            // 云端有更新 → 检查本地是否有未保存的修改
            const hasUnsaved = saveTimer !== null || pendingSave !== null || savingPromise !== null
            if (hasUnsaved) {
              // 冲突：将本地编辑版另存为同目录下的一份新文稿
              try {
                // 先把本地未保存内容写回，确保 conflict 副本内容完整
                await flushPendingSave()
                const conflictDTO = await documentApi.create({
                  title: `${localDoc.title}_conflict`,
                  folderId: localDoc.folderId || fid
                })
                // 用本地编辑的内容覆盖新建文稿的内容
                await documentApi.updateContent(conflictDTO.id, localDoc.content)
                const conflictDoc = mapDocument(conflictDTO)
                conflictDoc.content = localDoc.content
                // 插入到当前文稿列表
                folderDocuments.value.unshift(conflictDoc)
              } catch { /* 冲突备份失败时静默，继续加载云端 */ }
              // 加载云端数据到编辑器
              try {
                const dto = await documentApi.getById(cloudDoc.id)
                const doc = mapDocument(dto)
                if (doc.isLocked) doc.content = ''
                currentDocumentData.value = doc
              } catch {
                // 加载失败时保留本地数据不做变更
              }
            } else {
              // 无未保存修改 → 直接加载云端最新内容
              try {
                const dto = await documentApi.getById(cloudDoc.id)
                const doc = mapDocument(dto)
                if (doc.isLocked) doc.content = ''
                currentDocumentData.value = doc
              } catch { /* ignore */ }
            }
          }
        }
      }
    } catch (e: any) {
      error.value = e?.message || '同步刷新失败'
    }
  }

  // ============ 辅助 ============
  /** 从列表中移除指定文档，若正是当前选中则跳到下一篇 */
  async function removeDocFromListAndAdvance(documentId: string) {
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
  }

  /** 列表非空时选中第一篇，否则清空编辑区 */
  async function selectFirstDocOrClear() {
    if (folderDocuments.value.length > 0) {
      await selectDocument(folderDocuments.value[0].id)
    } else {
      selectedDocumentId.value = null
      currentDocumentData.value = null
    }
  }

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

  // ============ 拖拽排序 ============

  /** 持久化同级目录排序到后端 */
  async function reorderFolderSiblings(parentId: string | null, orderedIds: string[]) {
    const items = orderedIds.map((id, idx) => ({ id, sortOrder: idx }))
    try {
      await folderApi.reorderFolders(parentId, items)
    } catch (e: any) {
      error.value = e?.message || '保存排序失败'
    }
  }

  /** 持久化当前目录下文稿排序到后端 */
  async function reorderDocumentItems(orderedIds: string[]) {
    const fid = selectedFolderId.value
    if (!fid || fid.startsWith('__')) return
    const items = orderedIds.map((id, idx) => ({ id, sortOrder: idx }))
    try {
      await documentApi.reorderDocuments(fid, items)
    } catch (e: any) {
      error.value = e?.message || '保存排序失败'
    }
  }

  return {
    // 状态
    folders,
    folderDocuments,
    currentDocumentData,
    selectedFolderId,
    selectedFolderLocked,
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
    restoreLastView,
    // 同步
    syncRefresh,
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
    updateContentStream,
    isExternalStreaming,
    flushPendingSave,
    commitDocumentVersion,
    versionRefreshTick,
    lastVersionedDocId,
    createDocument,
    createNewDocument,
    openDraftBox,
    openAllDocuments,
    openTrashBox,
    openSearch,
    renameDocument,
    deleteDocument,
    restoreDocument,
    hardDeleteDocument,
    emptyTrash,
    // 排序
    reorderFolderSiblings,
    reorderDocumentItems,
    // 辅助
    findFolderById,
    getDocumentPreview
  }
})
