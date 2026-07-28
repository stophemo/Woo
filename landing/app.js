const RELEASE_API = "https://api.github.com/repos/stophemo/Woo/releases/latest";

const demoNotes = {
  quiet: {
    kicker: "写作随想 · 7 月 28 日",
    title: "让工具退后，让思考向前",
    lead: "好的写作工具，不应该把注意力放在自己身上。",
    body: `
      <p>它应该足够快，让念头出现时不必等待；也应该足够安静，让界面在文字开始之后慢慢退场。</p>
      <blockquote>写作不是把句子搬进软件，而是为思考留出一块不被打扰的地方。</blockquote>
      <h3>保持简单，也保留余地</h3>
      <p>本地保存承担确定性，版本历史留住探索过程。云同步可以按需启用，但不应成为开始写作的前提。</p>
    `,
    count: "428 words",
  },
  sync: {
    kicker: "产品笔记 · 跨端体验",
    title: "跨端写作的理想节奏",
    lead: "设备不同，适合承接的思考也不同。",
    body: `
      <p>电脑适合长篇推进，手机适合捕捉瞬间。真正自然的跨端体验，不是把同一个界面缩放两遍。</p>
      <blockquote>同步的价值，是让上一次停笔的位置成为下一次打开时的起点。</blockquote>
      <h3>先本地，后同步</h3>
      <p>每台设备都先独立完成保存，再把增量变化交给同步引擎。网络不再是写作的前置条件。</p>
    `,
    count: "736 words",
  },
  reading: {
    kicker: "阅读手记 · 2026 年 7 月",
    title: "七月阅读手记",
    lead: "真正留下来的，不是读过多少，而是重新想过什么。",
    body: `
      <p>读完一本书之后，先不急着总结。把仍然在脑海里回响的句子写下来，再追问它为什么留下。</p>
      <blockquote>笔记不是阅读的收据，而是下一次思考的入口。</blockquote>
      <h3>把摘录变成自己的语言</h3>
      <p>沿着大纲整理主题，用版本记录观点如何变化。文字被重新组织之后，阅读才真正进入自己的经验。</p>
    `,
    count: "1,204 words",
  },
};

const noteButtons = [...document.querySelectorAll("[data-demo-note]")];
const editorKicker = document.querySelector("[data-editor-kicker]");
const editorTitle = document.querySelector("[data-editor-title]");
const editorLead = document.querySelector("[data-editor-lead]");
const editorBody = document.querySelector("[data-editor-body]");
const editorCount = document.querySelector("[data-editor-count]");

function selectDemoNote(key) {
  const note = demoNotes[key];
  if (!note || !editorKicker || !editorTitle || !editorLead || !editorBody || !editorCount) return;

  noteButtons.forEach((button) => {
    const isActive = button.dataset.demoNote === key;
    button.classList.toggle("is-active", isActive);
    button.setAttribute("aria-pressed", String(isActive));
  });

  editorKicker.textContent = note.kicker;
  editorTitle.textContent = note.title;
  editorLead.textContent = note.lead;
  editorBody.innerHTML = note.body;
  editorCount.textContent = note.count;
}

noteButtons.forEach((button) => {
  button.addEventListener("click", () => selectDemoNote(button.dataset.demoNote));
});

const nav = document.querySelector("[data-nav]");
const navToggle = document.querySelector("[data-nav-toggle]");

function setNavOpen(open) {
  if (!nav || !navToggle) return;
  nav.classList.toggle("is-open", open);
  navToggle.setAttribute("aria-expanded", String(open));
  navToggle.setAttribute("aria-label", open ? "关闭导航" : "打开导航");
}

navToggle?.addEventListener("click", () => {
  setNavOpen(navToggle.getAttribute("aria-expanded") !== "true");
});

nav?.querySelectorAll("a").forEach((link) => {
  link.addEventListener("click", () => setNavOpen(false));
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") setNavOpen(false);
});

document.addEventListener("click", (event) => {
  if (!nav || !navToggle || event.target instanceof Node === false) return;
  if (!nav.contains(event.target) && !navToggle.contains(event.target)) setNavOpen(false);
});

document.querySelectorAll("[data-current-year]").forEach((element) => {
  element.textContent = String(new Date().getFullYear());
});

function normalizedVersion(tag) {
  if (typeof tag !== "string") return null;
  if (!/^v?\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(tag)) return null;
  return tag.startsWith("v") ? tag : `v${tag}`;
}

function formatReleaseDate(value) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  return new Intl.DateTimeFormat("zh-CN", {
    timeZone: "Asia/Shanghai",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(date).replaceAll("/", "-");
}

function findAsset(assets, platform) {
  const rules = {
    macos: [/^Woo_macos-arm64\.dmg$/i, /macos-arm64.*\.dmg$/i],
    windows: [/^Woo_windows-x64-setup\.exe$/i, /windows-x64-setup\.exe$/i],
    android: [/^Woo_android-arm64-v8a\.apk$/i, /android-arm64-v8a\.apk$/i],
  };

  for (const rule of rules[platform] || []) {
    const asset = assets.find((item) => typeof item?.name === "string" && rule.test(item.name));
    if (asset) return asset;
  }
  return null;
}

fetch(RELEASE_API, {
  headers: { Accept: "application/vnd.github+json" },
})
  .then((response) => {
    if (!response.ok) throw new Error(`GitHub API returned ${response.status}`);
    return response.json();
  })
  .then((release) => {
    const version = normalizedVersion(release.tag_name);
    if (version) {
      document.querySelectorAll("[data-version]").forEach((element) => {
        element.textContent = version;
      });
    }

    const releaseDate = formatReleaseDate(release.published_at);
    if (releaseDate) {
      document.querySelectorAll("[data-release-date]").forEach((element) => {
        element.textContent = releaseDate;
      });
    }

    if (typeof release.html_url === "string") {
      document.querySelectorAll("[data-release-link]").forEach((link) => {
        link.href = release.html_url;
      });
    }

    const assets = Array.isArray(release.assets) ? release.assets : [];
    ["macos", "windows", "android"].forEach((platform) => {
      const asset = findAsset(assets, platform);
      if (!asset || typeof asset.browser_download_url !== "string") return;
      document.querySelectorAll(`[data-download="${platform}"]`).forEach((link) => {
        link.href = asset.browser_download_url;
      });
    });
  })
  .catch(() => {
    // 页面保留稳定的 latest 下载链接，GitHub API 不可用时仍可正常下载。
  });
