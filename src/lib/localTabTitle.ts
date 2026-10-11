import { invoke } from "@tauri-apps/api/core"
import { homeDir } from "@tauri-apps/api/path"

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

/** A running command longer than this shows as its program alone. */
const MAX_COMMAND_TITLE_LENGTH = 20

// The user's home directory, which titles show as `~`. Read before the first
// local tab is named, so naming after a directory can stay synchronous.
let home: string | null = null
let homeLoaded: Promise<void> | null = null

function loadHome(): Promise<void> {
  homeLoaded ??= homeDir().then(
    (dir) => {
      home = dir
    },
    () => undefined
  )
  return homeLoaded
}

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

// Git Bash reports `/c/Users`, which is `C:/Users` on Windows.
function comparablePath(path: string): string {
  const slashed = path.replace(/\\/g, "/").replace(/^\/([A-Za-z])(\/|$)/, "$1:/")
  const trimmed = slashed.replace(/\/+$/, "")
  return /^[A-Za-z]:/.test(trimmed) ? trimmed.toLowerCase() : trimmed
}

/**
 * The title for a shell in `cwd`: the directory's own name, `~` for the home
 * directory, the root itself for a root. Null for an empty path.
 */
export function directoryTitle(cwd: string, homeDirectory: string | null): string | null {
  const trimmed = cwd.trim()
  if (!trimmed) return null
  const path = trimmed.replace(/[\\/]+$/, "")
  if (homeDirectory && comparablePath(path) === comparablePath(homeDirectory)) return "~"
  if (!path) return trimmed.startsWith("\\") ? "\\" : "/"
  if (/^[A-Za-z]:$/.test(path)) return `${path}\\`
  return path.slice(Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\")) + 1)
}

/**
 * The title for a running command: the whole command when it is short
 * (`npm run dev`), else its program without path or extension (`cargo`).
 * Environment assignments, `sudo` and PowerShell's `&` in front are skipped.
 */
export function commandTitle(command: string): string | null {
  let rest = command.trim()
  const runner = /^(?:sudo|&|\w+=\S*)\s+/
  while (runner.test(rest)) rest = rest.replace(runner, "")
  const quoted = /^(["'])(.*?)\1/.exec(rest)
  const first = quoted ? quoted[2] : rest.split(/\s/, 1)[0]
  const args = rest
    .slice(quoted ? quoted[0].length : first.length)
    .trim()
    .replace(/\s+/g, " ")
  const program = first.replace(/^.*[\\/]/, "").replace(/\.(exe|cmd|bat|com|ps1)$/i, "")
  if (!program) return null
  const whole = args ? `${program} ${args}` : program
  return whole.length <= MAX_COMMAND_TITLE_LENGTH ? whole : program
}

/** A local tab's title while its shell sits in `cwd`, once the home directory is known. */
function cwdTitle(cwd: string | undefined): string | null {
  return cwd ? directoryTitle(cwd, home) : null
}

/**
 * The tab with the directory its local shell reported. A tab named
 * automatically takes the directory's name as its title; one the user named
 * keeps it.
 */
export function withLocalCwd(tab: Tab, cwd: string): Tab {
  const title = (tab.autoTitle && cwdTitle(cwd)) || tab.title
  if (tab.connection?.cwd === cwd && title === tab.title) return tab
  return { ...tab, title, connection: { ...(tab.connection ?? { type: "terminal" }), cwd } }
}

/** A tab's tooltip: its title with the host it reaches, or a local shell's directory. */
export function tabTooltip(tab: Tab): string {
  const detail = tab.type === "terminal" ? tab.connection?.cwd : tab.connection?.host
  return detail && detail !== tab.title ? `${tab.title} (${detail})` : tab.title
}

/** A local tab saved with one of the old default names, to be named automatically instead. */
export function hasLegacyLocalTitle(tab: Pick<Tab, "type" | "title">): boolean {
  return tab.type === "terminal" && LEGACY_LOCAL_TITLES.has(tab.title.replace(/-\d+$/, ""))
}

/**
 * A local tab opened without a name is named automatically: after its
 * directory, or after its shell until the shell reports one.
 */
export async function nameLocalTab<T extends Omit<Tab, "id" | "isActive">>(tab: T): Promise<T> {
  if (tab.type !== "terminal" || tab.title) return tab
  await loadHome()
  const title = cwdTitle(tab.connection?.cwd) ?? (await localShellName(tab.connection))
  return { ...tab, title, autoTitle: true, duplicateBaseTitle: undefined }
}

// Before titles followed the directory, a local tab was named after its shell
// and numbered (`pwsh-2`), with the shell's name kept as its base title. A
// rename cleared the base, so a tab still carrying it was never named by hand.
async function hasGeneratedTitle(tab: Tab): Promise<boolean> {
  if (tab.type !== "terminal" || tab.autoTitle) return false
  if (hasLegacyLocalTitle(tab)) return true
  const base = tab.duplicateBaseTitle
  const numbered = (suffix: string) => /^-\d+$/.test(suffix)
  if (!base || !tab.title.startsWith(base)) return false
  if (tab.title !== base && !numbered(tab.title.slice(base.length))) return false
  return base === (await localShellName(tab.connection))
}

/** Saved local tabs still showing a generated name, named automatically from now on. */
export function renameLegacyLocalTabs(tabs: Tab[]): Promise<Tab[]> {
  return Promise.all(
    tabs.map(async (tab) =>
      (await hasGeneratedTitle(tab)) ? nameLocalTab({ ...tab, title: "" }) : tab
    )
  )
}
