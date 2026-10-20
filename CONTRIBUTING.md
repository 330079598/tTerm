# Contributing to tTerm

Thanks for helping out. Issues and pull requests are both welcome.

The rules for changing the code (size limits, where code goes, Rust and
TypeScript conventions, commit messages) live in [AGENTS.md](./AGENTS.md). They
apply to people and coding agents alike, so read it before your first pull
request.

## Prerequisites

- Node.js 20.19 or later (CI uses the current LTS)
- pnpm 9. Use pnpm, not npm or yarn.
- Rust, latest stable
- The [Tauri 2 system dependencies](https://tauri.app/start/prerequisites/) for your platform:
  - **Windows**: Microsoft C++ Build Tools
  - **macOS**: Xcode Command Line Tools
  - **Linux**: WebKitGTK, OpenSSL, AppIndicator and the other Tauri packages

## Build and run

```bash
pnpm install
pnpm tauri:dev
```

`pnpm tauri:dev` runs the app as `tTerm Dev` (`com.stone.tTerm.dev`), so it can
run next to an installed tTerm and keeps its own database, settings and WebView
storage.

On Linux, if the Wayland backend exits with a GDK protocol error or WebKitGTK
reports GBM buffer errors, use the XWayland-compatible command instead. It runs
the app with `GDK_BACKEND=x11` and disables WebKitGTK's DMABUF renderer, and it
needs XWayland installed.

```bash
pnpm tauri:dev:linux
```

To build the desktop app:

```bash
pnpm tauri build
```

Tauri writes the platform bundles under `src-tauri/target`.

## Checks

Every change must pass these. CI runs the same ones in
`.github/workflows/check.yml`, with clippy and the Rust tests on Linux, Windows
and macOS.

```bash
pnpm lint && pnpm format:check && pnpm typecheck && pnpm test && pnpm check:size
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

Don't silence a failing check to get it green: no `--no-verify`, no
`eslint-disable`, no blanket `#[allow]`, and no raising a size limit or a
baseline entry. [AGENTS.md](./AGENTS.md) explains the file and function size
limits and how to split a file that is already too big.

## Scripts

```bash
pnpm dev              # Frontend dev server only
pnpm build            # Build frontend assets
pnpm preview          # Preview the frontend build
pnpm tauri:dev        # Run the app (separate tTerm Dev identity and data)
pnpm tauri:dev:linux  # Run the app on Linux through XWayland
pnpm tauri build      # Build the desktop app
pnpm test             # Frontend tests (Vitest)
pnpm lint             # ESLint
pnpm lint:fix         # ESLint with automatic fixes
pnpm format           # Format with Prettier
pnpm format:check     # Check formatting
pnpm typecheck        # TypeScript type check
pnpm check:size       # File size limits
pnpm themes:update    # Refresh the theme library from iTerm2-Color-Schemes
pnpm licenses         # Regenerate the third-party license list
```

## How tTerm is built

The frontend calls Rust commands through Tauri `invoke`. Local terminals run on
`portable-pty`; SSH and SFTP run on `russh` and `russh-sftp`.

Settings stay in `config.json`. Everything else (connections, tunnels, known
hosts, commands, custom themes, the restored session, backup and sync settings,
and encrypted secrets) lives in a local SQLite database, `tterm.db`. Saved
passwords use envelope encryption: each secret is encrypted with a data key,
and only that key is protected by the system credential store or a master
password.

### Tech stack

- **Frontend**: React 19, TypeScript, Vite, TanStack Router, xterm.js (WebGL and
  Canvas renderers), dockview, CodeMirror 6, dnd-kit, i18next, Radix UI,
  lucide-react, Tailwind CSS 4
- **Backend**: Tauri 2, Rust, Tokio, portable-pty, russh, russh-sftp, SQLite
  (rusqlite), keyring, aes-gcm, argon2, zeroize

### Project structure

```text
.
├── src/                      # React frontend
│   ├── components/           # Terminal, SFTP, tunnels, broadcast, settings, theme and UI components
│   ├── contexts/             # Global config, theme, keymap and transfer state
│   ├── hooks/                # Hooks shared by more than one feature
│   ├── i18n/                 # English and Chinese locale resources
│   ├── lib/                  # Pure logic with no React, each module with its own tests
│   ├── routes/               # TanStack Router pages
│   └── types/                # Types shared across features
├── src-tauri/src/            # Rust backend, one module per domain
│   ├── backup/               # Backup export and import, scheduled backups, WebDAV backup
│   ├── command_library/      # Saved commands and tags
│   ├── config/               # App configuration and paths
│   ├── core/                 # PTY, commands and app state
│   ├── db/                   # SQLite schema, migrations and legacy JSON import
│   ├── monitor/              # Server monitor metrics over SSH
│   ├── profiles/             # Connection profiles and SSH config import
│   ├── sftp/                 # SFTP connections, file operations and transfers
│   ├── ssh/                  # SSH client, agent, jump hosts, host keys and the secret store
│   ├── sync/                 # Multi-device sync over WebDAV (three-way merge)
│   ├── terminal/             # Terminal types and I/O
│   ├── tunnel/               # Port forwarding, SOCKS5 and the shared-connection hub
│   └── zmodem/               # ZMODEM (rz/sz) engine, trigger detection, send and receive
├── docs/                     # Documentation and screenshots
└── scripts/                  # Size check, theme and license generators
```

### Things to watch

- Frontend interactions should hold up with many tabs, split panes and
  transfers running at once.
- Rust commands should return errors the user can act on, since the UI shows
  them as they are.
- File transfers should expose progress, cancellation and failure states.
- Password, key and host-fingerprint flows must stay local, safe and explicit.
- New persisted data needs a database migration
  (`src-tauri/src/db/migrations.rs`) and must be covered by backup and import.
- User-facing text goes through i18next, with every key in both
  `src/i18n/locales/en.json` and `src/i18n/locales/zh.json`.

For how ZMODEM works and how to run its integration tests against real
`lrzsz`, see [docs/zmodem.md](./docs/zmodem.md#implementation).

## Releases

GitHub Actions builds the releases:

- **release-main.yml** runs on pushes to `main` and on `v*` tags. It builds
  macOS (Apple Silicon and Intel), Ubuntu 24.04 and Windows, and creates a
  draft GitHub release.
- **release-dev-beta.yml** runs on pushes to `dev-beta` and builds beta
  pre-releases for the Beta Dev update channel.
- **sync-release-notes.yml** is started by hand and copies release notes into
  a channel's updater manifest.
