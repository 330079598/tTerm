import { ScrollText, Settings, Waypoints } from "lucide-react"

import { TAB_ICON_GLYPHS } from "@/components/TabIconPicker/glyphs"
import { useConfig } from "@/contexts/ConfigContext"
import { distroIconSrc } from "@/lib/distro"
import {
  isDistroTabIcon,
  resolveTabIcon,
  tabIconDistro,
  type ResolvedTabIcon,
} from "@/lib/tabIcons"
import type { Tab } from "@/types/tab"

const PAGE_ICONS = { settings: Settings, tunnels: Waypoints, logs: ScrollText } as const

/**
 * An icon a terminal tab can show: a glyph drawn in its color, or the text
 * color for none, or a distribution's logo in its own colors.
 */
export function TabIconGlyph({
  icon,
  className,
  size = 13,
}: {
  icon: ResolvedTabIcon
  className?: string
  size?: number
}) {
  if (isDistroTabIcon(icon.icon)) {
    return (
      <img
        className={className}
        src={distroIconSrc(tabIconDistro(icon.icon))}
        width={size}
        height={size}
        alt=""
        aria-hidden="true"
        draggable={false}
      />
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
 * shells apart; for an SSH tab its own or its host's distribution's.
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
  return icon && <TabIconGlyph icon={icon} className={className} />
}
