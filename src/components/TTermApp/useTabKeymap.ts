import { useEffect, useMemo } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import { useKeymap } from "@/contexts/KeymapContext"
import { getSiblingTabId, getTabIdAtPosition, resolveTabShortcutTarget } from "@/lib/tabNavigation"
import { tabConnectionActionLabel } from "@/lib/tabMenuActions"
import { nudgeTab, nudgeTabBar } from "@/lib/tabNudge"
import { isPageTab, type Tab } from "@/types/tab"

interface TabKeymapActions {
  handleNewTab: () => void
  handleRemoveTab: (id: string) => Promise<void>
  openRenameDialog: (tab: Tab) => void
  /** Opens the tab closed last again; false when there is none. */
  reopenClosedTab: () => boolean
  /** Saves a tab's connection, or edits the saved one it came from. */
  editTabProfile: (tab: Tab) => void
}

/**
 * Keyboard shortcuts for opening, switching, renaming, duplicating, closing
 * and reopening tabs, and for saving their connections. Switching goes
 * between the tabs of the bar, to the pane each had the focus in; renaming,
 * duplicating, closing and saving act on the active pane.
 */
export function useTabKeymap(shell: AppShell, actions: TabKeymapActions) {
  const { tabs, groups, activeTabId, setActiveTab, duplicateTab } = shell
  const { handleNewTab, handleRemoveTab, openRenameDialog, reopenClosedTab, editTabProfile } =
    actions
  const { registerHandler } = useKeymap()

  const keymapTabIds = useMemo(() => groups.map((group) => group.activeTabId), [groups])

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
    // Page tabs and remote file editors cannot be duplicated, so the chord passes through.
    const unregisterDuplicateActive = registerHandler("tabs.duplicateActive", () => {
      if (!activeTabId || duplicateTab(activeTabId) === null) {
        return false
      }
    })
    const unregisterCloseActive = registerHandler("tabs.closeActive", () => {
      if (activeTabId) {
        void handleRemoveTab(activeTabId)
      }
    })
    // With nothing closed, the chord goes on to the terminal.
    const unregisterReopenClosed = registerHandler("tabs.reopenClosed", () => {
      if (!reopenClosedTab()) return false
    })
    // Only where the tab's menu offers it; a page or a serial tab never saved
    // passes the chord on.
    const unregisterSaveConnection = registerHandler("tabs.saveConnection", () => {
      const tab = tabs.find((candidate) => candidate.id === activeTabId)
      if (!tab || !tabConnectionActionLabel(tab)) return false
      editTabProfile(tab)
    })
    return () => {
      unregisterSaveConnection()
      unregisterReopenClosed()
      unregisterNewTab()
      unregisterNextTab()
      unregisterPrevTab()
      unregisterSwitchToNth()
      unregisterRenameActive()
      unregisterDuplicateActive()
      unregisterCloseActive()
    }
  }, [
    registerHandler,
    tabs,
    keymapTabIds,
    activeTabId,
    setActiveTab,
    duplicateTab,
    handleNewTab,
    handleRemoveTab,
    openRenameDialog,
    reopenClosedTab,
    editTabProfile,
  ])
}
