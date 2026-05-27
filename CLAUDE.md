# CLAUDE.md

请始终使用简体中文与我对话，并在回答时保持专业、简洁。
This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Dev Commands

```bash
cd app-desktop
npm install              # install dependencies; postinstall runs electron-builder install-app-deps
npm run dev              # start Vite dev server (renderer hot-reload on http://localhost:5173)
npm run preview          # Vite preview of built renderer
npm run electron:dev     # Electron in dev mode (loads Vite dev server URL). Requires `npm run dev` running.
npm run build            # vue-tsc + vite build (frontend only, no Electron package)
npm run electron:build   # vue-tsc + vite build + electron-builder (full release)
npm run rebuild:sqlite   # electron-rebuild better-sqlite3 after Electron version bump
npm run build:icons      # generate app icons from source

# Dev workflow — two concurrent terminals:
#   Terminal 1: npm run dev          (keeps Vite dev server alive)
#   Terminal 2: npm run electron:dev (launches Electron, loads from Vite)

# This project has zero tests — no test runner or test files exist.
# Build outputs → app-desktop/release/
#   woo-desktop-{version}-win-x64-setup.exe  (NSIS installer)
#   woo-desktop-{version}-win-x64-portable.exe
#   woo-desktop-{version}-win-x64.zip
```

## Architecture

**Woo (无我笔记)** is a local-first Markdown note-taking desktop app built with Electron + Vue 3. It uses SQLite (better-sqlite3) as the primary data store with optional Supabase cloud sync.

### IPC Layer

```
Renderer (Vue)  →  window.woo.invoke(channel, ...args)  →  ipcMain.handle  →  Service  →  SQLite/Supabase
                     │                                      │
                     │  Returns { ok, data }                │  wrap() catches errors
                     │  or { ok: false, message }           │  and returns { ok: false, message }
                     ▼                                      ▼
              api.ts  unwrap()                         register.cjs  wrap()
              throws on !ok                             catches throws
```

- All renderer-to-main IPC goes through `window.woo.invoke()` exposed via `preload.cjs` contextBridge.
- IPC handlers in `electron/ipc/register.cjs` use a `wrap()` helper that catches thrown errors → `{ ok: false, message }`.
- Renderer-side `api.ts` has a matching `unwarp()` that throws on `ok: false`, so service functions look like normal async calls.
- `window.electronAPI` is a separate contextBridge slot for Electron-native operations (window controls, menu actions, update checks).

### Structure

```
Woo/
├── app-desktop/                # Electron + Vue 3 desktop app
│   ├── electron/               # Main process (CJS modules)
│   │   ├── main.cjs            # Entry: window creation, menu, IPC registration, DB init, sync startup
│   │   ├── preload.cjs         # contextBridge API (electronAPI + woo.invoke)
│   │   ├── ipc/register.cjs    # All IPC handler registration (wrap() pattern)
│   │   ├── db/
│   │   │   ├── index.cjs       # Connection management (per-user DB files, auto-switch)
│   │   │   └── schema.cjs      # Schema init + ALTER TABLE migrations
│   │   ├── services/
│   │   │   ├── folderService.cjs, documentService.cjs, versionService.cjs
│   │   │   ├── authService.cjs       # Supabase Auth wrapper
│   │   │   └── syncEngine.cjs        # Supabase sync engine
│   │   └── config/supabase.cjs       # Supabase client singleton
│   └── src/                    # Renderer process (ESM, Vue 3)
│       ├── stores/              # Pinia stores: workspace, auth, sync, aiChat, theme
│       ├── services/            # IPC client wrappers (api.ts) + AI clients (gemini.ts, deepseek.ts)
│       ├── components/
│       │   ├── layout/          # App shell: EditArea, FolderTree, LeftSidebar, ThumbnailColumn, ...
│       │   ├── ui/              # Reusable UI components (ContextMenu, Dropdown, UpdateNotification)
│       │   └── icons/           # SVG icon components
│       ├── config/menus.ts      # Menu definitions
│       └── types/               # TypeScript interfaces (Document, FolderNode, ChatMessage, etc.)
└── app-mobile/                  # Flutter mobile app (placeholder)
```

### Key Architecture Decisions

- **Local-first with optional cloud sync**: SQLite (better-sqlite3) is primary data store. Supabase sync uses last-write-wins, incremental push/pull, and tombstone-based deletion propagation.
- **SQLite per user**: Unauthenticated → `woo.db`. After login → `woo-{username}.db`. First login copies local data to user DB. Switching users disconnects/reconnects automatically.
- **Content editing**: Tiptap (ProseMirror) WYSIWYG editor. Content stored as HTML. Title auto-extracted from first line.
- **Draft system**: Documents created without a folder stored in localStorage, not the database. Draft IDs prefixed `draft_`.
- **Conflict handling**: Sync detects remote changes conflicting with local unsaved edits → saves local version as `_conflict` copy, loads remote version.
- **Version control**: Auto-versions committed on significant changes (100 chars or 10 lines diff). 24h/7d layered merge strategy.
- **Window chrome**: frameless on Windows (custom title bar), native frame on macOS with hidden title + traffic light overlay.

### Sync Engine Design (`electron/services/syncEngine.cjs`)

