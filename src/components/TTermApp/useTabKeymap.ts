import { useEffect, useMemo } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import { useKeymap } from "@/contexts/KeymapContext"
import { getSiblingTabId, getTabIdAtPosition } from "@/lib/tabNavigation"
import { isPageTab, type Tab } from "@/types/tab"

interface TabKeymapActions {
  handleNewTab: () => void
  handleRemoveTab: (id: string) => Promise<void>
  openRenameDialog: (tab: Tab) => void
}

/** Keyboard shortcuts for opening, switching, renaming and closing tabs. */
export function useTabKeymap(shell: AppShell, actions: TabKeymapActions) {
  const { tabs, activeTabId, setActiveTab } = shell
  const { handleNewTab, handleRemoveTab, openRenameDialog } = actions
  const { registerHandler } = useKeymap()

  const keymapTabIds = useMemo(() => tabs.map((tab) => tab.id), [tabs])

  useEffect(() => {
    const unregisterNewTab = registerHandler("workspace.newTab", () => {
      handleNewTab()
    })
    const unregisterNextTab = registerHandler("tabs.next", () => {
      const nextId = getSiblingTabId(keymapTabIds, activeTabId, 1)
      if (nextId) {
        setActiveTab(nextId)
      }
    })
    const unregisterPrevTab = registerHandler("tabs.prev", () => {
      const prevId = getSiblingTabId(keymapTabIds, activeTabId, -1)
      if (prevId) {
        setActiveTab(prevId)
      }
    })
    const unregisterSwitchToNth = registerHandler("tabs.switchToNth", (event) => {
      const position = Number.parseInt(event.key, 10)
      if (Number.isNaN(position)) {
        return false
      }
      const tabId = getTabIdAtPosition(keymapTabIds, position)
      if (tabId) {
        setActiveTab(tabId)
      }
    })
    // Page tabs keep their fixed titles, as their context menu offers no rename.
    const unregisterRenameActive = registerHandler("tabs.renameActive", () => {
      const tab = tabs.find((candidate) => candidate.id === activeTabId)
      if (!tab || isPageTab(tab)) {
        return false
      }
      openRenameDialog(tab)
    })
    const unregisterCloseActive = registerHandler("tabs.closeActive", () => {
      if (activeTabId) {
        void handleRemoveTab(activeTabId)
      }
    })
    return () => {
      unregisterNewTab()
      unregisterNextTab()
      unregisterPrevTab()
      unregisterSwitchToNth()
      unregisterRenameActive()
      unregisterCloseActive()
    }
  }, [
    registerHandler,
    tabs,
    keymapTabIds,
    activeTabId,
    setActiveTab,
    handleNewTab,
    handleRemoveTab,
    openRenameDialog,
  ])
}
