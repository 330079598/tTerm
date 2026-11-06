// The Tauri modules the frontend imports. `modules.test.ts` checks this list
// against `vite.config.ts`, so a new import cannot slip into a parity build and
// hit a real plugin with no browser implementation.

export interface ParityTauriModule {
  specifier: string
  file: string
}

export const PARITY_TAURI_MODULES: readonly ParityTauriModule[] = [
  { specifier: "@tauri-apps/api/core", file: "src/parity/tauri/core.ts" },
  { specifier: "@tauri-apps/api/event", file: "src/parity/tauri/event.ts" },
  { specifier: "@tauri-apps/api/window", file: "src/parity/tauri/window.ts" },
  { specifier: "@tauri-apps/api/app", file: "src/parity/tauri/app.ts" },
  { specifier: "@tauri-apps/api/path", file: "src/parity/tauri/path.ts" },
  { specifier: "@tauri-apps/api/webviewWindow", file: "src/parity/tauri/webviewWindow.ts" },
  { specifier: "@tauri-apps/plugin-dialog", file: "src/parity/tauri/dialog.ts" },
  { specifier: "@tauri-apps/plugin-os", file: "src/parity/tauri/os.ts" },
  { specifier: "@tauri-apps/plugin-opener", file: "src/parity/tauri/opener.ts" },
  { specifier: "@tauri-apps/plugin-process", file: "src/parity/tauri/process.ts" },
] as const

/** Builds the `mode === "parity"` Vite aliases from the module list. */
export function parityTauriAliases(
  resolvePath: (file: string) => string
): Array<[specifier: string, replacement: string]> {
  return PARITY_TAURI_MODULES.map(({ specifier, file }) => [specifier, resolvePath(file)])
}
