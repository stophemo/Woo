import { chromium } from '/Users/return/.nvm/versions/node/v24.16.0/lib/node_modules/playwright/index.mjs';

(async () => {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 430, height: 932 } });
  const page = await context.newPage();

  // 浏览器 dev 模式下没有 Tauri 桥接，注入 mock 避免 IPC 报错影响截图
  await page.addInitScript(() => {
    window.__TAURI_INTERNALS__ = {
      invoke: async (cmd) => {
        const listCommands = ['folderTree', 'documentListAll', 'documentListTrash', 'documentListOrphans', 'documentSearch', 'documentListByFolder']
        if (listCommands.includes(cmd)) {
          return { ok: true, data: [] }
        }
        if (cmd === 'authGetSession') return { ok: true, data: null }
        if (cmd === 'lockGetStatus') return { ok: true, data: { hasPassword: false, mode: null } }
        if (cmd === 'syncStatus') return { ok: true, data: { isSyncing: false, lastSyncTime: null, pendingChanges: 0 } }
        if (cmd === 'syncTrigger') return { ok: true, data: { syncTime: new Date().toISOString() } }
        if (cmd === 'appGetVersion') return { ok: true, data: '0.5.1' }
        return { ok: true, data: null }
      },
      transformCallback: () => 0,
      unregisterCallback: () => {},
      convertFileSrc: (p) => p
    }
  })

  await page.goto('http://localhost:5173/');
  await page.waitForTimeout(2000);
  await page.screenshot({ path: '/tmp/woo-mobile-1-initial.png' });

  // 点击汉堡按钮打开左侧抽屉
  const hamburger = await page.$('.hamburger-btn');
  if (hamburger) {
    await hamburger.click();
    await page.waitForTimeout(500);
    await page.screenshot({ path: '/tmp/woo-mobile-2-left-sidebar.png' });
  }

  // 点击遮罩右侧空白处关闭左侧抽屉（小屏下遮罩中心可能被抽屉盖住）
  const overlay = await page.$('.left-sidebar-overlay');
  if (overlay) {
    await overlay.click({ position: { x: 360, y: 400 } });
    await page.waitForTimeout(300);
  }

  // 点击文稿列表按钮打开底部抽屉
  const docBtn = await page.$('.mobile-menu-right button[title="文稿列表"]');
  if (docBtn) {
    await docBtn.click();
    await page.waitForTimeout(500);
    await page.screenshot({ path: '/tmp/woo-mobile-3-doc-drawer.png' });
  }

  // 关闭文稿抽屉
  const closeBtn = await page.$('.mobile-doc-close');
  if (closeBtn) {
    await closeBtn.click();
    await page.waitForTimeout(300);
  }

  // 点击设置按钮
  const settingsBtn = await page.$('.mobile-menu-right button[title="设置"]');
  if (settingsBtn) {
    await settingsBtn.click();
    await page.waitForTimeout(500);
    await page.screenshot({ path: '/tmp/woo-mobile-4-settings.png' });
  }

  await browser.close();
})();
