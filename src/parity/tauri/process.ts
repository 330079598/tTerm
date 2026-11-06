// Replaces `@tauri-apps/plugin-process` when Vite runs with `--mode parity`.
// Relaunching or exiting belongs to the native process; the capture must fail
// loudly if a screen reaches for it.

function unavailable(): never {
  throw new Error(
    "Parity mock: @tauri-apps/plugin-process is not available in the reference capture"
  )
}

export const relaunch = unavailable
export const exit = unavailable
