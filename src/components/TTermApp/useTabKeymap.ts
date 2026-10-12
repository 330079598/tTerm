import { useEffect, useMemo } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import { useKeymap } from "@/contexts/KeymapContext"
import { getSiblingTabId, getTabIdAtPosition } from "@/lib/tabNavigation"

interface TabKeymapActions {
  handleNewTab: () => void
  handleRemoveTab: (id: string) => Promise<void>
}

/** Keyboard shortcuts for opening, switching and closing tabs. */
export function useTabKeymap(shell: AppShell, actions: TabKeymapActions) {
  const { tabs, activeTabId, setActiveTab } = shell
  const { handleNewTab, handleRemoveTab } = actions
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
      unregisterCloseActive()
    }
  }, [registerHandler, keymapTabIds, activeTabId, setActiveTab, handleNewTab, handleRemoveTab])
}
