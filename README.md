# tTerm — A Modern SSH Terminal with Built-in SFTP

English | [简体中文](./README.zh-CN.md)

`tTerm` is a desktop terminal app for developers, operators, and anyone who works with remote servers. It brings **local terminals, SSH sessions, split panes, SFTP file management, port forwarding, server monitoring, and encrypted credential storage** into one lightweight application.

If you often connect to servers, move files between local and remote machines, or juggle many terminal sessions at once, tTerm is designed to be the tool you can open and start working with immediately.

![main-overview](./docs/screenshot/main-overview.png)

## Why tTerm?

- **Terminal and files in one place**: Open the SFTP drawer from an SSH session to browse, transfer, and edit remote files without switching apps.
- **Built for many servers at once**: Split the workspace into panes, broadcast a command to several terminals, and keep frequently used commands in a searchable library.
- **Fast, resumable transfers**: Parallel SFTP channels, resume after interruption, conflict handling, and bandwidth limits. ZMODEM (`rz`/`sz`) covers hosts without SFTP.
- **Visual port forwarding**: Manage local, remote, and dynamic (SOCKS5) SSH tunnels from a dedicated page, with live status, traffic counters, and automatic reconnects.
- **See the server at a glance**: A monitor bar under each SSH session shows CPU, memory, network, disk, and latency, with nothing to install on the server.
- **Safer SSH workflow**: Host key confirmation, SSH agent and jump-host support, and saved passwords encrypted locally with AES-256-GCM.
- **Modern desktop experience**: Built with Tauri for a lightweight, fast, cross-platform app with themes, customizable shortcuts, and English/Chinese UI.

## Key Features

### Workspace and Tabs

- Multiple tabs with drag-and-drop reordering, duplicate, rename, and close others / left / right
- Split panes: split any tab left, right, up, or down, drag tabs between groups, and maximize a group
- Tab search and overview when many tabs are open; adaptive or fixed tab width
- Session and layout restore after restart, optionally connecting only the active tab at startup
- Confirmation before closing tabs with active transfers or quitting with running tunnels

### Terminal

- Local shell with auto-detection, or pick one (on Windows: cmd, PowerShell, PowerShell 7, WSL, Git Bash, or a custom executable)
- Search with real-time highlighting
- WebGL or Canvas renderer, configurable scrollback (10,000 lines by default)
- Web link detection and clear-history action
- Fully customizable keyboard shortcuts with conflict detection, including warnings when a binding would shadow common shell keys such as `Ctrl+A` or `Ctrl+R`
- On macOS, Option can act as the Meta key (Alt shortcuts in the shell, Emacs, and tmux)
- Optional session logging: raw and/or plain-text logs, file name templates, size-based rotation, and gzip compression of closed files

### Broadcast Input

Run the same thing on several servers at once (`Cmd/Ctrl+Shift+B`). The screenshot at the top shows live mode typing into three split panes:

- **Command mode**: type a command once and send it to the selected terminals, with a confirmation for multi-line input
- **Live mode**: keystrokes and pastes from a primary terminal are mirrored to every target in real time
- Per-target delivery status, and disconnected targets can be reconnected before sending
- Live mode stops automatically when a password prompt appears, so passwords stay in the source terminal

### Command Library

![command-library](./docs/screenshot/command-library.png)

- Save commands with a name, description, tags, and favorites (`Cmd/Ctrl+Shift+P` to open)
- Scope a command to all connections or to a single saved connection
- Recently executed commands are captured automatically
- Insert a command into the current terminal without executing it, or save the selected terminal text as a new command
- **Command variables**: write a placeholder such as `{{host}}` in a command and a form asks for its value on insert. Text, number, choice, and secret types, with optional defaults and required flags
- Optional "confirm before inserting" for destructive commands, which shows the full command first
- Tag management: create, rename, and delete

### SSH Connections

![saved-connections](./docs/screenshot/saved-connections.png)

