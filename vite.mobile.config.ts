import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { readFileSync } from 'fs'

const pkg = JSON.parse(readFileSync('./package.json', 'utf-8'))

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
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  envDir: '.',
  clearScreen: false,
})
