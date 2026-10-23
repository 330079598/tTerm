import { useCallback, useReducer, useRef } from "react"
import { countTemplatePanes, type LayoutShape } from "@/lib/paneTemplates"
import { findGroupOfTab, getPaneIds, type SplitDirection, type TabGroup } from "@/lib/tabGroups"
import { ensureTabDefaults, tabsReducer, type RemoveTabsOptions } from "@/lib/tabsState"
import { isPageTab, type PageTabType, Tab } from "@/types/tab"

export interface UseTabsReturn {
  tabs: Tab[]
  activeTabId: string | null
  /** The tabs of the tab bar, in its order, each holding one or more of `tabs` as panes. */
  groups: TabGroup[]
  addTab: (tab: Omit<Tab, "id" | "isActive">) => string
  openSettingsTab: (title: string) => void
  renameSettingsTab: (title: string) => void
  openTunnelsTab: (title: string) => void
  renameTunnelsTab: (title: string) => void
  openLogsTab: (title: string) => void
  renameLogsTab: (title: string) => void
  removeTabs: (ids: string[], options?: RemoveTabsOptions) => void
  setActiveTab: (id: string) => void
  /** Moves a tab of the tab bar, a group, from one place in the bar to another. */
  moveTab: (fromIndex: number, toIndex: number) => void
  /** Copies a tab into a new tab of the bar, or into a pane beside it when `split` is given. */
  duplicateTab: (id: string, split?: SplitDirection) => string | null
  renameTab: (id: string, newTitle: string) => void
  restoreSession: (tabs: Tab[], activeTabId: string | null, layout?: unknown) => void
  updateTab: (id: string, updater: (tab: Tab) => Tab) => void
  /** Moves a pane beside another one, in the same tab of the bar or another. */
  placePane: (id: string, targetId: string, direction: SplitDirection) => void
  /** Moves all the panes of one tab of the bar beside a pane of another. */
  mergeGroup: (id: string, targetId: string, direction: SplitDirection) => void
  /** Takes a pane out into a tab of the bar of its own, after its tab or at `index`. */
  breakOutPane: (id: string, index?: number) => void
  /** Gives each pane of a tab of the bar a tab of its own. */
  ungroupPanes: (id: string) => void
  togglePaneZoom: (id: string) => void
  resizeSplit: (groupId: string, path: number[], sizes: number[]) => void
  /**
   * Lays the tab of the bar holding pane `id` out as a template; panes the
   * template adds open copies of that pane.
   */
  applyLayoutTemplate: (id: string, root: LayoutShape) => void
}

export function useTabs(): UseTabsReturn {
  const [state, dispatch] = useReducer(tabsReducer, {
    tabs: [],
    activeTabId: null,
    groups: [],
    groupCount: 0,
  })
  const { tabs, activeTabId, groups } = state
  const tabIdCounter = useRef(0)

  const generateTabId = useCallback(() => {
    return `tab-${++tabIdCounter.current}`
  }, [])

  // Restore session
  const restoreSession = useCallback(
    (restoredTabs: Tab[], restoredActiveTabId: string | null, layout?: unknown) => {
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

      dispatch({ type: "restore", tabs: restoredTabs, activeTabId: restoredActiveTabId, layout })
    },
    []
  )

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
    (id: string, split?: SplitDirection) => {
      const tab = tabs.find((candidate) => candidate.id === id)
      if (!tab || isPageTab(tab) || tab.type === "remote-file-editor") {
        return null
      }

      const newId = generateTabId()
      dispatch({ type: "duplicate", sourceId: id, newId, split })
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

  const placePaneCallback = useCallback(
    (id: string, targetId: string, direction: SplitDirection) =>
      dispatch({ type: "place-pane", id, targetId, direction }),
    []
  )
  const mergeGroupCallback = useCallback(
    (id: string, targetId: string, direction: SplitDirection) =>
      dispatch({ type: "merge-group", id, targetId, direction }),
    []
  )
  const breakOutPaneCallback = useCallback(
    (id: string, index?: number) => dispatch({ type: "break-out", id, index }),
    []
  )
  const ungroupPanesCallback = useCallback((id: string) => dispatch({ type: "ungroup", id }), [])
  const togglePaneZoom = useCallback((id: string) => dispatch({ type: "toggle-zoom", id }), [])
  const resizeSplitCallback = useCallback(
    (groupId: string, path: number[], sizes: number[]) =>
      dispatch({ type: "resize-split", groupId, path, sizes }),
    []
  )

  const applyLayoutTemplate = useCallback(
    (id: string, root: LayoutShape) => {
      const group = findGroupOfTab(groups, id)
      if (!group) return
      const missing = countTemplatePanes(root) - getPaneIds(group.root).length
      const newIds = Array.from({ length: Math.max(0, missing) }, generateTabId)
      dispatch({ type: "apply-template", id, root, newIds })
    },
    [generateTabId, groups]
  )

  return {
    tabs,
    activeTabId,
    groups,
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
    placePane: placePaneCallback,
    mergeGroup: mergeGroupCallback,
    breakOutPane: breakOutPaneCallback,
    ungroupPanes: ungroupPanesCallback,
    togglePaneZoom,
    resizeSplit: resizeSplitCallback,
    applyLayoutTemplate,
  }
}
