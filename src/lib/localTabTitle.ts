import { invoke } from "@tauri-apps/api/core"

import type { Tab } from "@/types/tab"

// Titles local tabs were opened with before they were named after their shell.
const LEGACY_LOCAL_TITLES = new Set([
  "OS terminal",
  "Terminal",
  "Local terminal",
  "终端",
  "本地终端",
])

const FALLBACK_TITLE = "Terminal"

/** The name of the shell a local tab starts (`cmd`, `pwsh`, `zsh`), for its title. */
export async function localShellName(connection: Tab["connection"]): Promise<string> {
  try {
    const name = await invoke<string | null>("local_terminal_shell_name", {
      shell: connection?.terminalShell ?? null,
      customPath: connection?.terminalShellCustomPath ?? null,
    })
    return name || FALLBACK_TITLE
  } catch {
    return FALLBACK_TITLE
  }
}

/** A local tab saved with one of the old default names, to be named after its shell instead. */
export function hasLegacyLocalTitle(tab: Pick<Tab, "type" | "title">): boolean {
  return tab.type === "terminal" && LEGACY_LOCAL_TITLES.has(tab.title.replace(/-\d+$/, ""))
}

/** A local tab opened without a name gets its shell's, numbered among the open tabs when added. */
export async function nameLocalTab<T extends Omit<Tab, "id" | "isActive">>(tab: T): Promise<T> {
  if (tab.type !== "terminal" || tab.title) return tab
  return { ...tab, title: await localShellName(tab.connection), numberTitle: true }
}

/** Saved local tabs still showing an old default name, renamed after their shell. */
export function renameLegacyLocalTabs(tabs: Tab[]): Promise<Tab[]> {
  return Promise.all(
    tabs.map((tab) => (hasLegacyLocalTitle(tab) ? nameLocalTab({ ...tab, title: "" }) : tab))
  )
}
