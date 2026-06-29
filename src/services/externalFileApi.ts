/**
 * 外部文件接口：读取/写入通过 "打开方式" 或拖拽打开的外部文件。
 * 这些文件不存储在数据库中，直接编辑源文件路径。
 */
import { invoke } from './api'

export interface ReadExternalFileResult {
  filePath: string
  fileName: string
  content: string
}

/** 读取外部文件内容 */
export function readExternalFile(filePath: string): Promise<ReadExternalFileResult> {
  return invoke<ReadExternalFileResult>('readExternalFile', { filePath })
}

/** 将内容写回外部文件 */
export function writeExternalFile(filePath: string, content: string): Promise<void> {
  return invoke<void>('writeExternalFile', { filePath, content })
}

/** 拉取前端就绪前收到的文件打开请求（启动时 "打开方式" 缓冲的路径） */
export function popPendingOpenFiles(): Promise<string[]> {
  return invoke<string[]>('popPendingOpenFiles')
}
