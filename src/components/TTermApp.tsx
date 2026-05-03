import "@/components/TTermApp.css"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { platform } from "@tauri-apps/plugin-os"
import { BookMarked, Minus, Plus, Settings, Square, X } from "lucide-react"
import React, { useCallback, useEffect, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { ConnectionDialog } from "@/components/ConnectionDialog"
import { ContextMenu } from "@/components/ContextMenu"
import { ProfilesPanel, SavedProfile } from "@/components/ProfilesPanel"
import { RenameDialog } from "@/components/RenameDialog"
import { SettingsDialog } from "@/components/SettingsDialog"
import { TabBar } from "@/components/TabBar"
import { TransferManager } from "@/components/TransferManager"
import { EmptyState } from "@/components/TTermApp/EmptyState"
import { TabPanels } from "@/components/TTermApp/TabPanels"
import { buildTabFromConnection } from "@/components/TTermApp/ttermAppUtils"
import type { ContextMenuState, RenameDialogState } from "@/components/TTermApp/types"
import { Dialog, DialogContent } from "@/components/ui/dialog"
import { useConfig } from "@/contexts/ConfigContext"
import { useTransferManager } from "@/contexts/TransferContext"
import { useConnectionManager } from "@/hooks/useConnectionManager"
import { useSessionPersistence } from "@/hooks/useSessionPersistence"
import { useTabs } from "@/hooks/useTabs"
import { markSessionReady } from "@/lib/startup"
import { Tab, TabContextMenuAction } from "@/types/tab"

export const TTermApp: React.FC = () => {
  const { t, i18n } = useTranslation()
  const [os] = useState<string>(() => platform())
  const isMacos = os === "macos"
  const isLinux = os === "linux"
  const isWindows = os === "windows"
  const [showConnectionDialog, setShowConnectionDialog] = useState(false)
  const [showProfilesPanel, setShowProfilesPanel] = useState(false)
  const [showSettings, setShowSettings] = useState(false)
  const [editingProfile, setEditingProfile] = useState<SavedProfile | null>(null)
  const [profilesRefreshKey, setProfilesRefreshKey] = useState(0)
  const [renameDialogState, setRenameDialogState] = useState<RenameDialogState>({
    isOpen: false,
    tabId: null,
    currentName: "",
  })
  const settingsButtonRef = useRef<HTMLButtonElement>(null)
  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    visible: false,
    x: 0,
    y: 0,
    tab: null,
    actions: [],
  })

  const {
    tabs,
    activeTabId,
    addTab,
    removeTab,
    setActiveTab,
    moveTab,
    duplicateTab,
    closeOtherTabs,
    closeTabsToRight,
    renameTab,
    restoreSession,
    updateTab,
  } = useTabs()

  const { saveSession, loadSession } = useSessionPersistence()
  const { cleanupConnection } = useConnectionManager()
  const { config, isLoaded } = useConfig()
  const { cancelTransfer, clearCompletedTransfers, removeTransfer, transfers } =
    useTransferManager()

  useEffect(() => {
    if (isLoaded) {
      i18n.changeLanguage(config.language)
    }
  }, [isLoaded, config.language, i18n])

  useEffect(() => {
    let cancelled = false

    const loadAndRestoreSession = async () => {
      try {
        const savedSession = await loadSession()
        if (cancelled) {
          return
        }

        if (savedSession && savedSession.tabs.length > 0) {
          restoreSession(savedSession.tabs, savedSession.activeTabId)
        } else {
          addTab(
            buildTabFromConnection({
              title: "Terminal",
              type: "terminal",
              isModified: false,
            })
          )
        }
      } finally {
        if (!cancelled) {
          markSessionReady()
        }
      }
    }

    void loadAndRestoreSession()

    return () => {
      cancelled = true
    }
  }, [addTab, loadSession, restoreSession])

  useEffect(() => {
    if (tabs.length > 0) {
      saveSession(tabs, activeTabId)
    }
  }, [tabs, activeTabId, saveSession])

  const handleNewTab = useCallback(() => {
    setShowConnectionDialog(true)
  }, [])

  const handleConnect = useCallback(
    (connection: Omit<Tab, "id" | "isActive">) => {
      addTab(buildTabFromConnection(connection))
    },
    [addTab]
  )

  const handleRemoveTab = useCallback(
    (id: string) => {
      cleanupConnection(id)
      removeTab(id)
    },
    [removeTab, cleanupConnection]
  )

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
    async (action: string) => {
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
        case "rename":
          setRenameDialogState({
            isOpen: true,
            tabId: tab.id,
            currentName: tab.title,
          })
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
          closeOtherTabs(tab.id)
          break
        case "close-right":
          closeTabsToRight(tab.id)
          break
        default:
          break
      }
    },
    [
      contextMenu.tab,
      handleNewTab,
      duplicateTab,
      handleRemoveTab,
      closeOtherTabs,
      closeTabsToRight,
      updateTab,
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

  const handleReconnectTab = useCallback(
    (tabId: string) => {
      updateTab(tabId, (tab) => ({
        ...tab,
        sessionNonce: (tab.sessionNonce ?? 0) + 1,
      }))
    },
    [updateTab]
  )

  const handlePinConnectionHeader = useCallback(
    (tabId: string) => {
      updateTab(tabId, (tab) => ({
        ...tab,
        connectionHeaderPinned: true,
      }))
    },
    [updateTab]
  )

  const handleUnpinConnectionHeader = useCallback(
    (tabId: string) => {
      updateTab(tabId, (tab) => ({
        ...tab,
        connectionHeaderPinned: false,
      }))
    },
    [updateTab]
  )

  const nativeControlsReservePx = isWindows ? 46 * 3 : 0

  const handleSettingsClick = useCallback(() => {
    setShowSettings(true)
  }, [])

  const handleMinimizeWindow = useCallback(() => {
    void getCurrentWindow().minimize()
  }, [])

  const handleToggleMaximizeWindow = useCallback(() => {
    void getCurrentWindow().toggleMaximize()
  }, [])

  const handleCloseWindow = useCallback(() => {
    void getCurrentWindow().close()
  }, [])

  const renderTabContent = () => {
    if (tabs.length === 0) {
      return (
        <EmptyState
          handleConnect={handleConnect}
          handleNewTab={handleNewTab}
          onEditProfile={(profile) => {
            setEditingProfile(profile)
            setShowConnectionDialog(true)
          }}
          profilesRefreshKey={profilesRefreshKey}
        />
      )
    }

    return (
      <TabPanels
        activeTabId={activeTabId}
        handlePinConnectionHeader={handlePinConnectionHeader}
        handleReconnectTab={handleReconnectTab}
        handleUnpinConnectionHeader={handleUnpinConnectionHeader}
        tabs={tabs}
      />
    )
  }

  return (
    <div className={`app ${isMacos ? "macos" : ""} ${isLinux ? "linux" : ""}`}>
      <div className="title-bar">
        <div className="title-bar-left">
          <div className="tab-list-container">
            <TabBar
              tabs={tabs}
              activeTabId={activeTabId}
              onTabClick={setActiveTab}
              onTabClose={handleRemoveTab}
              onNewTab={handleNewTab}
              onTabMove={moveTab}
              onContextMenu={handleTabContextMenu}
            />
            <div className="tab-add-button">
              <button className="tab-action" onClick={handleNewTab}>
                <Plus size={16} />
              </button>
              <button className="tab-action" onClick={() => setShowProfilesPanel(true)}>
                <BookMarked size={16} />
              </button>
              <TransferManager
                transfers={transfers}
                onCancel={cancelTransfer}
                onRemove={removeTransfer}
                onClearCompleted={clearCompletedTransfers}
              />
            </div>
          </div>
        </div>

        <div className="drag-space" data-tauri-drag-region></div>

        <div className="title-bar-right" style={{ paddingRight: `${nativeControlsReservePx}px` }}>
          <button
            ref={settingsButtonRef}
            className="tab-action settings-button"
            onClick={handleSettingsClick}
          >
            <Settings size={16} />
          </button>
          {isLinux && (
            <div className="window-controls" aria-label="Window controls">
              <button
                className="window-control-button"
                onClick={handleMinimizeWindow}
                aria-label={t("window.minimize", { defaultValue: "Minimize" })}
              >
                <Minus size={16} />
              </button>
              <button
                className="window-control-button"
                onClick={handleToggleMaximizeWindow}
                aria-label={t("window.maximize", { defaultValue: "Maximize" })}
              >
                <Square size={13} />
              </button>
              <button
                className="window-control-button close"
                onClick={handleCloseWindow}
                aria-label={t("window.close", { defaultValue: "Close" })}
              >
                <X size={16} />
              </button>
            </div>
          )}
        </div>
      </div>

      <div className="content-area">{renderTabContent()}</div>

      {showConnectionDialog && (
        <ConnectionDialog
          isOpen={showConnectionDialog}
          onClose={() => {
            setShowConnectionDialog(false)
            setEditingProfile(null)
            setProfilesRefreshKey((key) => key + 1)
          }}
          onConnect={handleConnect}
          editProfile={editingProfile}
        />
      )}

      <Dialog open={showProfilesPanel} onOpenChange={setShowProfilesPanel}>
        <DialogContent
          showCloseButton={false}
          className="flex h-[min(720px,85vh)] flex-col overflow-hidden p-0 sm:max-w-3xl"
        >
          <ProfilesPanel
            refreshKey={profilesRefreshKey}
            surface="panel"
            onClose={() => setShowProfilesPanel(false)}
            onCreate={() => {
              setEditingProfile(null)
              setShowConnectionDialog(true)
              setShowProfilesPanel(false)
            }}
            onConnect={(connection) => {
              handleConnect(connection)
              setShowProfilesPanel(false)
            }}
            onEdit={(profile) => {
              setEditingProfile(profile)
              setShowConnectionDialog(true)
              setShowProfilesPanel(false)
            }}
          />
        </DialogContent>
      </Dialog>

      {contextMenu.visible && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          actions={contextMenu.actions}
          onAction={handleContextMenuAction}
          onClose={handleCloseContextMenu}
        />
      )}

      {showSettings && <SettingsDialog onClose={() => setShowSettings(false)} />}

      <RenameDialog
        isOpen={renameDialogState.isOpen}
        currentName={renameDialogState.currentName}
        onConfirm={handleRenameConfirm}
        onClose={handleRenameClose}
      />
    </div>
  )
}
