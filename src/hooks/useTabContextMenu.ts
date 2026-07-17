import { useCallback, useState } from "react"

import type { ContextMenuState, RenameDialogState } from "@/components/TTermApp/types"
import type { Tab, TabContextMenuAction } from "@/types/tab"

interface UseTabContextMenuDeps {
  handleNewTab: () => void
  duplicateTab: (id: string) => string | null
  splitTab: (id: string, direction: "right" | "below") => void
  handleRemoveTab: (id: string) => void
  handleCloseOtherTabs: (id: string) => void
  handleCloseTabsToRight: (id: string) => void
  handleCloseTabsToLeft: (id: string) => void
  updateTab: (id: string, updater: (tab: Tab) => Tab) => void
  renameTab: (id: string, newName: string) => void
  editTabProfile: (tab: Tab) => void
}

export function useTabContextMenu({
  handleNewTab,
  duplicateTab,
  splitTab,
  handleRemoveTab,
  handleCloseOtherTabs,
  handleCloseTabsToRight,
  handleCloseTabsToLeft,
  updateTab,
  renameTab,
  editTabProfile,
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
        case "split-down":
          splitTab(tab.id, "below")
          break
        case "rename":
          setRenameDialogState({
            isOpen: true,
            tabId: tab.id,
            currentName: tab.title,
          })
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
        case "close":
          handleRemoveTab(tab.id)
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
      handleRemoveTab,
      handleCloseOtherTabs,
      handleCloseTabsToRight,
      handleCloseTabsToLeft,
      updateTab,
      editTabProfile,
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

  return {
    contextMenu,
    renameDialogState,
    handleTabContextMenu,
    handleContextMenuAction,
    handleCloseContextMenu,
    handleRenameConfirm,
    handleRenameClose,
  }
}
