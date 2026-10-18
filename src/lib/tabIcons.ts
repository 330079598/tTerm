import { isDistroId, type DistroId } from "@/lib/distro"
import type { Tab } from "@/types/tab"

/** The icons a local tab can show, in the order the picker lays them out, by theme. */
export const TAB_ICON_IDS = [
  // Terminals and code.
  "terminal",
  "square-terminal",
  "square-chevron-right",
  "chevron-right",
  "code",
  "braces",
  "brackets",
  "hash",
  "binary",
  "file-code",
  "bot",
  "sparkles",
  // Developer tools.
  "git-branch",
  "git-merge",
  "git-commit-horizontal",
  "package",
  "layers",
  "puzzle",
  "wrench",
  "hammer",
  // Infrastructure and devices.
  "server",
  "database",
  "cloud",
  "container",
  "box",
  "cpu",
  "hard-drive",
  "network",
  "globe",
  "wifi",
  "monitor",
  "laptop",
  // Status and symbols.
  "zap",
  "flame",
  "rocket",
  "bug",
  "shield",
  "lock",
  "key",
  "star",
  "heart",
  "flag",
  "bookmark",
  "bell",
  "eye",
  "target",
  "crown",
  "gem",
  "anchor",
  "compass",
  // Nature and fun.
  "fish",
  "leaf",
  "tree-pine",
  "sun",
  "moon",
  "snowflake",
  "coffee",
  "cat",
  "dog",
  "bird",
  "rabbit",
  "turtle",
  "ghost",
  "skull",
  "gamepad-2",
  "music",
  "palette",
] as const

export type TabIconId = (typeof TAB_ICON_IDS)[number]

/** A Linux distribution's logo, drawn in its own colors. */
export type DistroTabIcon = `distro:${DistroId}`

/** Anything a tab can show: a glyph, or a distribution's logo. */
export type TabIconChoice = TabIconId | DistroTabIcon

/** Colors offered next to a custom one; each reads on light and dark tabs alike. */
export const TAB_ICON_COLORS = [
  "#3b9ae1",
  "#2bb3a3",
  "#4caf7a",
  "#d4a72c",
  "#e9853a",
  "#f05133",
  "#e5534b",
  "#db61a2",
  "#a371f7",
  "#8b949e",
] as const

/**
 * An icon and color chosen for a shell or a tab. A missing one leaves the
 * choice to what applies below it; an empty color means no tint.
 */
export interface TabIconStyle {
  icon?: TabIconChoice
  color?: string
}

/** An icon as shown: an empty color draws it in the tab's text color. */
export interface ResolvedTabIcon {
  icon: TabIconChoice
  color: string
}

export interface TabIconConfig {
  /** Show an icon in front of each terminal tab's title. */
  enabled: boolean
  /** Choices by shell, keyed by {@link shellKey}. */
  shells: Record<string, TabIconStyle>
}

export const DEFAULT_TAB_ICON_CONFIG: TabIconConfig = { enabled: true, shells: {} }

const PLAIN: ResolvedTabIcon = { icon: "terminal", color: "" }
const POWERSHELL: ResolvedTabIcon = { icon: "square-chevron-right", color: "#3b9ae1" }
const UNIX: ResolvedTabIcon = { icon: "terminal", color: "#4caf7a" }
const SERVER: ResolvedTabIcon = { icon: "server", color: "" }

// How each shell looks until the user picks otherwise, keyed like the
// settings: the lowercased names local_terminal_shell_name gives.
const BUILT_IN: Record<string, ResolvedTabIcon> = {
  pwsh: POWERSHELL,
  powershell: POWERSHELL,
  cmd: { icon: "square-terminal", color: "" },
  "git bash": { icon: "git-branch", color: "#f05133" },
  wsl: { icon: "terminal", color: "#e9853a" },
  bash: UNIX,
  zsh: UNIX,
  sh: UNIX,
  fish: { icon: "fish", color: "#4caf7a" },
}

const SHELL_NAMES: Record<string, string> = {
  pwsh: "PowerShell 7 (pwsh)",
  powershell: "Windows PowerShell",
  cmd: "cmd",
  "git bash": "Git Bash",
  wsl: "WSL",
}

const WINDOWS_SHELLS = ["pwsh", "powershell", "cmd", "git bash", "wsl"]
const UNIX_SHELLS = ["zsh", "bash", "fish", "sh"]

/** The key a shell's choices are kept under: its name, lowercased. */
export function shellKey(shellName: string | undefined): string {
  return shellName?.trim().toLowerCase() ?? ""
}

/** A shell's name for the settings list. */
export function shellDisplayName(key: string): string {
  return SHELL_NAMES[key] ?? key
}

/** How a shell looks before any choice of the user's. */
export function builtInShellIcon(key: string): ResolvedTabIcon {
  return BUILT_IN[key] ?? PLAIN
}

/**
 * The shells the settings list: those of this platform, then the ones found
 * on this machine (its default shell), then any other with a choice saved.
 */
