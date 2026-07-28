const RELEASE_API = "https://api.github.com/repos/stophemo/Woo/releases/latest";

const demoNotes = {
  quiet: {
    kicker: "个人写作 · 7 月 28 日",
    title: "七月写作计划",
    lead: "这个月只做一件事：把三个零散主题写完整。",
    body: `
      <p>每天上午留出四十分钟，只推进一个段落。先写清楚，再决定哪些值得保留。</p>
      <blockquote>不追求一次成稿，先让每个想法都有落下来的地方。</blockquote>
      <h3>本周安排</h3>
      <p>周一整理材料，周三完成初稿，周末留给重读和删改。每次停笔前，写下下一步从哪里开始。</p>
    `,
    count: "428 words",
  },
  sync: {
    kicker: "产品复盘 · 移动体验",
    title: "减少切换，保留上下文",
    lead: "真正影响效率的，往往是每次切换后都要重新找回思路。",
    body: `
      <p>首页只保留最常用的入口，把搜索和新建放在拇指容易触达的位置。编辑时，尽量减少离开当前文稿的理由。</p>
      <blockquote>好的移动体验，不是缩小桌面界面，而是重新安排每一步的优先级。</blockquote>
      <h3>下一个迭代</h3>
      <p>缩短文稿切换路径，保留上次浏览位置，并把同步状态放在需要确认时才出现的地方。</p>
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
const downloadMenu = document.querySelector("[data-download-menu]");

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
  if (event.key !== "Escape") return;
  setNavOpen(false);
  downloadMenu?.removeAttribute("open");
});

document.addEventListener("click", (event) => {
  if (event.target instanceof Node === false) return;
  if (nav && navToggle && !nav.contains(event.target) && !navToggle.contains(event.target)) {
    setNavOpen(false);
  }
  if (downloadMenu && !downloadMenu.contains(event.target)) {
    downloadMenu.removeAttribute("open");
  }
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
