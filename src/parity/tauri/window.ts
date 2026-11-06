// Replaces `@tauri-apps/api/window` when Vite runs with `--mode parity`.
// Window commands and drag/drop events only exist in the native app; a screen
// that needs them cannot be captured faithfully, so obtaining the window fails.

export function getCurrentWindow(): never {
  throw new Error("Parity mock: @tauri-apps/api/window is not available in the reference capture")
}
