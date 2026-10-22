// The tabs' state and every change made to it, apart from the hook that
// holds it so it can be followed one action at a time.

import { withDistinctLocalTitles } from "@/lib/localTabTitle"
import { isPageTab, type PageTabType, type Tab } from "@/types/tab"

export interface RemoveTabsOptions {
  preferredActiveTabId?: string
  activatePreferred?: boolean
}

export interface RemoveTabsResult {
  tabs: Tab[]
  activeTabId: string | null
}

export type TabsState = RemoveTabsResult

export type TabsAction =
  | { type: "restore"; tabs: Tab[]; activeTabId: string | null }
  | { type: "add"; tab: Tab }
  | { type: "open-page"; tab: Tab }
  | { type: "rename-page"; tabType: PageTabType; title: string }
  | { type: "remove"; ids: string[]; options?: RemoveTabsOptions }
  | { type: "set-active"; id: string }
  | { type: "move"; fromIndex: number; toIndex: number }
  | { type: "duplicate"; sourceId: string; newId: string }
  | { type: "rename"; id: string; title: string }
  | { type: "update"; id: string; updater: (tab: Tab) => Tab }

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
    case "restore":
      return {
        activeTabId: action.activeTabId,
        tabs: action.tabs.map((tab) =>
          ensureTabDefaults({
            ...tab,
            isActive: tab.id === action.activeTabId,
            hasConnected: tab.id === action.activeTabId,
          })
        ),
      }
    case "add":
      return {
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
    case "remove":
      return removeTabsFromState(state.tabs, state.activeTabId, action.ids, action.options)
    case "set-active":
      return activateTabInState(state, action.id)
    case "move": {
      if (
        action.fromIndex < 0 ||
        action.fromIndex >= state.tabs.length ||
        action.toIndex < 0 ||
        action.toIndex >= state.tabs.length ||
        action.fromIndex === action.toIndex
      ) {
        return state
      }
      const tabs = [...state.tabs]
      const [movedTab] = tabs.splice(action.fromIndex, 1)
      tabs.splice(action.toIndex, 0, movedTab)
      return { ...state, tabs }
    }
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
      return {
        activeTabId: action.newId,
        tabs: [...state.tabs.map((tab) => ({ ...tab, isActive: false })), newTab],
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
  }
}

// Automatic titles depend on the other tabs, so every change is followed by a
// pass that sets apart local tabs in different directories of the same name.
export function tabsReducer(state: TabsState, action: TabsAction): TabsState {
  const next = applyTabsAction(state, action)
  if (next.tabs === state.tabs) return next
  const tabs = withDistinctLocalTitles(next.tabs)
  return tabs === next.tabs ? next : { ...next, tabs }
}
