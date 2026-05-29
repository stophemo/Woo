import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import electronSimpleImport from 'vite-plugin-electron/simple'

const electronSimple = (electronSimpleImport as any).default ?? electronSimpleImport

// 本地版：Electron 主入口为 electron/main.cjs，主进程通过 IPC 直接访问 SQLite
export default defineConfig(async () => {
  const electronPlugins = await electronSimple({
    main: {
      entry: 'electron/main.cjs'
    },
    preload: {
      entry: 'electron/preload.cjs'
    }
  })
  return {
    plugins: [
      vue(),
      ...electronPlugins
    ],
    server: {
      port: 5173
    },
    build: {
      outDir: 'dist'
    },
    envDir: '..'
  }
})
