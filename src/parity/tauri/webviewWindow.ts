// Replaces `@tauri-apps/api/webviewWindow` when Vite runs with `--mode parity`.
// The webview background is set natively; there is no browser equivalent that
// would make the screenshot representative.

export function getCurrentWebviewWindow(): never {
  throw new Error(
    "Parity mock: @tauri-apps/api/webviewWindow is not available in the reference capture"
  )
}
