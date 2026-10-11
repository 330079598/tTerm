import {
  Fish,
  GitBranch,
  ScrollText,
  Settings,
  SquareChevronRight,
  SquareTerminal,
  Terminal,
  Waypoints,
  type LucideIcon,
} from "lucide-react"

import type { Tab } from "@/types/tab"

const PAGE_ICONS = { settings: Settings, tunnels: Waypoints, logs: ScrollText } as const

interface IconChoice {
  icon: LucideIcon
  /** A hint of the shell's own color, readable on light and dark tabs alike. */
  color?: string
}

// Keyed by the lowercased names local_terminal_shell_name gives.
const SHELL_ICONS: Record<string, IconChoice> = {
  pwsh: { icon: SquareChevronRight, color: "#3b9ae1" },
  powershell: { icon: SquareChevronRight, color: "#3b9ae1" },
  cmd: { icon: SquareTerminal },
  "git bash": { icon: GitBranch, color: "#f05133" },
  wsl: { icon: Terminal, color: "#e9853a" },
  bash: { icon: Terminal, color: "#4caf7a" },
  zsh: { icon: Terminal, color: "#4caf7a" },
  sh: { icon: Terminal, color: "#4caf7a" },
  fish: { icon: Fish, color: "#4caf7a" },
}

function tabIcon(tab: Pick<Tab, "type" | "shellName">): IconChoice | null {
  if (tab.type in PAGE_ICONS) return { icon: PAGE_ICONS[tab.type as keyof typeof PAGE_ICONS] }
  if (tab.type !== "terminal") return null
  return SHELL_ICONS[tab.shellName?.toLowerCase() ?? ""] ?? { icon: Terminal }
}

/**
 * The icon in front of a tab's title: an app page's, or for a local terminal
 * its shell's, so tabs named after their directory still tell shells apart.
 */
export function TabIcon({
  tab,
  className,
}: {
  tab: Pick<Tab, "type" | "shellName">
  className?: string
}) {
  const choice = tabIcon(tab)
  if (!choice) return null
  const { icon: Icon, color } = choice
  return (
    <Icon
      className={className}
      size={13}
      style={color ? { color } : undefined}
      aria-hidden="true"
    />
  )
}
