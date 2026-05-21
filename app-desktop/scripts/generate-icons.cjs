/**
 * 图标生成脚本
 * -------------------------------------------------
 * 读取 build/icon.svg，利用 Electron 自带的 Chromium
 * 渲染为多个尺寸的 PNG，并合并打包成 Windows .ico。
 *
 * 运行：npm run build:icons
 */
const { app, BrowserWindow } = require('electron')
const path = require('path')
const fs = require('fs')

const ROOT = path.join(__dirname, '..')
const BUILD_DIR = path.join(ROOT, 'build')
const SVG_PATH = path.join(BUILD_DIR, 'icon.svg')
// 同步一份到 public 目录，打包后能直接被 main.cjs 在生产环境引用
const PUBLIC_DIR = path.join(ROOT, 'public')

const PNG_SIZES = [16, 24, 32, 48, 64, 128, 256, 512]
// Windows .ico 通常最大使用 256
const ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]

function buildIco(entries) {
  const count = entries.length
  const headerSize = 6 + count * 16
  const totalDataSize = entries.reduce((s, e) => s + e.data.length, 0)
  const buf = Buffer.alloc(headerSize + totalDataSize)

  // ICONDIR
  buf.writeUInt16LE(0, 0)          // reserved
  buf.writeUInt16LE(1, 2)          // type: 1 = icon
  buf.writeUInt16LE(count, 4)      // count

  let offset = headerSize
  for (let i = 0; i < count; i++) {
    const e = entries[i]
    const dir = 6 + i * 16
    const w = e.size >= 256 ? 0 : e.size
    const h = e.size >= 256 ? 0 : e.size
    buf.writeUInt8(w, dir + 0)     // width
    buf.writeUInt8(h, dir + 1)     // height
    buf.writeUInt8(0, dir + 2)     // color count
    buf.writeUInt8(0, dir + 3)     // reserved
    buf.writeUInt16LE(1, dir + 4)  // color planes
    buf.writeUInt16LE(32, dir + 6) // bits per pixel
    buf.writeUInt32LE(e.data.length, dir + 8)
    buf.writeUInt32LE(offset, dir + 12)
    e.data.copy(buf, offset)
    offset += e.data.length
  }
  return buf
}

async function run() {
  if (!fs.existsSync(SVG_PATH)) {
    console.error('[icons] 找不到源文件：', SVG_PATH)
    app.exit(1)
    return
  }
  if (!fs.existsSync(PUBLIC_DIR)) fs.mkdirSync(PUBLIC_DIR, { recursive: true })

  const svg = fs.readFileSync(SVG_PATH, 'utf8')

  const win = new BrowserWindow({
    width: 600,
    height: 600,
    show: false,
    webPreferences: {
      offscreen: true,
      contextIsolation: true,
      nodeIntegration: false,
    },
  })

  const html = `<!doctype html>
<html>
<body style="margin:0;padding:0;background:transparent">
<script>
  const svgSource = ${JSON.stringify(svg)};
  async function render(sizes) {
    const blob = new Blob([svgSource], { type: 'image/svg+xml' });
    const url = URL.createObjectURL(blob);
    const img = new Image();
    await new Promise((resolve, reject) => {
      img.onload = resolve;
      img.onerror = reject;
      img.src = url;
    });
    const out = {};
    for (const s of sizes) {
      const canvas = document.createElement('canvas');
      canvas.width = s; canvas.height = s;
      const ctx = canvas.getContext('2d');
      ctx.imageSmoothingEnabled = true;
      ctx.imageSmoothingQuality = 'high';
      ctx.clearRect(0, 0, s, s);
      ctx.drawImage(img, 0, 0, s, s);
      out[s] = canvas.toDataURL('image/png');
    }
    URL.revokeObjectURL(url);
    return JSON.stringify(out);
  }
  window.__render = render;
</script>
</body>
</html>`

  await win.loadURL('data:text/html;charset=utf-8,' + encodeURIComponent(html))

  const sizes = Array.from(new Set([...PNG_SIZES, ...ICO_SIZES])).sort((a, b) => a - b)
  const resultStr = await win.webContents.executeJavaScript(
    `window.__render(${JSON.stringify(sizes)})`
  )
  const results = JSON.parse(resultStr)

  // 写 PNG
  const pngBuffers = {}
  for (const s of sizes) {
    const base64 = results[s].replace(/^data:image\/png;base64,/, '')
    const buf = Buffer.from(base64, 'base64')
    pngBuffers[s] = buf
    if (PNG_SIZES.includes(s)) {
      const outPath = path.join(BUILD_DIR, `icon-${s}.png`)
      fs.writeFileSync(outPath, buf)
      console.log('[icons] 生成 PNG：', outPath)
    }
  }

  // 主 PNG（512）同时复制到 build/icon.png 与 public/icon.png，方便运行时引用
  const mainPng = pngBuffers[512]
  fs.writeFileSync(path.join(BUILD_DIR, 'icon.png'), mainPng)
  fs.writeFileSync(path.join(PUBLIC_DIR, 'icon.png'), mainPng)
  console.log('[icons] 生成主图：build/icon.png、public/icon.png')

  // 生成 ICO
  const icoEntries = ICO_SIZES.map(s => ({ size: s, data: pngBuffers[s] }))
  const ico = buildIco(icoEntries)
  const icoPath = path.join(BUILD_DIR, 'icon.ico')
  fs.writeFileSync(icoPath, ico)
  console.log('[icons] 生成 ICO：', icoPath)

  win.close()
  app.quit()
}

app.whenReady().then(run).catch(err => {
  console.error('[icons] 生成失败：', err)
  app.exit(1)
})
