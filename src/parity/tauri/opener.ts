// Replaces `@tauri-apps/plugin-opener` when Vite runs with `--mode parity`.
// Opening a URL or revealing a file needs the native shell; a no-op would hide
// the fact that the screen depends on it.

function unavailable(): never {
  throw new Error(
    "Parity mock: @tauri-apps/plugin-opener is not available in the reference capture"
  )
}

export const openUrl = unavailable
export const openPath = unavailable
export const revealItemInDir = unavailable
