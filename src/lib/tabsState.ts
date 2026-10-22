// The tabs' state and every change made to it, apart from the hook that
// holds it so it can be followed one action at a time.

import { withDistinctLocalTitles } from "@/lib/localTabTitle"
import {
  breakOutPane,
  findGroupOfTab,
  focusPaneInGroups,
  getFallbackTabId,
  getPaneIds,
  insertNextTo,
  mergeGroup,
  moveGroup,
  paneLeaf,
  placePane,
  reconcileGroups,
  removeTabsFromGroups,
  resizeSplit,
  toggleZoom,
  ungroupPanes,
  type SplitDirection,
  type TabGroup,
} from "@/lib/tabGroups"
import { restoreTabGroups } from "@/lib/tabGroupsPersistence"
import { isPageTab, type PageTabType, type Tab } from "@/types/tab"

export interface RemoveTabsOptions {
  preferredActiveTabId?: string
  activatePreferred?: boolean
}

export interface RemoveTabsResult {
  tabs: Tab[]
  activeTabId: string | null
}

export interface TabsState extends RemoveTabsResult {
  groups: TabGroup[]
  /** How many group ids have been handed out, so none is handed out twice. */
  groupCount: number
}

export type TabsAction =
  | { type: "restore"; tabs: Tab[]; activeTabId: string | null; layout?: unknown }
  | { type: "add"; tab: Tab }
  | { type: "open-page"; tab: Tab }
  | { type: "rename-page"; tabType: PageTabType; title: string }
  | { type: "remove"; ids: string[]; options?: RemoveTabsOptions }
  | { type: "set-active"; id: string }
  | { type: "move"; fromIndex: number; toIndex: number }
  | { type: "duplicate"; sourceId: string; newId: string; split?: SplitDirection }
  | { type: "rename"; id: string; title: string }
  | { type: "update"; id: string; updater: (tab: Tab) => Tab }
  | { type: "place-pane"; id: string; targetId: string; direction: SplitDirection }
  | { type: "merge-group"; id: string; targetId: string; direction: SplitDirection }
  | { type: "break-out"; id: string; index?: number }
  | { type: "ungroup"; id: string }
  | { type: "toggle-zoom"; id: string }
  | { type: "resize-split"; groupId: string; path: number[]; sizes: number[] }

export function removeTabsFromState(
  tabs: Tab[],
  activeTabId: string | null,
  ids: Iterable<string>,
  options: RemoveTabsOptions = {}
): RemoveTabsResult {
  const idsToRemove = new Set(ids)
  if (!tabs.some((tab) => idsToRemove.has(tab.id))) {
    return { tabs, activeTabId }
  }

  const activeTabIndex = tabs.findIndex((tab) => tab.id === activeTabId)
  const remainingTabs = tabs.filter((tab) => !idsToRemove.has(tab.id))
  if (remainingTabs.length === 0) {
    return { tabs: [], activeTabId: null }
  }

  const preferredTab = remainingTabs.find((tab) => tab.id === options.preferredActiveTabId)
  const currentActiveTab = remainingTabs.find((tab) => tab.id === activeTabId)
  const fallbackTab = remainingTabs[Math.min(Math.max(activeTabIndex, 0), remainingTabs.length - 1)]
  const nextActiveTab =
    (options.activatePreferred ? preferredTab : currentActiveTab) ??
    preferredTab ??
    currentActiveTab ??
    fallbackTab
  const nextActiveTabId = nextActiveTab?.id ?? null

  return {
    activeTabId: nextActiveTabId,
    tabs: remainingTabs.map((tab) => ({
      ...tab,
      isActive: tab.id === nextActiveTabId,
      hasConnected: tab.id === nextActiveTabId ? true : tab.hasConnected,
    })),
  }
}

export function ensureTabDefaults(tab: Tab): Tab {
  if (isPageTab(tab) || tab.type === "remote-file-editor") {
    return {
      ...tab,
      hasConnected: true,
      sessionNonce: 0,
      connection: tab.type === "remote-file-editor" ? tab.connection : undefined,
    }
  }

  return {
    ...tab,
    hasConnected: tab.hasConnected ?? tab.isActive,
    sessionNonce: tab.sessionNonce ?? 0,
    connectionHeaderPinned: tab.connectionHeaderPinned ?? true,
  }
}

