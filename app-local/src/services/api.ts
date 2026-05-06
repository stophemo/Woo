/**
 * IPC 客户端（本地纯简化版）。
 *
 * 设计目标：
 * - 无认证、无 token，所有 IPC 直接调用
 * - 主进程 handler 返回 { ok, data | message }
 * - 这里将其解包：ok=true → 返回 data；ok=false → throw Error(message)
 */

interface IpcResult<T> {
  ok: boolean
  data?: T
  message?: string
}

function getWoo(): { invoke: (channel: string, ...args: any[]) => Promise<IpcResult<any>> } {
  const w = (window as any).woo
  if (!w || typeof w.invoke !== 'function') {
    throw new Error('Electron 本地运行时未就绪')
  }
  return w
}

/**
 * 调用任意 IPC 通道。
 */
export async function invoke<T = unknown>(channel: string, ...args: any[]): Promise<T> {
  console.log('[API] invoke:', channel, args)
  const res = await getWoo().invoke(channel, ...args)
  console.log('[API] response:', res)
  return unwrap<T>(res)
}

function unwrap<T>(res: IpcResult<T>): T {
  if (res && res.ok) return res.data as T
  const msg = (res && res.message) || '操作失败'
  const err: any = new Error(msg)
  throw err
}

// 默认导出兼容旧写法
export default {
  invoke: (channel: string, ...args: any[]) => getWoo().invoke(channel, ...args)
}
