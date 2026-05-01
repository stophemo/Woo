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
  // ============ 状态 ============
  const folders = ref<FolderNode[]>([])
  const folderDocuments = ref<Document[]>([]) // 当前目录下的文稿（不含 content）
  const selectedFolderId = ref<string | null>(null)
  const selectedDocumentId = ref<string | null>(null)
  // 当前选中文稿的完整数据（切换时整体赋值；编辑时原地修改 content）
  const currentDocumentData = ref<Document | null>(null)
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
    const oldName = folder.name
    folder.name = newName // 乐观更新
    try {
      await folderApi.rename(folder.id, newName)
    } catch (e: any) {
      folder.name = oldName
      error.value = e?.message || '重命名失败'
    }
  }

  async function createRootFolder() {
    try {
      const id = await folderApi.create({ name: '新建目录', parentId: null })
      folders.value.push({
        id,
        name: '新建目录',
        children: [],
        parentId: null,
        isExpanded: false
      })
    } catch (e: any) {
      error.value = e?.message || '创建目录失败'
    }
  }

  async function createSiblingFolder(target: FolderNode) {
    try {
      const id = await folderApi.create({ name: '新建目录', parentId: target.parentId })
      const sibling: FolderNode = {
        id,
        name: '新建目录',
        children: [],
        parentId: target.parentId,
        isExpanded: false
      }
      if (target.parentId === null) {
        folders.value.push(sibling)
      } else {
        addFolderToParent(folders.value, target.parentId, sibling)
      }
    } catch (e: any) {
      error.value = e?.message || '创建目录失败'
    }
  }

  async function createChildFolder(parent: FolderNode) {
    try {
      const id = await folderApi.create({ name: '新建子目录', parentId: parent.id })
      parent.children.push({
        id,
        name: '新建子目录',
        children: [],
        parentId: parent.id,
        isExpanded: false
      })
      parent.isExpanded = true
    } catch (e: any) {
      error.value = e?.message || '创建子目录失败'
    }
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
  function updateDocumentContent(documentId: string, content: string) {
    // 原地修改当前文稿对象的 content（引用不变，不触发编辑器重新渲染）
    if (currentDocumentData.value && currentDocumentData.value.id === documentId) {
      currentDocumentData.value.content = content
      currentDocumentData.value.updatedAt = new Date().toISOString()
    }
    const meta = folderDocuments.value.find(d => d.id === documentId)
    if (meta) meta.updatedAt = new Date().toISOString()

    if (saveTimer !== null) {
      clearTimeout(saveTimer)
    }
    saveTimer = window.setTimeout(async () => {
      try {
        await documentApi.updateContent(documentId, content)
      } catch (e: any) {
        error.value = e?.message || '保存失败'
      }
    }, 800)
  }

  async function createDocument(folderId: string, title = '新建文稿') {
    try {
      const dto = await documentApi.create({ title, folderId })
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
    // 文稿操作
    selectDocument,
    updateDocumentContent,
    createDocument,
    renameDocument,
    deleteDocument,
    // 辅助
    getDocumentPreview
  }
})