- Password, private key (with passphrase), or SSH agent authentication, including hardware-backed keys (YubiKey, FIDO2) loaded into the agent
- **Keyboard-interactive and multi-step logins**: when a server asks for a one-time code, an MFA token, or a second step after a key, the tab prompts for each answer; a saved password answers the server's password prompt on its own. An "Interactive" method stores no credentials at all
- Optional SSH agent forwarding per connection
- Jump-host chains, opened in order like OpenSSH `ProxyJump`
- Import hosts from `~/.ssh/config` with a preview, including `LocalForward` / `RemoteForward` / `DynamicForward` rules as tunnels
- Connection groups with drag-and-drop, search, and bulk delete
- Host key confirmation; review and remove trusted hosts under Settings → Security
- Automatic reconnect with capped backoff and a configurable attempt limit
- Per-connection keepalive interval and missed-reply limit
- Per-tab connection header with pinning
- **Sudo password autofill**: when `sudo` or `doas` asks for a password, a bar outside the terminal offers to fill the saved password with one click or `Cmd/Ctrl+Shift+Enter`. Prompts are recognized in English, Chinese, and other locales, and you can add custom patterns. A dedicated sudo password can be saved per connection, which is useful for key or agent logins. Autofill pauses for the session if the password is rejected.

### Server Monitor

- Compact status bar under each SSH session: CPU, memory, network throughput, disk, load, uptime, server IP, and SSH latency. Choose which metrics to show and in what order.
- Expandable detail panel with Overview, CPU, Memory, Network, and Disk tabs and recent trends
- Metrics are read over the existing SSH connection from `/proc`; nothing is installed on the server. Linux hosts only.
- Refresh interval from 1 to 60 seconds

### Built-in SFTP File Manager

![sftp-browser](./docs/screenshot/sftp-browser.png)

- Browse remote directories, jump to a path by typing it, resize columns, and see owner/group and permissions
- Click a column header to sort by name, modified date, size, kind, permissions, or owner; hide dotfiles when you don't need them
- Filter the current folder by text, glob, or regular expression
- Create folders, rename, and delete; batch delete with preview, and a reviewed remote command for very large folders
- Change permissions (chmod) with read/write/execute checkboxes or an octal mode, for one item or the whole selection
- Upload and download files and folders, drag and drop from the desktop, or paste copied local files with `Ctrl+V` / `Cmd+V`
- **Remote file editor**: open a remote file in a built-in editor with syntax highlighting and save it back
- Copy full remote paths

### Transfer Manager

- Parallel SFTP channels per transfer (1–16, default 4)
- Resume or retry interrupted transfers from where they stopped
- Conflict handling when files already exist: overwrite only if newer, overwrite all, skip, or keep both
- Separate upload and download bandwidth limits that also apply to running transfers
- Progress, speed, and status for every task, with expandable batch/folder groups and a history view
- ZMODEM transfers show up here alongside SFTP tasks

### ZMODEM File Transfer (rz / sz)

When SFTP isn't available (network-equipment consoles, embedded targets, bastion hosts that only expose an interactive shell), run `rz` / `sz` right in the terminal, just like SecureCRT / Xshell. Transfers are detected automatically in both local terminals and SSH sessions, and can also be triggered manually from the context menu.

See [docs/zmodem.md](./docs/zmodem.md) for usage, settings, and implementation details.

### Port Forwarding

![port-forwarding](./docs/screenshot/port-forwarding.png)

- Local, remote, and dynamic (SOCKS5) forwards built on your saved SSH connections, including jump hosts
- Tunnels to the same host share one SSH connection
- Live status, active and total connections, and bytes sent/received
- Automatic reconnect with notifications when a tunnel drops or comes back
- Start tunnels when tTerm opens (skipped when a password would need to be typed)
- Shows the equivalent OpenSSH command, and warns when a tunnel listens on an address reachable from your network
- Copy the listening address or open it in the browser

### Personalization

![theme-editor](./docs/screenshot/theme-editor.png)

- Built-in themes and a custom theme editor with live preview
- Terminal palette preview with 16-color swatches
- Font settings with a system font picker, and cursor style picker
- Interface text scale from 80% to 200%
- Window background blur (macOS, Windows) with adjustable blur and opacity; acrylic or mica material on Windows
- English and Chinese UI with automatic language detection
- Windows, macOS, and Linux desktop support

