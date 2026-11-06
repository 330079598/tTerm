// Replaces `@tauri-apps/plugin-dialog` when Vite runs with `--mode parity`.
// Native dialogs cannot open in the capture, and returning a fake path would
// silently change what the screen shows.

function unavailable(): never {
  throw new Error(
    "Parity mock: @tauri-apps/plugin-dialog is not available in the reference capture"
  )
}

export const open = unavailable
export const save = unavailable
export const message = unavailable
export const ask = unavailable
export const confirm = unavailable
