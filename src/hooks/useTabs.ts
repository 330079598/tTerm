import { useCallback, useReducer, useRef } from "react"
import { ensureTabDefaults, tabsReducer, type RemoveTabsOptions } from "@/lib/tabsState"
import { isPageTab, type PageTabType, Tab } from "@/types/tab"

export interface UseTabsReturn {
  tabs: Tab[]
  activeTabId: string | null
  addTab: (tab: Omit<Tab, "id" | "isActive">) => string
  openSettingsTab: (title: string) => void
  renameSettingsTab: (title: string) => void
  openTunnelsTab: (title: string) => void
  renameTunnelsTab: (title: string) => void
  openLogsTab: (title: string) => void
  renameLogsTab: (title: string) => void
  removeTabs: (ids: string[], options?: RemoveTabsOptions) => void
  setActiveTab: (id: string) => void
  moveTab: (fromIndex: number, toIndex: number) => void
  duplicateTab: (id: string) => string | null
  renameTab: (id: string, newTitle: string) => void
  restoreSession: (tabs: Tab[], activeTabId: string | null) => void
  updateTab: (id: string, updater: (tab: Tab) => Tab) => void
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

  const openPageTab = useCallback(
    (type: PageTabType, title: string) => {
      const tab = ensureTabDefaults({
        id: generateTabId(),
        title,
        type,
        isActive: true,
        isModified: false,
        hasConnected: true,
      })
      dispatch({ type: "open-page", tab })
    },
    [generateTabId]
  )

  const renamePageTab = useCallback((tabType: PageTabType, title: string) => {
    dispatch({ type: "rename-page", tabType, title })
  }, [])

  const openSettingsTab = useCallback(
    (title: string) => openPageTab("settings", title),
    [openPageTab]
  )
  const renameSettingsTab = useCallback(
    (title: string) => renamePageTab("settings", title),
    [renamePageTab]
  )
  const openTunnelsTab = useCallback(
    (title: string) => openPageTab("tunnels", title),
    [openPageTab]
  )
  const renameTunnelsTab = useCallback(
    (title: string) => renamePageTab("tunnels", title),
    [renamePageTab]
  )
  const openLogsTab = useCallback((title: string) => openPageTab("logs", title), [openPageTab])
  const renameLogsTab = useCallback(
    (title: string) => renamePageTab("logs", title),
    [renamePageTab]
  )

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
      if (!tab || isPageTab(tab) || tab.type === "remote-file-editor") {
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
    openTunnelsTab,
    renameTunnelsTab,
    openLogsTab,
    renameLogsTab,
    removeTabs,
    setActiveTab,
    moveTab,
    duplicateTab,
    renameTab,
    restoreSession,
    updateTab,
  }
}
