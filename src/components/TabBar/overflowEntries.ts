import type { TabPanes } from "@/components/TabBar/TabItem"
import type { Tab } from "@/types/tab"

/** A tab of the bar in the overflow menu, and the panes listed under it when it is split. */
export interface OverflowEntry {
  tab: Tab
  /** The tab's place in the bar, from 1. */
  number: number
  /** Its panes, every one or those that match, in their order; empty when it isn't split. */
  panes: Tab[]
  /** The tab matched by itself, not only through one of its panes. */
  matched: boolean
}

function searchableText(tab: Tab): string {
  const connection = tab.connection
  return [
    tab.title,
    tab.type,
    connection?.host,
    connection?.username,
    connection?.profileName,
    tab.remoteFile?.profileName,
    tab.remoteFile?.connectionLabel,
    tab.remoteFile?.host,
    tab.remoteFile?.path,
    connection?.port ? String(connection.port) : undefined,
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase()
}

/**
 * The tabs of the bar the overflow menu lists for `query`. A split tab is
 * listed when it or any of its panes matches, and lists the panes that do,
 * so a session in a pane out of sight can be found too.
 */
export function getOverflowEntries(
  tabs: readonly Tab[],
  tabPanes: ReadonlyMap<string, TabPanes>,
  query: string
): OverflowEntry[] {
  const normalized = query.trim().toLowerCase()
  return tabs.flatMap((tab, index) => {
    const number = index + 1
    const panes = tabPanes.get(tab.id)?.tabs ?? []
    const split = panes.length > 1 ? panes : []
    if (!normalized) return [{ tab, number, panes: split, matched: true }]
    const matched = `${number} ${searchableText(tab)}`.includes(normalized)
    const matchingPanes = split.filter((pane) => searchableText(pane).includes(normalized))
    if (!matched && matchingPanes.length === 0) return []
    return [{ tab, number, panes: matchingPanes, matched }]
  })
}

/** What Enter opens: the first tab that matched by itself, or else the first matching pane. */
export function getFirstOverflowTarget(entries: readonly OverflowEntry[]): string | undefined {
  const first = entries[0]
  if (!first) return undefined
  return first.matched ? first.tab.id : first.panes[0]?.id
}