1. **PULL tombstones** — reads `sync_tombstone` table from Supabase (records newer than last pull) → hard-deletes matching local rows + cascade-deletes versions.
2. **PULL remote changes** — reads Supabase records where `update_time > last_sync_time` → merges into local SQLite (last-write-wins: local wins if `local.update_time >= remote.update_time`).
3. **PUSH local changes** — reads local records where `update_time > last_sync_time` → upserts to Supabase.
4. **Cleanup** — scans local `deleted=2` records past cleanup window → hard-deletes from both DBs + writes tombstone to Supabase.
5. **Tombstone GC** — reclaims Supabase tombstones older than 30 days.

Sync runs every 60s. Auth state changes trigger immediate sync + DB switch.

### Soft-Delete Three-State Scheme

- `deleted = 0` — active
- `deleted = 1` — in trash (visible in trash view, can be restored)
- `deleted = 2` — pending cleanup (will be hard-deleted after cleanup window)

### Store Patterns (Pinia)

- **workspace store** — optimistic updates for folder/document operations (revert on error), 800ms debounced auto-save for content, merge-on-sync-refresh (diff-based, preserves Vue reactivity), `_conflict` document creation on sync collision.
- **auth store** — session persisted to localStorage for Electron restart recovery.
- **sync store** — listens to `sync-status` CustomEvents forwarded from main process via preload.

### AI Integration

- `src/services/gemini.ts` — Google Gemini API client
- `src/services/deepseek.ts` — DeepSeek API client
- `src/stores/aiChat.ts` — AI chat state management
- `app-desktop/src/components/layout/RightSidebar.vue` — AI chat panel
- Users configure API keys in settings (no bundled keys).

### Key Data Flow

1. `App.vue` mounts → `authStore.bootstrap()` restores session → `workspaceStore.bootstrap()` loads folder tree
2. User selects folder → `workspaceStore.selectFolder()` → loads document list via IPC
3. User selects document → `workspaceStore.selectDocument()` → loads full content via IPC
4. Editor edits → 800ms debounced auto-save via IPC, with auto-version commits on significant changes
5. Sync engine (main process, 60s interval) pushes local changes and pulls remote changes from Supabase
6. Sync completion → main process sends `sync:data-changed` → preload forwards as CustomEvent → workspaceStore.syncRefresh() does diff merge

### Database Schema (SQLite)

- `note_folder` — folder tree with `parent_id` for hierarchy, soft-delete via `deleted`
- `note_document` — documents with `folder_id`, HTML `content`, soft-delete
- `note_document_version` — version history with auto/manual/restore change types
- `sync_meta` — key-value store for sync state (last_sync_time, last_tombstone_pull)

### Build Tooling

- `vite-plugin-electron/simple` compiles Electron main + preload. However, the preload is loaded directly from source (`electron/preload.cjs`), not from the compiled output, to avoid CJS/ESM compatibility issues.
- electron-builder configured for Windows (NSIS/portable/zip), macOS (dmg/zip), and Linux (AppImage/deb).
- better-sqlite3 requires `asarUnpack` + `electron-rebuild` after Electron version changes.

## Code Conventions

- Electron main process uses `.cjs` (CJS); renderer uses ESM (`"type": "module"`)
- TypeScript strict mode with `noUnusedLocals` and `noUnusedParameters`
- Conventional commits: `feat:`, `fix:`, `chore:`, `release:`, with optional scope like `feat(app-desktop):`
- Supabase URL and anon key are committed (public by design, RLS secures data)
- Vue 3 Composition API (`<script setup lang="ts">`) throughout
- All IPC service functions return raw data (errors thrown). The IPC wrapper (`wrap()`) converts to `{ ok, data/message }` format.
- Pinia stores use Composition API style (`defineStore('name', () => { ... })`)

## Notes

- **`AGENTS.md`** at root is a superseded guidance file — should be deleted.
- **`.env`** uses `SYNC_CLEANUP_SECONDS=50` — this is an unusually short cleanup interval (likely for debugging). Adjust for production: `60*60*24*7` = 7 days.
- Root `vercel.json` exists for the auth-success landing page, not the desktop app itself.

## Git & Releases

- Release tags: `v*`, `Woo-desktop-v*`, or `app-desktop-v*`
- Push tag triggers GitHub Actions (`release-app-desktop.yml`) for macOS + Windows builds (5 artifacts)
- macOS: no code signing (`CSC_IDENTITY_AUTO_DISCOVERY: "false"`)
- Remote uses HTTPS; use `gh auth git-credential` helper
- CI generates release notes from conventional commits between tags

## 跨设备记忆同步（自动）

记忆文件在 `.claude/memory/`（纳入 git），当我执行 git 操作时自动同步，无需你手动干预。

### 自动规则

- **拉取代码时**：`git pull` 后自动执行 `./scripts/sync-memory.sh --pull`，将远端记忆写入本地系统路径
- **提交推送时**：提交前自动执行 `./scripts/sync-memory.sh` 将本地系统记忆同步到 `.claude/memory/`，然后 `git add .claude/memory/` 纳入提交

### skill

- `.claude/skills/sync-memory.md` — 可通过 `/sync-memory` 手动调用同步
