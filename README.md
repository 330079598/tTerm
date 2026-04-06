# TTerm - Modern Terminal Emulator

TTerm is a modern terminal emulator inspired by Tabby, built with Tauri, React, and TypeScript. It provides a tabbed interface for managing multiple terminal sessions, SSH connections, and SFTP file transfers.

## ✨ Features

- 🖥️ **Multi-tab Interface** - Manage multiple terminal sessions with drag-and-drop reordering
- 🔐 **SSH Support** - Connect to remote servers with password or key-based authentication
- 📁 **SFTP File Manager** - Browse, upload, and download files with drag-and-drop support
- 🎨 **Custom Themes** - Multiple built-in themes (Default, Light, Ocean, Forest, Sunset)
- 🌍 **Internationalization** - Support for English and Chinese
- 💾 **Session Persistence** - Automatically restore tabs on app restart
- 🔒 **Secure Storage** - System keyring integration for password management

## 🚀 Getting Started

### Prerequisites

- Node.js 18+ and pnpm
- Rust 1.70+
- Platform-specific dependencies:
  - **macOS**: Xcode Command Line Tools
  - **Windows**: Microsoft C++ Build Tools
  - **Linux**: Development packages (see Tauri docs)

### Installation

```bash
# Install dependencies
pnpm install

# Run in development mode
pnpm tauri dev

# Build for production
pnpm tauri build
```

## 📖 Usage

### SFTP Drag & Drop Upload

1. Connect to an SSH server
2. Click the "SFTP" button in the connection header
3. Navigate to your desired directory
4. Drag files from your file manager directly into the SFTP panel
5. Monitor upload progress in the transfer manager

**macOS Note**: If drag & drop doesn't work, make sure to completely restart the app after installation. See [MACOS_DRAG_DROP_FIX.md](./MACOS_DRAG_DROP_FIX.md) for troubleshooting.

See [DRAG_DROP_UPLOAD.md](./DRAG_DROP_UPLOAD.md) for detailed documentation.

### Keyboard Shortcuts

- `Cmd/Ctrl + T` - New tab
- `Cmd/Ctrl + W` - Close tab
- `Cmd/Ctrl + Tab` - Next tab
- `Cmd/Ctrl + Shift + Tab` - Previous tab

## 🛠️ Tech Stack

- **Frontend**: React 18, TypeScript, Vite 6, Tailwind CSS 4
- **Backend**: Tauri 2 (Rust)
- **UI Components**: Custom components with lucide-react icons
- **Routing**: TanStack Router
- **State Management**: React hooks

## 📁 Project Structure

```
/src              - Frontend React application
/src-tauri        - Tauri backend (Rust)
/public           - Static assets
/dist             - Build output
```

See [structure.md](./.kiro/steering/structure.md) for detailed structure documentation.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 📄 License

This project is licensed under the MIT License.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
