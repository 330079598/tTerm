import { ChevronRight, ScrollText, Settings, Waypoints } from "lucide-react"

import { TAB_ICON_GLYPHS } from "@/components/TabIconPicker/glyphs"
import { useConfig } from "@/contexts/ConfigContext"
import { distroIconSrc } from "@/lib/distro"
import {
  hasLocalBadge,
  isDistroTabIcon,
  resolveTabIcon,
  tabIconDistro,
  type ResolvedTabIcon,
} from "@/lib/tabIcons"
import { cn } from "@/lib/utils"
import type { Tab } from "@/types/tab"

const PAGE_ICONS = { settings: Settings, tunnels: Waypoints, logs: ScrollText } as const

/**
 * An icon a terminal tab can show: a glyph drawn in its color, or the text
 * color for none, or a distribution's logo in its own colors. `localBadge`
 * marks a logo with a prompt in its corner, for a local shell.
 */
export function TabIconGlyph({
  icon,
  className,
  size = 13,
  localBadge = false,
}: {
  icon: ResolvedTabIcon
  className?: string
  size?: number
  localBadge?: boolean
}) {
  if (isDistroTabIcon(icon.icon)) {
    const logo = (
      <img
        className={localBadge ? undefined : className}
        src={distroIconSrc(tabIconDistro(icon.icon))}
        width={size}
        height={size}
        alt=""
        aria-hidden="true"
        draggable={false}
      />
    )
    if (!localBadge) return logo
    // Inverted, so it reads on any logo; the attention marks beside the
    // icon are dots, so this one is a square.
    return (
      <span className={cn("relative inline-flex shrink-0", className)} aria-hidden="true">
        {logo}
        <span className="bg-foreground text-background ring-background absolute -right-[3px] -bottom-[2px] flex h-2 w-2 items-center justify-center rounded-[2px] ring-1">
          <ChevronRight size={7} strokeWidth={4} />
        </span>
      </span>
    )
  }
  const Glyph = TAB_ICON_GLYPHS[icon.icon]
  return (
    <Glyph
      className={className}
      size={size}
      style={icon.color ? { color: icon.color } : undefined}
      aria-hidden="true"
    />
  )
}

/**
 * The icon in front of a tab's title: an app page's; for a local terminal
 * its own or its shell's, so tabs named after their directory still tell
 * shells apart, with a WSL tab's logo badged as local; for an SSH tab its own
 * or its host's distribution's.
 */
export function TabIcon({
  tab,
  className,
}: {
  tab: Pick<Tab, "type" | "shellName" | "distroId" | "iconStyle">
  className?: string
}) {
  const { config } = useConfig()
  if (tab.type in PAGE_ICONS) {
    const Icon = PAGE_ICONS[tab.type as keyof typeof PAGE_ICONS]
    return <Icon className={className} size={13} aria-hidden="true" />
  }
  const icon = resolveTabIcon(tab, config.tab_icons)
  return (
    icon && <TabIconGlyph icon={icon} className={className} localBadge={hasLocalBadge(tab, icon)} />
  )
}
