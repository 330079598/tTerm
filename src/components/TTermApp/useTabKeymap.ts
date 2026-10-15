import { useEffect, useMemo } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import { useKeymap } from "@/contexts/KeymapContext"
import { getSiblingTabId, getTabIdAtPosition, resolveTabShortcutTarget } from "@/lib/tabNavigation"
import { nudgeTab, nudgeTabBar } from "@/lib/tabNudge"
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
    // A shortcut that lands on the active tab, or on no tab, would otherwise
    // change nothing on screen and look as if it had not fired.
    const goToTab = (tabId: string | undefined) => {
      const target = resolveTabShortcutTarget(tabId, activeTabId)
      if (target.kind === "switch") {
        setActiveTab(target.tabId)
      } else if (target.kind === "current") {
        nudgeTab(target.tabId)
      } else {
        nudgeTabBar()
      }
    }

    const unregisterNewTab = registerHandler("workspace.newTab", () => {
      handleNewTab()
    })
    const unregisterNextTab = registerHandler("tabs.next", () => {
      goToTab(getSiblingTabId(keymapTabIds, activeTabId, 1))
    })
    const unregisterPrevTab = registerHandler("tabs.prev", () => {
      goToTab(getSiblingTabId(keymapTabIds, activeTabId, -1))
    })
    // The chord's key comes from the physical key, while event.key is what
    // Option types on macOS ("¡" for Option+1), so only the chord has a digit.
    const unregisterSwitchToNth = registerHandler("tabs.switchToNth", (_event, chord) => {
      const position = Number.parseInt(chord.key, 10)
      if (Number.isNaN(position)) {
        return false
      }
      goToTab(getTabIdAtPosition(keymapTabIds, position))
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
