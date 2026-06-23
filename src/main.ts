import { createApp } from 'vue'
import { createPinia } from 'pinia'
import './style.css'
import App from './App.vue'
import { setupTauriBridge } from './setup'

// Surface uncaught errors to the DOM so we can see them when the app fails to mount.
function showError(label: string, err: unknown) {
  // eslint-disable-next-line no-console
  console.error(label, err)
  const root = document.getElementById('app')
  if (root && !root.dataset.errorRendered) {
    root.dataset.errorRendered = '1'
    const msg = err instanceof Error ? `${err.name}: ${err.message}\n${err.stack || ''}` : String(err)
    root.innerHTML = `
      <div style="position:fixed;inset:0;background:#1e1e1e;color:#f55;font-family:monospace;padding:24px;overflow:auto;white-space:pre-wrap;font-size:12px;z-index:99999">
        <div style="color:#fa5;font-weight:bold;margin-bottom:12px">${label}</div>
        <pre style="margin:0">${msg.replace(/[<>&]/g, c => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;' }[c]!))}</pre>
      </div>`
  }
}

window.addEventListener('error', (e) => showError('window.error', e.error || e.message))
window.addEventListener('unhandledrejection', (e) => showError('unhandledrejection', e.reason))

try {
  setupTauriBridge()
} catch (err) {
  showError('setupTauriBridge failed', err)
  // Don't bail: still try to mount the app so the user sees the rest of the UI.
}

try {
  const app = createApp(App)
  app.config.errorHandler = (err, _instance, info) => showError(`Vue error (${info})`, err)
  app.use(createPinia())
  app.mount('#app')
} catch (err) {
  showError('app.mount failed', err)
}
