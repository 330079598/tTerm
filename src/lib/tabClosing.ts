import type { TabContextMenuAction } from "@/types/tab"

export type TabCloseAction = "close-others" | "close-left" | "close-right"

export interface TabCloseContext {
  index: number
  otherIds: string[]
  leftIds: string[]
  rightIds: string[]
}

export interface TabCloseMenuLabels {
  closeTab: string
  closeOtherTabs: string
  closeTabsToLeft: string
  closeTabsToRight: string
}

export function getTabCloseContext(tabIds: string[], tabId: string): TabCloseContext | null {
  const index = tabIds.indexOf(tabId)
  if (index === -1) {
    return null
  }

  return {
    index,
    otherIds: tabIds.filter((id) => id !== tabId),
    leftIds: tabIds.slice(0, index),
    rightIds: tabIds.slice(index + 1),
  }
}

export function getTabIdsForCloseAction(
  tabIds: string[],
  tabId: string,
  action: TabCloseAction
): string[] {
  const context = getTabCloseContext(tabIds, tabId)
  if (!context) {
    return []
  }

  switch (action) {
    case "close-others":
      return context.otherIds
    case "close-left":
      return context.leftIds
    case "close-right":
      return context.rightIds
  }
}

/**
 * The closing items of a tab of the bar's menu. A split tab closes with all its
 * panes, while the close shortcut closes only the active pane, so it shows
 * the shortcut only when it has a single pane.
 */
export function getTabCloseMenuActions(
  tabIds: string[],
  tabId: string,
  labels: TabCloseMenuLabels,
  isSplit = false
): TabContextMenuAction[] {
  const context = getTabCloseContext(tabIds, tabId)
  return [
    {
      label: labels.closeTab,
      action: "close-tab",
      icon: "x",
      shortcut: isSplit ? undefined : "tabs.closeActive",
    },
    {
      label: labels.closeOtherTabs,
      action: "close-others",
      disabled: !context?.otherIds.length,
    },
    {
      label: labels.closeTabsToLeft,
      action: "close-left",
      disabled: !context?.leftIds.length,
    },
    {
      label: labels.closeTabsToRight,
      action: "close-right",
      disabled: !context?.rightIds.length,
    },
  ]
}
