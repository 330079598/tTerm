/** Tab ordering helpers for next/previous/nth-tab navigation. */

/**
 * Returns the tab id `offset` positions away from `activeTabId` in
 * `tabIds` (display order), wrapping around both ends.
 */
export function getSiblingTabId(
  tabIds: string[],
  activeTabId: string | null,
  offset: number
): string | undefined {
  if (!activeTabId || tabIds.length === 0) {
    return undefined
  }
  const index = tabIds.indexOf(activeTabId)
  if (index === -1) {
    return undefined
  }
  const nextIndex = (index + offset + tabIds.length) % tabIds.length
  return tabIds[nextIndex]
}

/** Returns the tab id at the given 1-based position, or undefined when out of range. */
export function getTabIdAtPosition(tabIds: string[], position: number): string | undefined {
  if (position < 1 || position > tabIds.length) {
    return undefined
  }
  return tabIds[position - 1]
}

/** What a tab shortcut leads to: another tab, the tab already active, or no tab at all. */
export type TabShortcutTarget =
  { kind: "switch"; tabId: string } | { kind: "current"; tabId: string } | { kind: "none" }

/** Classifies the tab a shortcut resolved to, so a shortcut that changes nothing can say so. */
export function resolveTabShortcutTarget(
  tabId: string | undefined,
  activeTabId: string | null
): TabShortcutTarget {
  if (!tabId) {
    return { kind: "none" }
  }
  return tabId === activeTabId ? { kind: "current", tabId } : { kind: "switch", tabId }
}
