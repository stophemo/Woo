import { defineConfig, type Plugin } from 'vite'
import vue from '@vitejs/plugin-vue'
import { readFileSync, existsSync, renameSync } from 'fs'
import { resolve } from 'path'

const pkg = JSON.parse(readFileSync('./package.json', 'utf-8'))

// Vue 3 + TypeScript - 移动端 (Vant UI)
//
// 移动端入口为 index-mobile.html（→ /src-mobile/main.ts）。Vite 默认以根目录
// index.html（桌面入口）为文档，因此需显式把 dev 与 build 都切换到移动端入口，
// 否则会错误地构建/serve 桌面端。
function mobileEntry(): Plugin {
  return {
    name: 'woo-mobile-entry',
    // 开发态：把根路径请求重写到 index-mobile.html（在 Vite 内置 html 中间件之前执行）
    configureServer(server) {
      server.middlewares.use((req, _res, next) => {
        if (req.url === '/' || req.url === '/index.html') {
          req.url = '/index-mobile.html'
        }
        next()
      })
    },
    // 构建态：产物 index-mobile.html 重命名为 index.html，供 Tauri frontendDist 加载
    closeBundle() {
      const from = resolve('dist-mobile/index-mobile.html')
      const to = resolve('dist-mobile/index.html')
      if (existsSync(from)) renameSync(from, to)
    },
  }
}

export default defineConfig({
  plugins: [vue(), mobileEntry()],
  server: {
    port: 5174,
    strictPort: true,
    host: true,
  },
  build: {
    outDir: 'dist-mobile',
    cssCodeSplit: false,
    crossorigin: false,
    rollupOptions: {
      input: resolve('index-mobile.html'),
    },
  },
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  envDir: '.',
  clearScreen: false,
})