// "web" -> "web-2" -> "web-3", taking the first free index.
export function getDuplicateTabTitle(base: string, existingTitles: string[]): string {
  const titles = new Set(existingTitles)
  let index = 2
  while (titles.has(`${base}-${index}`)) {
    index++
  }
  return `${base}-${index}`
}

function nextSessionNonce(sessionNonce?: number) {
  return ((sessionNonce ?? 0) + 1) >>> 0
}

function activateTabInState(state: TabsState, id: string): TabsState {
  if (!state.tabs.some((tab) => tab.id === id)) {
    return state
  }

  return {
    ...state,
    activeTabId: id,
    tabs: state.tabs.map((tab) => ({
      ...tab,
      isActive: tab.id === id,
      hasConnected: tab.id === id ? true : tab.hasConnected,
    })),
  }
}

function applyTabsAction(state: TabsState, action: TabsAction): TabsState {
  switch (action.type) {
    case "restore": {
      const groups = restoreTabGroups(
        action.layout,
        action.tabs.map((tab) => tab.id)
      ).map((group, index) => ({ ...group, id: `group-${index + 1}` }))
      return {
        activeTabId: action.activeTabId,
        groups,
        groupCount: groups.length,
        tabs: action.tabs.map((tab) =>
          ensureTabDefaults({
            ...tab,
            isActive: tab.id === action.activeTabId,
            hasConnected: tab.id === action.activeTabId,
          })
        ),
      }
    }
    case "add":
      return {
        ...state,
        activeTabId: action.tab.id,
        tabs: [
          ...state.tabs.map((tab) => ({ ...tab, isActive: false })),
          { ...action.tab, isActive: true, hasConnected: true },
        ],
      }
    case "open-page": {
      const existingPageTab = state.tabs.find((tab) => tab.type === action.tab.type)
      if (existingPageTab) {
        return activateTabInState(state, existingPageTab.id)
      }
      return {
        ...state,
        activeTabId: action.tab.id,
        tabs: [
          ...state.tabs.map((tab) => ({ ...tab, isActive: false })),
          { ...action.tab, isActive: true },
        ],
      }
    }
    case "rename-page": {
      let hasChanges = false
      const tabs = state.tabs.map((tab) => {
        if (tab.type !== action.tabType || tab.title === action.title) {
          return tab
        }
        hasChanges = true
        return { ...tab, title: action.title }
      })
      return hasChanges ? { ...state, tabs } : state
    }
    case "remove": {
      const groups = removeTabsFromGroups(state.groups, action.ids)
      const preferredActiveTabId =
        action.options?.preferredActiveTabId ??
        getFallbackTabId(state.groups, groups, state.activeTabId)
      return {
        ...state,
        ...removeTabsFromState(state.tabs, state.activeTabId, action.ids, {
          ...action.options,
          preferredActiveTabId,
        }),
        groups,
      }
    }
    case "set-active":
      return activateTabInState(state, action.id)
    case "move":
      return { ...state, groups: moveGroup(state.groups, action.fromIndex, action.toIndex) }
    case "duplicate": {
      const sourceTab = state.tabs.find((tab) => tab.id === action.sourceId)
      if (!sourceTab || isPageTab(sourceTab) || sourceTab.type === "remote-file-editor") {
        return state
      }
      const { id: _id, isActive: _isActive, ...tabData } = sourceTab
      // A copy named automatically starts in the same directory, under the
      // same name; the tab numbers tell them apart.
      const baseTitle = sourceTab.autoTitle
        ? undefined
        : (sourceTab.duplicateBaseTitle ?? sourceTab.title)
      const newTab: Tab = ensureTabDefaults({
        ...tabData,
        connection: tabData.connection ?? {
          type: tabData.type === "terminal" ? "terminal" : "ssh",
        },
        sessionNonce: nextSessionNonce(sourceTab.sessionNonce),
        id: action.newId,
        title:
          baseTitle === undefined
            ? sourceTab.title
            : getDuplicateTabTitle(
                baseTitle,
                state.tabs.map((tab) => tab.title)
              ),
        duplicateBaseTitle: baseTitle,
        isActive: true,
        hasConnected: true,
      })
      const sourceGroup = action.split && findGroupOfTab(state.groups, action.sourceId)
      const split = action.split
      return {
        ...state,
        activeTabId: action.newId,
        tabs: [...state.tabs.map((tab) => ({ ...tab, isActive: false })), newTab],
        groups:
          sourceGroup && split
            ? state.groups.map((group) =>
                group === sourceGroup
                  ? {
                      ...group,
                      root: insertNextTo(group.root, action.sourceId, paneLeaf(newTab.id), split),
                      zoomedTabId: undefined,
                    }
                  : group
              )
            : state.groups,
      }
    }
    case "rename":
      return {
        ...state,
        tabs: state.tabs.map((tab) =>
          tab.id === action.id
            ? { ...tab, title: action.title, duplicateBaseTitle: undefined, autoTitle: undefined }
            : tab
        ),
      }
    case "update":
      return {
        ...state,
        tabs: state.tabs.map((tab) => {
          if (tab.id !== action.id) {
            return tab
          }
          const updatedTab = ensureTabDefaults(action.updater(tab))
          return { ...updatedTab, isActive: updatedTab.id === state.activeTabId }
        }),
      }
    case "place-pane":
      return activateTabInState(
        { ...state, groups: placePane(state.groups, action.id, action.targetId, action.direction) },
        action.id
      )
    case "merge-group": {
      const groups = mergeGroup(state.groups, action.id, action.targetId, action.direction)
      const merged = findGroupOfTab(groups, action.targetId)
      return groups === state.groups || !merged
        ? state
        : activateTabInState({ ...state, groups }, merged.activeTabId)
    }
    case "break-out": {
      const groupCount = state.groupCount + 1
      const groups = breakOutPane(state.groups, action.id, `group-${groupCount}`, action.index)
      return groups === state.groups
        ? state
        : activateTabInState({ ...state, groups, groupCount }, action.id)
    }
    case "ungroup": {
      let groupCount = state.groupCount
      const groups = ungroupPanes(state.groups, action.id, () => `group-${++groupCount}`)
      return { ...state, groups, groupCount }
    }
    case "toggle-zoom":
      return activateTabInState(
        { ...state, groups: toggleZoom(state.groups, action.id) },
        action.id
      )
    case "resize-split":
      return {
        ...state,
        groups: state.groups.map((group) =>
          group.id === action.groupId
            ? { ...group, root: resizeSplit(group.root, action.path, action.sizes) }
            : group
        ),
      }
  }
}

