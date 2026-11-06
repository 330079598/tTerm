// Replaces `@tauri-apps/api/app` when Vite runs with `--mode parity`.
// The reference capture has no native app, so calling this is a test failure
// rather than a made-up version string.

export function getVersion(): never {
  throw new Error("Parity mock: @tauri-apps/api/app is not available in the reference capture")
}
