import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'path'

// Vue 3 + TypeScript - 移动端 (Vant UI)
export default defineConfig({
  plugins: [vue()],
  server: {
    port: 5174,
    strictPort: true,
    host: true,
  },
  build: {
    outDir: 'dist-mobile',
    cssCodeSplit: false,
    crossorigin: false,
  },
  resolve: {
    alias: {
      '@shared': resolve(__dirname, 'src'),
    },
  },
  envDir: '.',
  clearScreen: false,
})