/**
 * Keeps the groups in step with the tabs: every tab sits in one group, the
 * active tab is its group's active pane, and the tabs are listed in the order
 * the bar shows their groups.
 */
function settleGroups(state: TabsState): TabsState {
  let groupCount = state.groupCount
  let groups = reconcileGroups(
    state.groups,
    state.tabs.map((tab) => tab.id),
    () => `group-${++groupCount}`
  )
  if (state.activeTabId) {
    groups = focusPaneInGroups(groups, state.activeTabId)
  }
  const order = groups.flatMap((group) => getPaneIds(group.root))
  const inOrder = order.every((id, index) => state.tabs[index]?.id === id)
  let tabs = state.tabs
  if (!inOrder) {
    const byId = new Map(state.tabs.map((tab) => [tab.id, tab]))
    tabs = order.flatMap((id) => byId.get(id) ?? [])
  }
  if (groups === state.groups && tabs === state.tabs && groupCount === state.groupCount) {
    return state
  }
  return { ...state, tabs, groups, groupCount }
}

// Automatic titles depend on the other tabs, so every change is followed by a
// pass that sets apart local tabs in different directories of the same name.
export function tabsReducer(state: TabsState, action: TabsAction): TabsState {
  const next = settleGroups(applyTabsAction(state, action))
  if (next.tabs === state.tabs) return next
  const tabs = withDistinctLocalTitles(next.tabs)
  return tabs === next.tabs ? next : { ...next, tabs }
}
