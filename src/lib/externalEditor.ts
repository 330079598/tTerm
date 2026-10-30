import { open as openFileDialog } from "@tauri-apps/plugin-dialog"

import { getDetectedPlatform } from "@/lib/uiPlatform"

/**
 * The name to show for the external editor: the program's file name without
 * its folder or its application suffix, as in "Visual Studio Code" for
 * `/Applications/Visual Studio Code.app`.
 */
export function editorDisplayName(program: string): string {
  const name = program
    .trim()
    .replace(/[\\/]+$/, "")
    .split(/[\\/]/)
    .pop()
  return (name ?? "").replace(/\.(app|exe|cmd|bat|com)$/i, "") || program.trim()
}

/** Asks for the program to edit files in; null when the user cancels. */
export async function pickEditorProgram(title: string): Promise<string | null> {
  const platform = getDetectedPlatform()
  const selected = await openFileDialog({
    title,
    multiple: false,
    directory: false,
    defaultPath: platform === "macos" ? "/Applications" : undefined,
    filters:
      platform === "windows"
        ? [{ name: "Programs", extensions: ["exe", "cmd", "bat", "com"] }]
        : undefined,
  }).catch(() => null)
  return typeof selected === "string" ? selected : null
}
