import { useCallback, useReducer, useRef } from "react"
import { Tab } from "@/types/tab"

export interface UseTabsReturn {
  tabs: Tab[]
  activeTabId: string | null
  addTab: (tab: Omit<Tab, "id" | "isActive">) => string
  openSettingsTab: (title: string) => void
  renameSettingsTab: (title: string) => void
  removeTabs: (ids: string[], options?: RemoveTabsOptions) => void
  setActiveTab: (id: string) => void
  moveTab: (fromIndex: number, toIndex: number) => void
  duplicateTab: (id: string) => string | null
  renameTab: (id: string, newTitle: string) => void
  restoreSession: (tabs: Tab[], activeTabId: string | null) => void
  updateTab: (id: string, updater: (tab: Tab) => Tab) => void
}

export interface RemoveTabsOptions {
  preferredActiveTabId?: string
  activatePreferred?: boolean
}

export interface RemoveTabsResult {
  tabs: Tab[]
  activeTabId: string | null
}

type TabsState = RemoveTabsResult

type TabsAction =
  | { type: "restore"; tabs: Tab[]; activeTabId: string | null }
  | { type: "add"; tab: Tab }
  | { type: "open-settings"; tab: Tab }
  | { type: "rename-settings"; title: string }
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

function ensureTabDefaults(tab: Tab): Tab {
  if (tab.type === "settings" || tab.type === "remote-file-editor") {
    return {
      ...tab,
      hasConnected: true,
      sessionNonce: 0,
      connection: tab.type === "settings" ? undefined : tab.connection,
    }
  }

  return {
    ...tab,
    hasConnected: tab.hasConnected ?? tab.isActive,
    sessionNonce: tab.sessionNonce ?? 0,
    connectionHeaderPinned: tab.connectionHeaderPinned ?? true,
  }
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

function tabsReducer(state: TabsState, action: TabsAction): TabsState {
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
    case "open-settings": {
      const existingSettingsTab = state.tabs.find((tab) => tab.type === "settings")
      if (existingSettingsTab) {
        return activateTabInState(state, existingSettingsTab.id)
      }
      return {
        activeTabId: action.tab.id,
        tabs: [
          ...state.tabs.map((tab) => ({ ...tab, isActive: false })),
          { ...action.tab, isActive: true },
        ],
      }
    }
    case "rename-settings": {
      let hasChanges = false
      const tabs = state.tabs.map((tab) => {
        if (tab.type !== "settings" || tab.title === action.title) {
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
      if (!sourceTab || sourceTab.type === "settings" || sourceTab.type === "remote-file-editor") {
        return state
      }
      const { id: _id, isActive: _isActive, ...tabData } = sourceTab
      const newTab: Tab = ensureTabDefaults({
        ...tabData,
        connection: tabData.connection ?? {
          type: tabData.type === "terminal" ? "terminal" : "ssh",
        },
        sessionNonce: nextSessionNonce(sourceTab.sessionNonce),
        id: action.newId,
        title: `${sourceTab.title} (Copy)`,
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
          tab.id === action.id ? { ...tab, title: action.title } : tab
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

export function useTabs(): UseTabsReturn {
  const [state, dispatch] = useReducer(tabsReducer, { tabs: [], activeTabId: null })
  const { tabs, activeTabId } = state
  const tabIdCounter = useRef(0)

  const generateTabId = useCallback(() => {
    return `tab-${++tabIdCounter.current}`
  }, [])

  // Restore session
  const restoreSession = useCallback((restoredTabs: Tab[], restoredActiveTabId: string | null) => {
    // Update counter to avoid ID conflicts
    const maxId = restoredTabs.reduce((max, tab) => {
      const match = tab.id.match(/tab-(\d+)/)
      if (match) {
        const num = parseInt(match[1], 10)
        return Math.max(max, num)
      }
      return max
    }, 0)
    tabIdCounter.current = maxId

    dispatch({ type: "restore", tabs: restoredTabs, activeTabId: restoredActiveTabId })
  }, [])

  const addTab = useCallback(
    (tabData: Omit<Tab, "id" | "isActive">) => {
      const id = generateTabId()
      const newTab: Tab = ensureTabDefaults({
        ...tabData,
        id,
        isActive: false,
        hasConnected: true,
      })

      dispatch({ type: "add", tab: newTab })
      return id
    },
    [generateTabId]
  )

  const openSettingsTab = useCallback(
    (title: string) => {
      const tab = ensureTabDefaults({
        id: generateTabId(),
        title,
        type: "settings",
        isActive: true,
        isModified: false,
        hasConnected: true,
      })
      dispatch({ type: "open-settings", tab })
    },
    [generateTabId]
  )

  const renameSettingsTab = useCallback((title: string) => {
    dispatch({ type: "rename-settings", title })
  }, [])

  const removeTabs = useCallback((ids: string[], options?: RemoveTabsOptions) => {
    if (ids.length === 0) return
    dispatch({ type: "remove", ids, options })
  }, [])

  const setActiveTab = useCallback((id: string) => {
    dispatch({ type: "set-active", id })
  }, [])

  const moveTab = useCallback((fromIndex: number, toIndex: number) => {
    dispatch({ type: "move", fromIndex, toIndex })
  }, [])

  const duplicateTab = useCallback(
    (id: string) => {
      const tab = tabs.find((candidate) => candidate.id === id)
      if (!tab || tab.type === "settings" || tab.type === "remote-file-editor") {
        return null
      }

      const newId = generateTabId()
      dispatch({ type: "duplicate", sourceId: id, newId })
      return newId
    },
    [generateTabId, tabs]
  )

  const renameTab = useCallback((id: string, newTitle: string) => {
    dispatch({ type: "rename", id, title: newTitle })
  }, [])

  const updateTab = useCallback((id: string, updater: (tab: Tab) => Tab) => {
    dispatch({ type: "update", id, updater })
  }, [])

  return {
    tabs,
    activeTabId,
    addTab,
    openSettingsTab,
    renameSettingsTab,
    removeTabs,
    setActiveTab,
    moveTab,
    duplicateTab,
    renameTab,
    restoreSession,
    updateTab,
  }
}
