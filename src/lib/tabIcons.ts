import { isDistroId, type DistroId } from "@/lib/distro"
import { isSessionTab, type Tab } from "@/types/tab"

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
  "usb",
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

/** The AI agents with a logo of their own, by the name their reports give. */
export const AGENT_LOGO_IDS = ["claude", "codex", "opencode", "pi"] as const

export type AgentLogoId = (typeof AGENT_LOGO_IDS)[number]

/** An AI agent's logo, which a tab shows while the agent runs; never saved as a choice. */
export type AgentTabIcon = `agent:${AgentLogoId}`

// Claude Code's logo keeps its own color; the others are black and white, so
// they take the text color to read on light and dark tabs alike.
export const AGENT_LOGO_COLORS: Record<AgentLogoId, string> = {
  claude: "#d97757",
  codex: "",
  opencode: "",
  pi: "",
}

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
  icon: TabIconChoice | AgentTabIcon
  color: string
}

export interface TabIconConfig {
  /** Show an icon in front of each terminal tab's title. */
  enabled: boolean
  /** Show the logo of an AI agent while it runs in a tab, in place of the shell's or host's icon. */
  agents: boolean
  /** Choices by shell, keyed by {@link shellKey}. */
  shells: Record<string, TabIconStyle>
}

export const DEFAULT_TAB_ICON_CONFIG: TabIconConfig = { enabled: true, agents: true, shells: {} }

const PLAIN: ResolvedTabIcon = { icon: "terminal", color: "" }
const POWERSHELL: ResolvedTabIcon = { icon: "square-chevron-right", color: "#3b9ae1" }
const UNIX: ResolvedTabIcon = { icon: "terminal", color: "#4caf7a" }
const SERVER: ResolvedTabIcon = { icon: "server", color: "" }
const SERIAL: ResolvedTabIcon = { icon: "usb", color: "" }

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

export function isDistroTabIcon(icon: ResolvedTabIcon["icon"]): icon is DistroTabIcon {
  return icon.startsWith("distro:")
}

export function isAgentTabIcon(icon: ResolvedTabIcon["icon"]): icon is AgentTabIcon {
  return icon.startsWith("agent:")
}

/** The agent whose logo `icon` is. */
export function tabIconAgent(icon: AgentTabIcon): AgentLogoId {
  return icon.slice("agent:".length) as AgentLogoId
}

/** An agent's logo as a tab shows it, or null for an agent without one. */
export function agentTabIcon(agent: string): ResolvedTabIcon | null {
  if (!(AGENT_LOGO_IDS as readonly string[]).includes(agent)) return null
  const id = agent as AgentLogoId
  return { icon: `agent:${id}`, color: AGENT_LOGO_COLORS[id] }
}

/** The distribution whose logo `icon` is. */
export function tabIconDistro(icon: DistroTabIcon): DistroId {
  return icon.slice("distro:".length) as DistroId
}

/** What an SSH tab shows without a choice of its own: its host's distribution, once known. */
export function sshTabIcon(distroId: DistroId | undefined): ResolvedTabIcon {
  return distroId ? distroTabIcon(distroId) : SERVER
}

function distroTabIcon(distroId: DistroId): ResolvedTabIcon {
  return { icon: `distro:${distroId}`, color: "" }
}

/**
 * What a local tab shows without a choice of its own. A WSL tab shows the
 * distribution its launcher reported in place of the built-in look, under
 * any choice the user made for WSL.
 */
function localTabIcon(
  config: TabIconConfig,
  shellName: string | undefined,
  distroId: DistroId | undefined
): ResolvedTabIcon {
  const key = shellKey(shellName)
  if (key !== "wsl" || !distroId) return shellTabIcon(config, shellName)
  return applyTabIconStyle(distroTabIcon(distroId), config.shells[key])
}

/** A local tab with the distribution its WSL shell reported; the same tab when nothing changes. */
export function withLocalDistro(tab: Tab, distroId: DistroId): Tab {
  if (tab.type !== "terminal" || tab.distroId === distroId) return tab
  return { ...tab, distroId }
}

/**
 * Whether a tab's icon carries the local badge: a WSL tab showing a
 * distribution's logo, which would otherwise look like an SSH host's.
 */
export function hasLocalBadge(
  tab: Pick<Tab, "type" | "shellName">,
  icon: ResolvedTabIcon
): boolean {
  return tab.type === "terminal" && shellKey(tab.shellName) === "wsl" && isDistroTabIcon(icon.icon)
}

/** Whether a tab is a terminal, local or SSH, and so shows an icon. */
export function hasTabIcon(tab: Pick<Tab, "type">): boolean {
  return isSessionTab(tab)
}

/** What a terminal tab shows without a choice of its own. */
export function inheritedTabIcon(
  tab: Pick<Tab, "type" | "shellName" | "distroId">,
  config: TabIconConfig
): ResolvedTabIcon {
  if (tab.type === "ssh") return sshTabIcon(tab.distroId)
  if (tab.type === "serial") return SERIAL
  return localTabIcon(config, tab.shellName, tab.distroId)
}

/**
 * The icon a terminal tab shows, or null for other tabs and while icons are
 * off. The logo of the AI agent running in it, if any, stands in for the icon
 * it inherits; a choice made for the tab itself still wins.
 */
export function resolveTabIcon(
  tab: Pick<Tab, "type" | "shellName" | "distroId" | "iconStyle">,
  config: TabIconConfig,
  agent?: string
): ResolvedTabIcon | null {
  if (!hasTabIcon(tab) || !config.enabled) return null
  const agentIcon = agent && config.agents ? agentTabIcon(agent) : null
  return applyTabIconStyle(agentIcon ?? inheritedTabIcon(tab, config), tab.iconStyle)
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
  const { enabled, agents, shells } = value as Record<string, unknown>
  const normalized: Record<string, TabIconStyle> = {}
  if (shells && typeof shells === "object") {
    for (const [key, style] of Object.entries(shells)) {
      const valid = normalizeTabIconStyle(style)
      if (valid && shellKey(key)) normalized[shellKey(key)] = valid
    }
  }
  return { enabled: enabled !== false, agents: agents !== false, shells: normalized }
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
