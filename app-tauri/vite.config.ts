import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// Tauri v2 + Vue 3 + TypeScript
// dev server port must match tauri.conf.json > devUrl
export default defineConfig({
  plugins: [vue()],
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    outDir: 'dist',
    // Tauri custom protocol doesn't support CORS crossorigin attribute
    cssCodeSplit: false,
    crossorigin: false,
  },
  envDir: '..',
  clearScreen: false,
})
