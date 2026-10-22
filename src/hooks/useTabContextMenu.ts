import { useCallback, useState } from "react"

import type { ContextMenuState, RenameDialogState } from "@/components/TTermApp/types"
import { showLogsForTab } from "@/lib/appNavigation"
import { nameLocalTab } from "@/lib/localTabTitle"
import type { SplitDirection } from "@/lib/tabGroups"
import { hasTabIcon, type TabIconStyle } from "@/lib/tabIcons"
import type { Tab, TabContextMenuAction } from "@/types/tab"

interface UseTabContextMenuDeps {
  handleNewTab: () => void
  duplicateTab: (id: string) => string | null
  splitTab: (id: string, direction: SplitDirection) => void
  togglePaneZoom: (id: string) => void
  breakOutPane: (id: string) => void
  ungroupPanes: (id: string) => void
  /** Closes one pane, or the tab of the bar it is the only pane of. */
  handleRemoveTab: (id: string) => void
  /** Closes a tab of the bar with all its panes. */
  handleCloseTab: (id: string) => void
  handleCloseOtherTabs: (id: string) => void
  handleCloseTabsToRight: (id: string) => void
  handleCloseTabsToLeft: (id: string) => void
  updateTab: (id: string, updater: (tab: Tab) => Tab) => void
  renameTab: (id: string, newName: string) => void
  editTabProfile: (tab: Tab) => void
  setTabLogRecording: (tabId: string, recording: boolean) => void
  setTabIconStyle: (tabId: string, iconStyle: TabIconStyle | undefined) => void
}

export function useTabContextMenu({
  handleNewTab,
  duplicateTab,
  splitTab,
  togglePaneZoom,
  breakOutPane,
  ungroupPanes,
  handleRemoveTab,
  handleCloseTab,
  handleCloseOtherTabs,
  handleCloseTabsToRight,
  handleCloseTabsToLeft,
  updateTab,
  renameTab,
  editTabProfile,
  setTabLogRecording,
  setTabIconStyle,
}: UseTabContextMenuDeps) {
  const [renameDialogState, setRenameDialogState] = useState<RenameDialogState>({
    isOpen: false,
    tabId: null,
    currentName: "",
  })

  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    visible: false,
    x: 0,
    y: 0,
    tab: null,
    actions: [],
  })

  /** Opens the dialog that renames a tab, and picks a terminal tab's icon. */
  const openRenameDialog = useCallback((tab: Tab) => {
    setRenameDialogState({
      isOpen: true,
      tabId: tab.id,
      currentName: tab.title,
      iconTab: hasTabIcon(tab) ? tab : undefined,
      localTab: tab.type === "terminal" ? tab : undefined,
    })
  }, [])

  const handleTabContextMenu = useCallback(
    (event: React.MouseEvent, tab: Tab, actions: TabContextMenuAction[]) => {
      event.preventDefault()
      setContextMenu({
        visible: true,
        x: event.clientX,
        y: event.clientY,
        tab,
        actions,
      })
    },
    []
  )

  const handleContextMenuAction = useCallback(
    (action: string) => {
      if (!contextMenu.tab) {
        return
      }

      const tab = contextMenu.tab

      switch (action) {
        case "new":
          handleNewTab()
          break
        case "duplicate":
          duplicateTab(tab.id)
          break
        case "split-right":
          splitTab(tab.id, "right")
          break
        case "split-left":
          splitTab(tab.id, "left")
          break
        case "split-down":
          splitTab(tab.id, "below")
          break
        case "split-above":
          splitTab(tab.id, "above")
          break
        case "toggle-zoom":
          togglePaneZoom(tab.id)
          break
        case "move-to-new-tab":
          breakOutPane(tab.id)
          break
        case "ungroup":
          ungroupPanes(tab.id)
          break
        case "rename":
          openRenameDialog(tab)
          break
        case "edit-connection":
          editTabProfile(tab)
          break
        case "pin-header":
          updateTab(tab.id, (currentTab) => ({
            ...currentTab,
            connectionHeaderPinned: true,
          }))
          break
        case "unpin-header":
          updateTab(tab.id, (currentTab) => ({
            ...currentTab,
            connectionHeaderPinned: false,
          }))
          break
        case "log-start":
          setTabLogRecording(tab.id, true)
          break
        case "log-stop":
          setTabLogRecording(tab.id, false)
          break
        case "view-logs":
          showLogsForTab({
            tabId: tab.id,
            sessionType: tab.type === "ssh" ? "ssh" : "local",
            host: tab.connection?.host,
            port: tab.connection?.port,
            username: tab.connection?.username,
          })
          break
        case "close":
          handleRemoveTab(tab.id)
          break
        case "close-tab":
          handleCloseTab(tab.id)
          break
        case "close-others":
          handleCloseOtherTabs(tab.id)
          break
        case "close-right":
          handleCloseTabsToRight(tab.id)
          break
        case "close-left":
          handleCloseTabsToLeft(tab.id)
          break
        default:
          break
      }

      setContextMenu((prev) => ({ ...prev, visible: false }))
    },
    [
      contextMenu.tab,
      handleNewTab,
      duplicateTab,
      splitTab,
      togglePaneZoom,
      breakOutPane,
      ungroupPanes,
      openRenameDialog,
      handleRemoveTab,
      handleCloseTab,
      handleCloseOtherTabs,
      handleCloseTabsToRight,
      handleCloseTabsToLeft,
      updateTab,
      editTabProfile,
      setTabLogRecording,
    ]
  )

  const handleCloseContextMenu = useCallback(() => {
    setContextMenu((prev) => ({ ...prev, visible: false }))
  }, [])

  const handleRenameConfirm = useCallback(
    (newName: string) => {
      if (renameDialogState.tabId) {
        renameTab(renameDialogState.tabId, newName)
      }
    },
    [renameDialogState.tabId, renameTab]
  )

  const handleRenameClose = useCallback(() => {
    setRenameDialogState({
      isOpen: false,
      tabId: null,
      currentName: "",
    })
  }, [])

  const { iconTab, localTab } = renameDialogState
  const handleUseAutoTitle = useCallback(() => {
    if (!localTab) return
    void nameLocalTab({ ...localTab, title: "" }).then(({ title }) =>
      updateTab(localTab.id, (current) => ({
        ...current,
        title,
        autoTitle: true,
        duplicateBaseTitle: undefined,
      }))
    )
  }, [localTab, updateTab])

  const handleIconStyleChange = useCallback(
    (iconStyle: TabIconStyle | undefined) => {
      if (iconTab) setTabIconStyle(iconTab.id, iconStyle)
    },
    [iconTab, setTabIconStyle]
  )

  return {
    contextMenu,
    renameDialog: {
      isOpen: renameDialogState.isOpen,
      currentName: renameDialogState.currentName,
      onConfirm: handleRenameConfirm,
      onClose: handleRenameClose,
      onUseAutoTitle: localTab && !localTab.autoTitle ? handleUseAutoTitle : undefined,
      icon: iconTab && {
        tab: iconTab,
        value: iconTab.iconStyle,
        onChange: handleIconStyleChange,
      },
    },
    openRenameDialog,
    handleTabContextMenu,
    handleContextMenuAction,
    handleCloseContextMenu,
  }
}
