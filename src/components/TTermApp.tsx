import "@/components/TTermApp.css"
import React, { useCallback, useEffect, useRef, useState } from "react"
import { useTranslation } from "react-i18next"
import { platform } from "@tauri-apps/plugin-os"
import { BookMarked, Plus, Settings } from "lucide-react"

import { ConnectionDialog } from "@/components/ConnectionDialog"
import { ContextMenu } from "@/components/ContextMenu"
import { ProfilesPanel, SavedProfile } from "@/components/ProfilesPanel"
import { RenameDialog } from "@/components/RenameDialog"
import { SettingsDialog } from "@/components/SettingsDialog"
import { TabBar } from "@/components/TabBar"
import { EmptyState } from "@/components/TTermApp/EmptyState"
import { TabPanels } from "@/components/TTermApp/TabPanels"
import type { ContextMenuState, RenameDialogState } from "@/components/TTermApp/types"
import { buildTabFromConnection } from "@/components/TTermApp/ttermAppUtils"
import { useConfig } from "@/contexts/ConfigContext"
import { useConnectionManager } from "@/hooks/useConnectionManager"
import { useSessionPersistence } from "@/hooks/useSessionPersistence"
import { useTabs } from "@/hooks/useTabs"
import { Tab, TabContextMenuAction } from "@/types/tab"

export const TTermApp: React.FC = () => {
  const { t, i18n } = useTranslation()
  const [os] = useState<string>(() => platform())
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

  useEffect(() => {
    if (isLoaded) {
      i18n.changeLanguage(config.language)
    }
  }, [isLoaded, config.language, i18n])

  useEffect(() => {
    const loadAndRestoreSession = async () => {
      const savedSession = await loadSession()
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
    }

    loadAndRestoreSession()
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

  const nativeControlsReservePx = os === "macos" ? 0 : 46 * 3

  const handleSettingsClick = useCallback(() => {
    setShowSettings(true)
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
    <div className={`app ${os === "macos" ? "macos" : ""}`}>
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
              <button className="tab-action" onClick={handleNewTab} title={t("tabs.newTab")}>
                <Plus size={16} />
              </button>
              <button
                className="tab-action"
                onClick={() => setShowProfilesPanel(true)}
                title={t("profiles.title")}
              >
                <BookMarked size={16} />
              </button>
            </div>
          </div>
        </div>

        <div className="drag-space" data-tauri-drag-region></div>

        <div className="title-bar-right" style={{ paddingRight: `${nativeControlsReservePx}px` }}>
          <button
            ref={settingsButtonRef}
            className="tab-action settings-button"
            onClick={handleSettingsClick}
            title={t("settings.title")}
          >
            <Settings size={16} />
          </button>
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

      {showProfilesPanel && (
        <div className="modal-overlay" onClick={() => setShowProfilesPanel(false)}>
          <div className="modal-content" onClick={(event) => event.stopPropagation()}>
            <ProfilesPanel
              refreshKey={profilesRefreshKey}
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
          </div>
        </div>
      )}

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
