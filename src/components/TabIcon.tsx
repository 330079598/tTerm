import { ScrollText, Settings, Waypoints } from "lucide-react"

import type { Tab } from "@/types/tab"

const PAGE_ICONS = { settings: Settings, tunnels: Waypoints, logs: ScrollText } as const

/** The icon in front of a tab's title, for the kinds of tab that have one. */
export function TabIcon({ tab, className }: { tab: Pick<Tab, "type">; className?: string }) {
  const Icon = tab.type in PAGE_ICONS ? PAGE_ICONS[tab.type as keyof typeof PAGE_ICONS] : null
  return Icon && <Icon className={className} size={13} aria-hidden="true" />
}
