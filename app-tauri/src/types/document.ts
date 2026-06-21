export interface Document {
  id: string
  title: string
  content: string          // HTML 内容（tiptap 编辑器格式）
  folderId: string         // 所属目录 ID
  folderName?: string       // 目录名称（仅全部视图时填充）
  createdAt: string        // ISO 时间串
  updatedAt: string        // ISO 时间串
  isLocked?: boolean       // 是否被锁定
}
