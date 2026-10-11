import { ScrollText, Settings, Waypoints } from "lucide-react"

import { TAB_ICON_GLYPHS } from "@/components/TabIconPicker/glyphs"
import { useConfig } from "@/contexts/ConfigContext"
import { resolveTabIcon, type ResolvedTabIcon } from "@/lib/tabIcons"
import type { Tab } from "@/types/tab"

const PAGE_ICONS = { settings: Settings, tunnels: Waypoints, logs: ScrollText } as const

/** An icon a local tab can show, drawn in its color, or the text color for none. */
export function TabIconGlyph({ icon, className }: { icon: ResolvedTabIcon; className?: string }) {
  const Glyph = TAB_ICON_GLYPHS[icon.icon]
  return (
    <Glyph
      className={className}
      size={13}
      style={icon.color ? { color: icon.color } : undefined}
      aria-hidden="true"
    />
  )
}

/**
 * The icon in front of a tab's title: an app page's, or for a local terminal
 * its own or its shell's, so tabs named after their directory still tell
 * shells apart.
 */
export function TabIcon({
  tab,
  className,
}: {
  tab: Pick<Tab, "type" | "shellName" | "iconStyle">
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
