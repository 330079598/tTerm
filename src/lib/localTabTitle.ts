import { invoke } from "@tauri-apps/api/core"
import { homeDir } from "@tauri-apps/api/path"

import { isDistroId } from "@/lib/distro"
import { normalizeTabIconStyle } from "@/lib/tabIcons"
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

/** The name of the shell a local tab starts (`cmd`, `pwsh`, `zsh`), or null when unknown. */
export async function detectLocalShellName(
  connection: Pick<NonNullable<Tab["connection"]>, "terminalShell" | "terminalShellCustomPath">
): Promise<string | null> {
  try {
    const name = await invoke<string | null>("local_terminal_shell_name", {
      shell: connection.terminalShell ?? null,
      customPath: connection.terminalShellCustomPath ?? null,
    })
    return name || null
  } catch {
    return null
  }
}

/** The name of the shell a local tab starts, for its title. */
export async function localShellName(connection: Tab["connection"]): Promise<string> {
  return (await detectLocalShellName(connection ?? {})) ?? FALLBACK_TITLE
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

/**
 * What a local tab named automatically shows while its shell runs a command:
 * the command's title, followed by the tab's own title while another tab runs
 * a command of the same title, as several agents at once do (`claude · web`,
 * `claude · api`). `commands` holds the command each tab's shell runs.
 */
export function runningCommandTitle(
  tab: Tab,
  commands: ReadonlyMap<string, string>
): string | null {
  const command = tab.autoTitle ? commands.get(tab.id) : undefined
  const title = command && commandTitle(command)
  if (!title) return null
  for (const [tabId, other] of commands) {
    if (tabId !== tab.id && commandTitle(other) === title) return `${title} · ${tab.title}`
  }
  return title
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

// A path's directories, without empty parts or a trailing separator.
function pathSegments(path: string): string[] {
  return path.split(/[\\/]+/).filter(Boolean)
}

/**
 * Titles for local tabs named automatically, set apart where shells sit in
 * different directories of the same name: `web/src` and `api/src`, with as
 * many parent directories as it takes. Tabs in the very same directory keep
 * the same title. Returns the same array when no title changes.
 */
export function distinctLocalTitles(tabs: Tab[], homeDirectory: string | null): Tab[] {
  const groups = new Map<string, { tab: Tab; cwd: string }[]>()
  for (const tab of tabs) {
    const cwd = tab.connection?.cwd
    const base = tab.autoTitle && cwd ? directoryTitle(cwd, homeDirectory) : null
    if (cwd && base) groups.set(base, [...(groups.get(base) ?? []), { tab, cwd }])
  }

  const titles = new Map<string, string>()
  for (const [base, members] of groups) {
    const paths = [...new Set(members.map(({ cwd }) => comparablePath(cwd)))]
    for (const { tab, cwd } of members) {
      const own = comparablePath(cwd)
      const others = paths.filter((path) => path !== own).map(pathSegments)
      titles.set(tab.id, others.length ? distinctSuffix(cwd, others) : base)
    }
  }

  let changed = false
  const next = tabs.map((tab) => {
    const title = titles.get(tab.id)
    if (title === undefined || title === tab.title) return tab
    changed = true
    return { ...tab, title }
  })
  return changed ? next : tabs
}

// The fewest trailing directories of `cwd` that no other path ends with.
function distinctSuffix(cwd: string, others: string[][]): string {
  const segments = pathSegments(cwd)
  const comparable = pathSegments(comparablePath(cwd))
  const separator = cwd.includes("\\") ? "\\" : "/"
  for (let count = 2; count <= segments.length; count++) {
    const suffix = comparable.slice(-count).join("/")
    if (others.every((other) => other.slice(-count).join("/") !== suffix)) {
      return segments.slice(-count).join(separator)
    }
  }
  return cwd.trim()
}

/** {@link distinctLocalTitles} with the user's home directory, for the tab list. */
export function withDistinctLocalTitles(tabs: Tab[]): Tab[] {
  return distinctLocalTitles(tabs, home)
}

/** A tab's tooltip: its title with the host it reaches, or a local shell's directory and name. */
export function tabTooltip(tab: Tab): string {
  const details =
    tab.type === "terminal" ? [tab.connection?.cwd, tab.shellName] : [tab.connection?.host]
  const detail = details.filter((part) => part && part !== tab.title).join(" · ")
  return detail ? `${tab.title} (${detail})` : tab.title
}

/** A local tab saved with one of the old default names, to be named automatically instead. */
export function hasLegacyLocalTitle(tab: Pick<Tab, "type" | "title">): boolean {
  return tab.type === "terminal" && LEGACY_LOCAL_TITLES.has(tab.title.replace(/-\d+$/, ""))
}

/**
 * A local tab with its shell's name. One opened without a name is named
 * automatically: after its directory, or after its shell until the shell
 * reports one.
 */
export async function nameLocalTab<T extends Omit<Tab, "id" | "isActive">>(tab: T): Promise<T> {
  if (tab.type !== "terminal") return tab
  const [shellName] = await Promise.all([localShellName(tab.connection), loadHome()])
  if (tab.title) return { ...tab, shellName }
  const title = cwdTitle(tab.connection?.cwd) ?? shellName
  return { ...tab, title, shellName, autoTitle: true, duplicateBaseTitle: undefined }
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

/**
 * Saved local tabs brought up to date: those still showing a generated name
 * are named automatically from now on, each learns its shell's name, and a
 * tab's own icon and its WSL distribution are checked.
 */
export async function refreshSavedLocalTabs(tabs: Tab[]): Promise<Tab[]> {
  await loadHome()
  return Promise.all(
    tabs.map(async (tab) => {
      if (tab.type !== "terminal") return tab
      const saved = {
        ...tab,
        iconStyle: normalizeTabIconStyle(tab.iconStyle),
        distroId: isDistroId(tab.distroId) ? tab.distroId : undefined,
      }
      if (await hasGeneratedTitle(saved)) return nameLocalTab({ ...saved, title: "" })
      return saved.shellName ? saved : { ...saved, shellName: await localShellName(tab.connection) }
    })
  )
}