export function shellsForSettings(
  platform: string,
  config: TabIconConfig,
  found: (string | null)[] = []
): string[] {
  const keys = platform === "windows" ? [...WINDOWS_SHELLS] : [...UNIX_SHELLS]
  for (const key of found.map((name) => shellKey(name ?? undefined))) {
    if (key && !keys.includes(key)) keys.push(key)
  }
  const saved = Object.keys(config.shells).filter((key) => !keys.includes(key))
  return [...keys, ...saved.sort()]
}

/** A style laid over another: each choice the top one makes wins. */
export function applyTabIconStyle(
  base: ResolvedTabIcon,
  style: TabIconStyle | undefined
): ResolvedTabIcon {
  return { icon: style?.icon ?? base.icon, color: style?.color ?? base.color }
}

/** What a shell's tabs show without a choice of their own. */
export function shellTabIcon(
  config: TabIconConfig,
  shellName: string | undefined
): ResolvedTabIcon {
  const key = shellKey(shellName)
  return applyTabIconStyle(builtInShellIcon(key), config.shells[key])
}

export function isDistroTabIcon(icon: TabIconChoice): icon is DistroTabIcon {
  return icon.startsWith("distro:")
}

/** The distribution whose logo `icon` is. */
export function tabIconDistro(icon: DistroTabIcon): DistroId {
  return icon.slice("distro:".length) as DistroId
}

/** What an SSH tab shows without a choice of its own: its host's distribution, once known. */
export function sshTabIcon(distroId: DistroId | undefined): ResolvedTabIcon {
  return distroId ? { icon: `distro:${distroId}`, color: "" } : SERVER
}

/** Whether a tab is a terminal, local or SSH, and so shows an icon. */
export function hasTabIcon(tab: Pick<Tab, "type">): boolean {
  return tab.type === "terminal" || tab.type === "ssh"
}

/** What a terminal tab shows without a choice of its own. */
export function inheritedTabIcon(
  tab: Pick<Tab, "type" | "shellName" | "distroId">,
  config: TabIconConfig
): ResolvedTabIcon {
  return tab.type === "ssh" ? sshTabIcon(tab.distroId) : shellTabIcon(config, tab.shellName)
}

/** The icon a terminal tab shows, or null for other tabs and while icons are off. */
export function resolveTabIcon(
  tab: Pick<Tab, "type" | "shellName" | "distroId" | "iconStyle">,
  config: TabIconConfig
): ResolvedTabIcon | null {
  if (!hasTabIcon(tab) || !config.enabled) return null
  return applyTabIconStyle(inheritedTabIcon(tab, config), tab.iconStyle)
}

function isTabIconChoice(value: unknown): value is TabIconChoice {
  if (typeof value === "string" && value.startsWith("distro:")) {
    return isDistroId(value.slice("distro:".length))
  }
  return (TAB_ICON_IDS as readonly unknown[]).includes(value)
}

/** A saved style with unknown icons and malformed colors left out; undefined when nothing is left. */
export function normalizeTabIconStyle(value: unknown): TabIconStyle | undefined {
  if (!value || typeof value !== "object") return undefined
  const { icon, color } = value as Record<string, unknown>
  const style: TabIconStyle = {}
  if (isTabIconChoice(icon)) style.icon = icon
  if (typeof color === "string" && (color === "" || /^#[0-9a-f]{6}$/i.test(color))) {
    style.color = color.toLowerCase()
  }
  return style.icon === undefined && style.color === undefined ? undefined : style
}

/** A saved SSH tab with its own icon and its host's distribution checked. */
export function normalizeSavedSshTab(tab: Tab): Tab {
  if (tab.type !== "ssh") return tab
  return {
    ...tab,
    iconStyle: normalizeTabIconStyle(tab.iconStyle),
    distroId: isDistroId(tab.distroId) ? tab.distroId : undefined,
  }
}

/** The tab icon settings from a saved config, each choice valid. */
export function normalizeTabIconConfig(value: unknown): TabIconConfig {
  if (!value || typeof value !== "object") return { ...DEFAULT_TAB_ICON_CONFIG, shells: {} }
  const { enabled, shells } = value as Record<string, unknown>
  const normalized: Record<string, TabIconStyle> = {}
  if (shells && typeof shells === "object") {
    for (const [key, style] of Object.entries(shells)) {
      const valid = normalizeTabIconStyle(style)
      if (valid && shellKey(key)) normalized[shellKey(key)] = valid
    }
  }
  return { enabled: enabled !== false, shells: normalized }
}

/** The config with one shell's choice replaced; undefined forgets it. */
export function withShellIconStyle(
  config: TabIconConfig,
  key: string,
  style: TabIconStyle | undefined
): TabIconConfig {
  const shells = { ...config.shells }
  const valid = normalizeTabIconStyle(style)
  if (valid) shells[key] = valid
  else delete shells[key]
  return { ...config, shells }
}