### Security and Data

- **Encrypted saved passwords**: SSH, jump-host, and sudo passwords are encrypted with AES-256-GCM in tTerm's local database. Choose how the key is unlocked:
  - **System credential store** (Keychain on macOS, Credential Manager on Windows, Secret Service on Linux)
  - **Master password** (Argon2id), with an optional unlock prompt at startup
  - **Don't save passwords**: keep them for the current session only
- Optional recovery password in case the system credential store loses the key
- View or delete saved passwords from settings
- **Backup and migration**: export exactly the data you choose (settings, connections and tunnels, workspace, known hosts, commands, themes, logs). Passwords are only included in password-protected backups (Argon2id + AES-256-GCM). Imports are previewed first, can merge or replace, and create a recovery snapshot automatically.
- Scheduled local backups (daily or weekly) with a retention limit. These never contain passwords.
- **WebDAV remote backup**: upload encrypted backups to a WebDAV service (Nextcloud, Synology, AList, Jianguoyun, ...) and restore them on any device
- **Multi-device sync**: devices that share a WebDAV folder merge each other's changes to connections, passwords, commands, known hosts, settings, and themes automatically. The sync file is encrypted before upload, and you choose what is synced.
- Data from older versions (JSON config files and the old password vault) is migrated automatically
- Data stays on your machine unless you set up WebDAV yourself; tTerm has no cloud service of its own

### Updates

- Built-in updater with **Stable** and **Beta Dev** channels
- Optional background download, configurable check frequency, and release notes in the app
- Warns before a restart would interrupt active sessions

## Getting Started

### Download

