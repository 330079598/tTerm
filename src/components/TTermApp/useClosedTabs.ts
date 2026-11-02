import { useCallback, useRef } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import {
  closedEntries,
  pushClosedEntries,
  type ClosedEntry,
  type ClosedTab,
} from "@/lib/closedTabs"
import type { TabGroup } from "@/lib/tabGroups"

/**
 * The session tabs closed lately, kept for this run of the app only, and
 * undoing the last closing: a whole tab of the bar comes back with its split,
 * a pane back into the tab it was closed from.
 */
export function useClosedTabs(
  { tabsRef, reopenTabs }: Pick<AppShell, "tabsRef" | "reopenTabs">,
  prepareTab: (tab: ClosedTab) => Promise<ClosedTab>
) {
  const closedRef = useRef<ClosedEntry[]>([])

  /** Keeps the tabs `ids` as they are now, and where they are in `groups`, before they close. */
  const rememberClosedTabs = useCallback(
    (ids: readonly string[], groups: readonly TabGroup[]) => {
      const entries = closedEntries(tabsRef.current, groups, ids)
      closedRef.current = pushClosedEntries(closedRef.current, entries)
    },
    [tabsRef]
  )

  /** Opens what was closed last again; false when nothing was. */
  const reopenClosedTab = useCallback((): boolean => {
    const entry = closedRef.current[closedRef.current.length - 1]
    if (!entry) return false
    closedRef.current = closedRef.current.slice(0, -1)
    void Promise.all(entry.tabs.map(prepareTab)).then((tabs) => reopenTabs(tabs, entry.layout))
    return true
  }, [prepareTab, reopenTabs])

  const hasClosedTabs = useCallback(() => closedRef.current.length > 0, [])

  return { rememberClosedTabs, reopenClosedTab, hasClosedTabs }
}
