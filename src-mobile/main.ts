import { createApp } from 'vue'
import { createPinia } from 'pinia'
import Vant from 'vant'
import 'vant/lib/index.css'
import './assets/vant-icon.css' // 覆盖 Vant 图标字体为本地 woff2，避免 CSP 拦截 alicdn
import App from './App.vue'
import router from './router'

const app = createApp(App)
app.use(createPinia())
app.use(Vant)
app.use(router)
app.mount('#app')