Pre-built binaries are available on the [Releases](https://github.com/330079598/tTerm/releases) page for:

- **macOS** — Apple Silicon (aarch64) and Intel (x86_64) `.dmg` and `.app` bundles
- **Windows** — `.exe` NSIS installer
- **Linux** — `.AppImage`, `.deb`, and `.rpm` packages

### Prerequisites

- Node.js 18+
- pnpm
- Rust (latest stable)
- Tauri 2 system dependencies

Platform-specific requirements:

- **Windows**: Microsoft C++ Build Tools
- **macOS**: Xcode Command Line Tools
- **Linux**: Tauri dependencies such as WebKitGTK, OpenSSL, and AppIndicator packages

### Install Dependencies

```bash
pnpm install
```

### Run in Development Mode

```bash
pnpm tauri dev
```

On Linux, if the Wayland backend exits with a GDK protocol error or WebKitGTK
reports GBM buffer errors, use the XWayland-compatible development command:

```bash
pnpm tauri:dev:linux
```

This command runs the app with `GDK_BACKEND=x11` and disables WebKitGTK's
DMABUF renderer. An XWayland installation is required.

### Build the Desktop App

```bash
pnpm tauri build
```

Tauri writes platform-specific bundles under `src-tauri/target`.

### CI/CD

GitHub Actions workflows are included for automated builds:

- **release-main.yml** — Triggered on push to `main` or `v*` tags. Builds for macOS (aarch64 + x86_64), Ubuntu 24.04, and Windows. Creates draft GitHub releases.
- **release-dev-beta.yml** — Triggered on push to `dev-beta`. Builds beta pre-releases for the Beta Dev update channel.
- **sync-release-notes.yml** — Manually triggered. Syncs release notes into the updater manifest for a channel.

## Common Scripts

```bash
# Start the frontend dev server
pnpm dev

# Build frontend assets
pnpm build

# Preview the frontend build
pnpm preview

# Start Tauri development mode
pnpm tauri dev

# Start Tauri on Linux through XWayland (Wayland/GBM compatibility mode)
pnpm tauri:dev:linux

# Build the desktop application
pnpm tauri build

# Run frontend tests (Vitest)
pnpm test

# Run ESLint
pnpm lint

# Fix ESLint issues automatically
pnpm lint:fix

# Format source files
pnpm format

# Check formatting
pnpm format:check
```

## Tech Stack

### Frontend

- React 19
- TypeScript
- Vite
- TanStack Router
- xterm.js (WebGL and Canvas renderers)
- dockview (split-pane workspace)
- CodeMirror 6 (remote file editor)
- dnd-kit
- i18next / react-i18next
- Radix UI
- lucide-react
- Tailwind CSS 4

### Desktop and Backend

- Tauri 2 (updater, dialog, clipboard, OS plugins)
- Rust 2021
- portable-pty
- russh
- russh-sftp
- Tokio
- SQLite (rusqlite)
- keyring
- aes-gcm / argon2 / zeroize

## Project Structure

```text
.
├── src/                      # React frontend application
│   ├── components/           # Terminal, SFTP, tunnels, broadcast, settings, theme, and UI components
│   ├── contexts/             # Global config, theme, keymap, and transfer state
│   ├── hooks/                # Tabs, connections, session restore, and feature hooks
│   ├── i18n/                 # English and Chinese locale resources
│   ├── lib/                  # Keymap, theme, startup, prompt detection, and utility helpers
│   ├── routes/               # TanStack Router pages
│   └── types/                # Frontend type definitions
├── src-tauri/                # Tauri / Rust backend
│   ├── src/backup/           # Backup export/import, scheduled backups, and WebDAV remote backup
│   ├── src/command_library/  # Saved commands and tags
│   ├── src/config/           # App configuration and paths
│   ├── src/core/             # PTY, commands, and app state
│   ├── src/db/               # SQLite schema, migrations, and legacy JSON import
│   ├── src/fonts/            # System font support
│   ├── src/monitor/          # Server monitor metrics over SSH
│   ├── src/profiles/         # Connection profiles and SSH config import
│   ├── src/session/          # Session persistence
│   ├── src/session_log.rs    # Terminal session logging
│   ├── src/sftp/             # SFTP connections, file operations, and transfers
│   ├── src/ssh/              # SSH client, agent, jump hosts, host keys, and encrypted secret store
│   ├── src/sync/             # Multi-device sync over WebDAV (three-way merge)
│   ├── src/terminal/         # Terminal types and I/O
│   ├── src/tunnel/           # Port forwarding, SOCKS5, and the shared-connection hub
│   ├── src/updater.rs        # In-app updates
│   ├── src/window_blur.rs    # Window background blur (macOS / Windows)
│   └── src/zmodem/           # ZMODEM (rz/sz) protocol engine, trigger detection, send/receive
├── docs/                     # Documentation and screenshots
├── public/                   # Static assets
└── dist/                     # Frontend build output
```

## Use Cases

- Maintain several servers and switch between SSH sessions quickly
- Run the same command on a group of servers at once
- Upload and download files between local and remote machines often, including large or interrupted transfers
- Reach private databases and web services through SSH tunnels
- Work with terminals and remote files in a single desktop window
- Move files with `rz`/`sz` on network gear, embedded devices, or shell-only bastion hosts
- Save frequently used connection profiles while keeping credentials local and encrypted
- Use a lighter, modern, themeable alternative to heavier terminal clients

## Development Notes

tTerm's frontend calls Rust backend commands through Tauri `invoke`. Terminal support is powered by `portable-pty`, and SSH and SFTP are powered by `russh` and `russh-sftp`. Settings stay in `config.json`; connections, tunnels, known hosts, commands, and encrypted secrets live in a local SQLite database (`tterm.db`). Saved passwords use envelope encryption: each secret is encrypted with a data key, and only that key is protected by the system credential store or a master password.

When developing, pay special attention to:

- Whether frontend interactions work well for multi-tab, split-pane, and multi-task workflows
- Whether Rust commands return clear errors that can be shown in the UI
- Whether file transfers expose complete progress, cancellation, and failure states
- Whether password, key, and host-fingerprint flows remain local, safe, and explicit
- Whether new data is covered by a database migration (`src-tauri/src/db/migrations.rs`) and by backup/import

For how ZMODEM is implemented and how to run its integration tests against real `lrzsz`, see [docs/zmodem.md](./docs/zmodem.md#implementation).

## Contributing

Issues and pull requests are welcome. Before submitting changes, run:

```bash
pnpm lint
pnpm format:check
pnpm test
pnpm build
```

If your changes touch the Rust/Tauri backend, also run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

## License

MIT
