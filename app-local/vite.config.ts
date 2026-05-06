import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import electron from 'vite-plugin-electron'

// 本地版：Electron 主入口为 electron/main.cjs，主进程通过 IPC 直接访问 SQLite
export default defineConfig({
  plugins: [
    vue(),
    electron({
      entry: 'electron/main.cjs'
    })
  ],
  server: {
    port: 5173
  },
  build: {
    outDir: 'dist'
  }
})
