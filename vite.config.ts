import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { readFileSync } from 'fs'

const packageInfo = JSON.parse(readFileSync('./package.json', 'utf-8')) as { version: string }

// Tauri v2 + Vue 3 + TypeScript
// dev server port must match tauri.conf.json > devUrl
export default defineConfig({
  plugins: [vue()],
  server: {
    port: 5173,
    strictPort: true,
    // Allow Tauri on Android/iOS simulators/devices to reach the dev server.
    host: true,
  },
  build: {
    outDir: 'dist',
    // Tauri custom protocol doesn't support CORS crossorigin attribute
    cssCodeSplit: false,
    crossorigin: false,
  },
  define: {
    __APP_VERSION__: JSON.stringify(packageInfo.version),
  },
  envDir: '.',
  clearScreen: false,
})
