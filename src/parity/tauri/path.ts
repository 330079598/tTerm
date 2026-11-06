// Replaces `@tauri-apps/api/path` when Vite runs with `--mode parity`.
// A fake home directory would make a screen look plausible while proving
// nothing about the native path, so the call fails instead.

export function homeDir(): never {
  throw new Error("Parity mock: @tauri-apps/api/path is not available in the reference capture")
}
