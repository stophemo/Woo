import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { FolderNode } from '../types/folder'
import type { Document } from '../types/document'

export const useWorkspaceStore = defineStore('workspace', () => {
  // ============ 状态 ============

  // 目录树数据
  const folders = ref<FolderNode[]>([
    {
      id: '1',
      name: '随想',
      children: [],
      parentId: null,
      isExpanded: false
    },
    {
      id: '2',
      name: '随记',
      children: [
        {
          id: '2-1',
          name: '日记',
          children: [],
          parentId: '2',
          isExpanded: false
        },
        {
          id: '2-2',
          name: '周记',
          children: [],
          parentId: '2',
          isExpanded: false
        }
      ],
      parentId: null,
      isExpanded: false
    },
    {
      id: '3',
      name: '随摘',
      children: [],
      parentId: null,
      isExpanded: false
    },
    {
      id: '4',
      name: '备忘',
      children: [],
      parentId: null,
      isExpanded: false
    },
    {
      id: '5',
      name: '工作',
      children: [],
      parentId: null,
      isExpanded: false
    }
  ])

  // 文稿数据（模拟数据）
  const documents = ref<Document[]>([
    {
      id: 'doc-1',
      title: '关于自由的思考',
      content: '<h1>关于自由的思考</h1><p>自由不是随心所欲，而是自我主宰。自由意味着责任，这就是为什么大多数人惧怕自由。</p><p>真正的自由，是在认清生活真相之后，依然热爱生活。</p>',
      folderId: '1',
      createdAt: Date.now() - 86400000 * 3,
      updatedAt: Date.now() - 86400000 * 1
    },
    {
      id: 'doc-2',
      title: '读书笔记：瓦尔登湖',
      content: '<h1>读书笔记：瓦尔登湖</h1><p>梭罗说："我步入丛林，因为我希望生活得有意义。我希望活得深刻，汲取生命中所有的精华。"</p><p>这本书让我重新思考了简朴生活的意义。</p>',
      folderId: '1',
      createdAt: Date.now() - 86400000 * 5,
      updatedAt: Date.now() - 86400000 * 2
    },
    {
      id: 'doc-3',
      title: '今日心情',
      content: '<h1>今日心情</h1><p>天气晴朗，心情也跟着好了起来。下午去公园散了步，看到了很美的晚霞。</p>',
      folderId: '2-1',
      createdAt: Date.now() - 86400000 * 1,
      updatedAt: Date.now() - 3600000
    },
    {
      id: 'doc-4',
      title: '本周总结',
      content: '<h1>本周总结</h1><p>这周完成了项目的核心功能开发，下周计划开始做测试和优化。</p><h2>完成事项</h2><ul><li><p>前端三栏布局</p></li><li><p>编辑器集成</p></li></ul>',
      folderId: '2-2',
      createdAt: Date.now() - 86400000 * 7,
      updatedAt: Date.now() - 86400000 * 1
    },
    {
      id: 'doc-5',
      title: '好句摘抄',
      content: '<h1>好句摘抄</h1><blockquote><p>生活不是等待暴风雨过去，而是学会在雨中跳舞。</p></blockquote><blockquote><p>世界上只有一种真正的英雄主义，那就是在认清生活的真相后依然热爱生活。 —— 罗曼·罗兰</p></blockquote>',
      folderId: '3',
      createdAt: Date.now() - 86400000 * 10,
      updatedAt: Date.now() - 86400000 * 3
    },
    {
      id: 'doc-6',
      title: '购物清单',
      content: '<h1>购物清单</h1><ul data-type="taskList"><li data-type="taskItem" data-checked="false"><label><input type="checkbox"></label><div><p>牛奶</p></div></li><li data-type="taskItem" data-checked="false"><label><input type="checkbox"></label><div><p>面包</p></div></li><li data-type="taskItem" data-checked="true"><label><input type="checkbox" checked></label><div><p>鸡蛋</p></div></li></ul>',
      folderId: '4',
      createdAt: Date.now() - 86400000 * 2,
      updatedAt: Date.now() - 7200000
    },
    {
      id: 'doc-7',
      title: '项目方案书',
      content: '<h1>项目方案书</h1><h2>项目背景</h2><p>本项目旨在开发一款高效的桌面笔记应用，帮助用户更好地管理知识和想法。</p><h2>技术选型</h2><ul><li><p>前端：Vue 3 + TypeScript</p></li><li><p>桌面端：Electron</p></li><li><p>编辑器：Tiptap</p></li></ul>',
      folderId: '5',
      createdAt: Date.now() - 86400000 * 15,
      updatedAt: Date.now() - 86400000 * 5
    },
    {
      id: 'doc-8',
      title: '会议记录 04-25',
      content: '<h1>会议记录 04-25</h1><h2>参会人员</h2><p>张三、李四、王五</p><h2>会议内容</h2><p>讨论了下一阶段的产品规划，重点确认了用户管理模块的设计方案。</p>',
      folderId: '5',
      createdAt: Date.now() - 86400000 * 3,
      updatedAt: Date.now() - 86400000 * 3
    }
  ])

  // 当前选中的目录 ID
  const selectedFolderId = ref<string | null>(null)

  // 当前选中的文稿 ID
  const selectedDocumentId = ref<string | null>(null)

  // ============ 计算属性 ============

  // 当前选中目录下的文稿列表（按更新时间倒序）
  const currentFolderDocuments = computed<Document[]>(() => {
    if (!selectedFolderId.value) return []
    return documents.value
      .filter(doc => doc.folderId === selectedFolderId.value)
      .sort((a, b) => b.updatedAt - a.updatedAt)
  })

  // 当前选中的文稿
  const currentDocument = computed<Document | null>(() => {
    if (!selectedDocumentId.value) return null
    return documents.value.find(doc => doc.id === selectedDocumentId.value) || null
  })

  // ============ 操作 ============

  // 选中目录
  function selectFolder(folderId: string) {
    selectedFolderId.value = folderId

    // 自动选中该目录下的第一个文稿
    const folderDocs = documents.value
      .filter(doc => doc.folderId === folderId)
      .sort((a, b) => b.updatedAt - a.updatedAt)

    if (folderDocs.length > 0) {
      selectedDocumentId.value = folderDocs[0].id
    } else {
      selectedDocumentId.value = null
    }
  }

  // 选中文稿
  function selectDocument(documentId: string) {
    selectedDocumentId.value = documentId
  }

  // 更新文稿内容
  function updateDocumentContent(documentId: string, content: string) {
    const doc = documents.value.find(d => d.id === documentId)
    if (doc) {
      doc.content = content
      doc.updatedAt = Date.now()
    }
  }

  // 切换目录展开/折叠
  function toggleFolder(folder: FolderNode) {
    folder.isExpanded = !folder.isExpanded
  }

  // 重命名目录
  function renameFolder(folder: FolderNode, newName: string) {
    folder.name = newName
  }

  // 创建根级目录
  function createRootFolder() {
    const newFolder: FolderNode = {
      id: Date.now().toString(),
      name: '新建目录',
      children: [],
      parentId: null,
      isExpanded: false
    }
    folders.value.push(newFolder)
  }

  // 创建同级目录
  function createSiblingFolder(targetFolder: FolderNode) {
    const sibling: FolderNode = {
      id: Date.now().toString(),
      name: '新建目录',
      children: [],
      parentId: targetFolder.parentId,
      isExpanded: false
    }

    if (targetFolder.parentId === null) {
      folders.value.push(sibling)
    } else {
      addFolderToParent(folders.value, targetFolder.parentId, sibling)
    }
  }

  // 创建子目录
  function createChildFolder(parentFolder: FolderNode) {
    const child: FolderNode = {
      id: Date.now().toString(),
      name: '新建子目录',
      children: [],
      parentId: parentFolder.id,
      isExpanded: false
    }
    parentFolder.children.push(child)
    parentFolder.isExpanded = true
  }

  // 删除目录
  function deleteFolder(folder: FolderNode) {
    if (folder.parentId === null) {
      const index = folders.value.findIndex(f => f.id === folder.id)
      if (index !== -1) {
        folders.value.splice(index, 1)
      }
    } else {
      removeFolderFromParent(folders.value, folder.id)
    }

    // 如果删除的是当前选中目录，清空选中
    if (selectedFolderId.value === folder.id) {
      selectedFolderId.value = null
      selectedDocumentId.value = null
    }
  }

  // ============ 辅助函数 ============

  function addFolderToParent(nodes: FolderNode[], parentId: string, folder: FolderNode): boolean {
    for (const node of nodes) {
      if (node.id === parentId) {
        node.children.push(folder)
        return true
      }
      if (node.children.length > 0) {
        if (addFolderToParent(node.children, parentId, folder)) {
          return true
        }
      }
    }
    return false
  }

  function removeFolderFromParent(nodes: FolderNode[], folderId: string): boolean {
    for (const node of nodes) {
      const index = node.children.findIndex(f => f.id === folderId)
      if (index !== -1) {
        node.children.splice(index, 1)
        return true
      }
      if (node.children.length > 0) {
        if (removeFolderFromParent(node.children, folderId)) {
          return true
        }
      }
    }
    return false
  }

  // 获取文稿预览文本（取纯文本前80字符）
  function getDocumentPreview(doc: Document): string {
    const tmp = document.createElement('div')
    tmp.innerHTML = doc.content
    const text = tmp.textContent || tmp.innerText || ''
    // 跳过标题（第一行），取正文预览
    const lines = text.split('\n').filter(l => l.trim())
    const body = lines.slice(1).join(' ').trim()
    return body.length > 80 ? body.substring(0, 80) + '...' : body
  }

  return {
    // 状态
    folders,
    documents,
    selectedFolderId,
    selectedDocumentId,
    // 计算属性
    currentFolderDocuments,
    currentDocument,
    // 操作
    selectFolder,
    selectDocument,
    updateDocumentContent,
    toggleFolder,
    renameFolder,
    createRootFolder,
    createSiblingFolder,
    createChildFolder,
    deleteFolder,
    getDocumentPreview
  }
})
