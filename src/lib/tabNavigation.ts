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

// The ninth tab shortcut always picks the last tab, as Chrome, Safari, Terminal
// and iTerm2 do, so the far end of a long tab bar is one key away.
const LAST_TAB_POSITION = 9

/**
 * Returns the tab id at the given 1-based position, or undefined when out of
 * range; position 9 is the last tab, however many tabs there are.
 */
export function getTabIdAtPosition(tabIds: string[], position: number): string | undefined {
  if (position === LAST_TAB_POSITION) {
    return tabIds[tabIds.length - 1]
  }
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
