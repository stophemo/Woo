export interface Document {
  id: string
  title: string
  content: string          // HTML 内容（tiptap 编辑器格式）
  folderId: string         // 所属目录 ID
  createdAt: string        // ISO 时间串
  updatedAt: string        // ISO 时间串
}
