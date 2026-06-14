# Repository Guidelines

## Project Structure & Module Organization

```
Woo/
├── app-desktop/        # Electron + Vue 3 + TypeScript desktop app
│   ├── electron/       # Main process (CJS): IPC handlers, services, DB
│   │   ├── ipc/        # IPC handler registration
│   │   ├── services/   # Business logic (auth, sync, documents, versions)
│   │   └── db/         # SQLite connection & schema migration
│   ├── src/            # Renderer process (ESM, Vue 3)
│   │   ├── stores/     # Pinia stores
│   │   ├── services/   # IPC client wrappers & AI API clients
│   │   ├── components/ # Layout, UI, and icon components
│   │   ├── config/     # Menu definitions, shortcut utils
│   │   └── types/      # TypeScript interfaces
│   └── scripts/        # Build helper scripts
├── app-mobile/         # Flutter mobile app (placeholder)
└── docs/               # Architecture docs & release guides
```

- `app-desktop/` is the primary codebase — all active development happens here.
- Source code lives under `electron/` (main process) and `src/` (renderer). Tests, if added, go in `__tests__/` alongside the module under test.

## Build, Test, and Development Commands

All commands run from `app-desktop/`:

| Command | Purpose |
|---|---|
| `npm run dev` | Start Vite dev server (renderer hot-reload at `localhost:5173`) |
| `npm run electron:dev` | Launch Electron in dev mode (requires `npm run dev` running) |
| `npm run build` | Type-check + build renderer only |
| `npm run electron:build` | Full release build (type-check + renderer + electron-builder) |
| `npm run rebuild:sqlite` | Rebuild native modules after Electron version upgrade |
| `npm run build:icons` | Generate app icons from source |

**Dev workflow**: Run two terminals — `npm run dev` + `npm run electron:dev`.

## Coding Style & Naming Conventions

- **Language**: TypeScript with strict mode enabled (`strict: true`, `noUnusedLocals`, `noUnusedParameters`).
- **Modules**: Main process uses CJS (`.cjs`); renderer uses ESM (`.ts`, `.vue`).
- **Naming**: Files use camelCase (`folderService.cjs`), Vue components use PascalCase (`EditArea.vue`). Pinia stores use camelCase (`useWorkspaceStore`).
- **Formatting**: No ESLint or Prettier — rely on TypeScript compiler checks. EditorConfig sets `charset = utf-8` only.
- **Mobile**: Flutter with `flutter_lints`; uses Riverpod for state management.

## Testing Guidelines

- The project currently has **no test framework or test runner** configured.
- When adding tests, place them in `__tests__/` directories co-located with the module under test.
- Flutter tests go in `app-mobile/test/` using the `flutter_test` package.

## Commit & Pull Request Guidelines

**Commit messages** follow [Conventional Commits](https://www.conventionalcommits.org/) with Chinese descriptions:

```
<type>(<scope>): <Chinese description>

feat(ci): release notes 支持自定义 + 简化自动生成
fix(app-desktop): 修复复制粘贴格式化丢失 & 代码块保护 & 组件卸载泄漏
chore: 将 .codegraph/ 加入 gitignore
docs: 重写 README 功能特性描述
refactor: remove dev auto-login, add 7-day session persistence
release: v0.4.8
```

Types: `feat`, `fix`, `chore`, `docs`, `refactor`, `perf`, `release`, `style`. Scope is optional (e.g. `app-desktop`, `ci`).

**Pull requests** should include:
- A clear description of the change and motivation.
- Reference to any related issue.
- Screenshots or screen recordings for UI changes.
- Release notes entry for user-facing changes.

## Architecture Overview

Woo is a **local-first** note-taking app: SQLite (via `better-sqlite3`) is the primary data store; Supabase cloud sync is optional. IPC follows a request–response pattern:

```
Renderer (Vue) -> window.woo.invoke(channel, args) -> ipcMain.handle -> Service -> SQLite/Supabase
```

All IPC handlers are wrapped with `wrap()` (catches errors -> `{ ok: false, message }`), and the renderer side uses `unwrap()` to re-throw on failure. See `CLAUDE.md` for detailed architecture documentation.
