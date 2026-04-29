export interface Document {
  id: string
  title: string
  content: string          // HTML 内容（tiptap 编辑器格式）
  folderId: string         // 所属目录 ID
  createdAt: number        // 创建时间戳
  updatedAt: number        // 更新时间戳
}
