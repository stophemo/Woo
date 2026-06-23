/**
 * Agent 安全确认机制。
 *
 * 破坏性操作（修改、删除笔记/目录）在执行前需要用户通过 UI 弹窗确认。
 * 工具 handler 调用 requestConfirmation() 返回 Promise<boolean>，
 * 自然地暂停 Agent 循环，等待用户在确认对话框中点击确认或取消。
 *
 * 该模块使用 Vue ref 提供共享响应式状态，tools.ts 和 ConfirmationDialog.vue 共同引用。
 */
import { ref } from 'vue'

export interface PendingConfirmation {
  /** 操作标识，如 "update_note"、"delete_note"、"delete_folder" */
  operation: string
  /** 人类可读的描述，如 '将修改笔记「会议记录」的全部内容' */
  details: string
  /** 用于解析 Promise 的 resolve 函数 */
  resolve: (value: boolean) => void
}

/** 共享响应式状态 — 被 tools.ts 和 ConfirmationDialog.vue 共同引用 */
export const pendingConfirmation = ref<PendingConfirmation | null>(null)

/**
 * 请求用户确认。
 * 工具 handler 调用此方法后，Agent 循环会暂停，直到用户在 UI 中确认或取消。
 *
 * @param operation 操作标识
 * @param details 显示给用户的操作描述
 * @returns Promise<boolean> — true 表示用户确认，false 表示取消
 */
export function requestConfirmation(operation: string, details: string): Promise<boolean> {
  return new Promise(resolve => {
    pendingConfirmation.value = { operation, details, resolve }
  })
}

/**
 * 用户点击确认/取消后由 ConfirmationDialog.vue 调用。
 * 解析待处理的 Promise 并清除状态。
 */
export function resolveConfirmation(confirmed: boolean) {
  const p = pendingConfirmation.value
  if (p) {
    p.resolve(confirmed)
    pendingConfirmation.value = null
  }
}

/**
 * 取消生成时调用（如用户点击停止按钮），自动拒绝待处理的确认。
 * 由 aiChat.ts 的 cancelGeneration() 调用。
 */
export function clearConfirmation() {
  const p = pendingConfirmation.value
  if (p) {
    p.resolve(false)
    pendingConfirmation.value = null
  }
}
