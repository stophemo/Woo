/**
 * Agent 终端日志工具。
 * 通过 IPC 将日志发到主进程，输出到终端（而非仅 DevTools）。
 */

type LogLevel = 'log' | 'warn' | 'error'

function ipcLog(level: LogLevel, ...args: any[]) {
  try {
    const w = (window as any).woo
    if (w && typeof w.invoke === 'function') {
      const msg = args.map(a => typeof a === 'string' ? a : JSON.stringify(a)).join(' ')
      w.invoke('log:agent', level, msg).catch(() => {})
    }
  } catch {
    // IPC 不可用时静默降级
  }
  // 同时也输出到 DevTools（备查）
  if (level === 'error') console.error('[Agent]', ...args)
  else if (level === 'warn') console.warn('[Agent]', ...args)
  else console.log('[Agent]', ...args)
}

export const agentLog = {
  log: (...args: any[]) => ipcLog('log', ...args),
  warn: (...args: any[]) => ipcLog('warn', ...args),
  error: (...args: any[]) => ipcLog('error', ...args),
}
