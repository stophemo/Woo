# Repository Guidelines

## Project Structure & Module Organization

```
Woo/
├── app-desktop/            # Electron + Vue 3 + TypeScript (active development)
│   ├── electron/           # Main process (CJS): IPC handlers, services, DB
│   │   ├── ipc/register.cjs      # All IPC handlers registered with wrap()
│   │   ├── db/index.cjs          # SQLite connection (per-user db switching)
│   │   ├── db/schema.cjs         # Schema + ALTER TABLE migrations
│   │   ├── services/             # folderService, documentService, versionService,
│   │   │                         # authService, syncEngine, lockService, kbService,
│   │   │                         # embeddingService, utils
│   │   └── config/supabase.cjs   # Supabase client singleton
│   ├── src/                     # Renderer (ESM, Vue 3, Composition API)
│   │   ├── stores/              # Pinia stores (workspace, auth, sync, aiChat, theme)
│   │   ├── services/api.ts      # IPC client: unwrap() on window.woo.invoke()
│   │   ├── services/gemini.ts, deepseek.ts  # AI API clients
│   │   └── components/layout/   # EditArea, FolderTree, LeftSidebar, RightSidebar, ...
│   └── scripts/                # Build helper scripts
├── app-mobile/              # Flutter mobile app (placeholder, sqflite + Riverpod)
└── docs/                    # Release guides, auth success page
```

- `app-desktop/` is the primary codebase. All commands run from `app-desktop/`.
- Source code: `electron/` (main process, CJS `.cjs`), `src/` (renderer, ESM `.ts`/`.vue`).

## Development Commands

| Command | Purpose |
|---|---|
| `npm run dev` | Start Vite dev server (hot-reload at `localhost:5173`) |
| `npm run electron:dev` | Launch Electron in dev mode (needs `npm run dev` running) |
| `npm run build` | `vue-tsc` + `vite build` (renderer only) |
| `npm run electron:build` | `vue-tsc` + `vite build` + `electron-builder` (full release) |
| `npm run rebuild:sqlite` | `electron-rebuild` for `better-sqlite3` + `sqlite-vec` |
| `npm run preview` | Preview built renderer |
| `npm run build:icons` | Generate app icons |

**Dev workflow**: two terminals — `npm run dev` + `npm run electron:dev`.

**Postinstall** runs `electron-builder install-app-deps` (rebuilds native modules).

**Important env vars** (loaded from root `.env` via Vite `envDir: '..'`):
- `ELECTRON_DEVTOOLS=1` — enable DevTools in dev mode
- `LOG_LEVEL=verbose` — verbose IPC handler logging
- `SYNC_CLEANUP_SECONDS` — sync cleanup interval (default: `60*60*24*7` = 7 days)

## Architecture

```
Renderer (Vue) → window.woo.invoke(channel, args) → ipcMain.handle → wrap() → Service → SQLite/Supabase
                 │                                     │
                 │ Returns { ok, data }                 │ wrap() catches errors →
                 │ or { ok: false, message }            │ { ok: false, message }
                 ▼                                     ▼
          api.ts unwrap()                         register.cjs wrap()
          throws on !ok
```

- Two separate `contextBridge` slots: `window.woo.invoke()` (business IPC) and `window.electronAPI` (native ops: window controls, menu actions, update check).
- Preload loaded directly from `electron/preload.cjs` source (not compiled output) to avoid CJS/ESM issues.
- `better-sqlite3` + `sqlite-vec` need `asarUnpack` in electron-builder config and `electron-rebuild` after Electron version change.
- DB: per-user databases (`woo.db` offline, `woo-{username}.db` after login). Session persisted via `supabase-auth.json`. Portable mode stores data in exe-relative `userData/`.
- Content stored as HTML (Tiptap/ProseMirror). Title auto-extracted from first line.
- Drafts: documents without a folder stored in localStorage (`draft_` prefix).
- Sync engine runs every 60s in main process, uses last-write-wins + tombstone delete propagation.

## Coding Style

- TypeScript strict mode: `strict: true`, `noUnusedLocals`, `noUnusedParameters`.
- Files: camelCase (`folderService.cjs`). Vue components: PascalCase (`EditArea.vue`). Stores: camelCase (`useWorkspaceStore`).
- No ESLint or Prettier — rely on TypeScript compiler checks only.
- Vue 3 `<script setup lang="ts">` everywhere. Pinia stores use Composition API style.
- All IPC services return raw data (errors thrown). `wrap()` converts to `{ ok, data/message }`.

## Tests

No test framework or runner configured. If adding tests, place in `__tests__/` alongside the module.

## IPC Channels (electron/ipc/register.cjs)

- **folder:** `tree`, `create`, `rename`, `remove`, `reorder`
- **document:** `listByFolder`, `listAll`, `listTrash`, `listOrphans`, `search`, `get`, `create`, `rename`, `updateContent`, `remove`, `restore`, `hardDelete`, `emptyTrash`, `reorder`
- **version:** `list`, `get`, `saveManual`, `commit`, `restore`
- **auth:** `signUp`, `signIn`, `signOut`, `getUser`, `getSession`
- **lock:** `status`, `setPassword`, `verifyPassword`, `lockFolder`, `unlockFolder`, `isFolderLocked`, `isFolderEffectivelyLocked`, `lockDocument`, `unlockDocument`, `isDocumentLocked`, `cloudPushSettings`, `cloudPullSettings`
- **system:** `setLogLevel`, `getLogLevel`
- **kb:** `rebuild`, `search`, `status`
- **sync:** `status`, `trigger`

## Commit & Release

- [Conventional Commits](https://www.conventionalcommits.org/) with Chinese descriptions: `feat(scope): 中文描述`
- Types: `feat`, `fix`, `chore`, `docs`, `refactor`, `perf`, `release`, `style`. Scope optional (`app-desktop`, `ci`).
- Tags matching `v*`, `Woo-desktop-v*`, `app-desktop-v*` trigger GitHub Actions (`release-app-desktop.yml`).
- macOS builds have no code signing (`CSC_IDENTITY_AUTO_DISCOVERY: "false"`).
- Prefer pushing a new tag over force-pushing an existing one.
