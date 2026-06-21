/**
 * Centralized frontend logger for Woo Tauri.
 * All console.* calls in the app should go through this module.
 * Error/warn logs are also forwarded to Rust's env_logger via IPC
 * so they appear in the terminal.
 *
 * Level control via VITE_LOG_LEVEL env var (debug|info|warn|error).
 * Default: info.
 */
import { invoke } from '@tauri-apps/api/core'

type LogLevel = 'debug' | 'info' | 'warn' | 'error'

const LEVEL_MAP: Record<LogLevel, number> = { debug: 0, info: 1, warn: 2, error: 3 }

function getCurrentLevel(): LogLevel {
  if (typeof import.meta !== 'undefined' && import.meta.env?.VITE_LOG_LEVEL) {
    const env = import.meta.env.VITE_LOG_LEVEL as string
    if (['debug', 'info', 'warn', 'error'].includes(env)) return env as LogLevel
  }
  return 'info'
}

const currentLevel = getCurrentLevel()

function forwardLog(level: string, tag: string, msg: string) {
  invoke('logWrite', { level, tag, message: msg }).catch(() => {})
}

export class Logger {
  constructor(private tag: string) {}

  debug(...args: any[]) { this.emit('debug', 'log', ...args) }
  info(...args: any[])  { this.emit('info', 'log', ...args) }
  warn(...args: any[])  { this.emit('warn', 'warn', ...args) }
  error(...args: any[]) { this.emit('error', 'error', ...args) }

  private emit(level: LogLevel, method: 'log' | 'warn' | 'error', ...args: any[]) {
    if (LEVEL_MAP[level] < LEVEL_MAP[currentLevel]) return
    const fn = console[method]
    const tag = this.tag
    const msg = args.map(a => typeof a === 'string' ? a : JSON.stringify(a)).join(' ')
    fn(`[${tag}]`, ...args)
    if (level === 'error' || level === 'warn') {
      forwardLog(level, tag, msg)
    }
  }
}

export const log = {
  db:     new Logger('DB'),
  sync:   new Logger('Sync'),
  agent:  new Logger('Agent'),
  ipc:    new Logger('IPC'),
  app:    new Logger('App'),
  editor: new Logger('Editor'),
  lock:   new Logger('Lock'),
  auth:   new Logger('Auth'),
}

export default log
