import "@/components/TTermApp.css"
import React, { useCallback, useEffect, useRef, useState } from "react"
import { TerminalTab } from "@/components/TerminalTab"
import { useTranslation } from "react-i18next"
import { platform } from "@tauri-apps/plugin-os"
import { BookMarked, Plus, Settings } from "lucide-react"
import { ContextMenu } from "@/components/ContextMenu"
import { ConnectionDialog } from "@/components/ConnectionDialog"
import { RenameDialog } from "@/components/RenameDialog"
import { SettingsDialog } from "@/components/SettingsDialog"
import { TabBar } from "@/components/TabBar"
import { ProfilesPanel, SavedProfile } from "@/components/ProfilesPanel"
import { useTabs } from "@/hooks/useTabs"
import { useSessionPersistence } from "@/hooks/useSessionPersistence"
import { useConnectionManager } from "@/hooks/useConnectionManager"
import { useConfig } from "@/contexts/ConfigContext"
import { setTheme, type Theme } from "@/lib/utils"
import { Tab, TabContextMenuAction } from "@/types/tab"

interface ContextMenuState {
  visible: boolean
  x: number
  y: number
  tab: Tab | null
  actions: TabContextMenuAction[]
}

const DEFAULT_CONNECTION_HEADER_PINNED = true

function buildTabFromConnection(
  connection: Omit<Tab, "id" | "isActive">
): Omit<Tab, "id" | "isActive"> {
  return {
    ...connection,
    sessionNonce: connection.sessionNonce ?? 0,
    connectionHeaderPinned: connection.connectionHeaderPinned ?? DEFAULT_CONNECTION_HEADER_PINNED,
  }
}

export const TTermApp: React.FC = () => {
  const { t, i18n } = useTranslation()
  const [os] = useState<string>(() => platform())
  const [showConnectionDialog, setShowConnectionDialog] = useState(false)
  const [showProfilesPanel, setShowProfilesPanel] = useState(false)
  const [showSettings, setShowSettings] = useState(false)
  const [editingProfile, setEditingProfile] = useState<SavedProfile | null>(null)
  const [profilesRefreshKey, setProfilesRefreshKey] = useState(0)
  const [renameDialogState, setRenameDialogState] = useState<{
    isOpen: boolean
    tabId: string | null
    currentName: string
  }>({
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

  // Load config and apply theme/language on mount
  useEffect(() => {
    if (isLoaded) {
      setTheme(config.theme as Theme)
      i18n.changeLanguage(config.language)
    }
  }, [isLoaded, config, i18n])

  useEffect(() => {
    // Try to restore previous session
    const loadAndRestoreSession = async () => {
      const savedSession = await loadSession()
      if (savedSession && savedSession.tabs.length > 0) {
        restoreSession(savedSession.tabs, savedSession.activeTabId)
      } else {
        // If no saved session, create default tab
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

  // Save session state when tabs or active tab changes
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
    (e: React.MouseEvent, tab: Tab, actions: TabContextMenuAction[]) => {
      e.preventDefault()
      setContextMenu({
        visible: true,
        x: e.clientX,
        y: e.clientY,
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

  // Reserve space for native frame buttons on Windows/Linux.
  // Keep this in sync with tauri-plugin-frame `button_width(46)` and 3 controls.
  const nativeControlsReservePx = os === "macos" ? 0 : 46 * 3

  // Handle settings button click
  const handleSettingsClick = useCallback(() => {
    setShowSettings(true)
  }, [])

  const renderTabContent = () => {
    if (tabs.length === 0) {
      return (
        <div className="terminal-placeholder">
          <h3>{t("welcome.title")}</h3>
          <p>{t("welcome.description")}</p>
          <button onClick={handleNewTab} className="btn-primary" style={{ marginTop: "16px" }}>
            {t("welcome.newConnection")}
          </button>
          <div style={{ marginTop: "32px", width: "100%", maxWidth: 480 }}>
            <ProfilesPanel
              refreshKey={profilesRefreshKey}
              onConnect={(conn) => {
                handleConnect(conn)
              }}
              onEdit={(profile) => {
                setEditingProfile(profile)
                setShowConnectionDialog(true)
              }}
            />
          </div>
        </div>
      )
    }

    return (
      <>
        {tabs.map((tab) => (
          <div
            key={`${tab.id}:${tab.sessionNonce ?? 0}`}
            style={{
              width: "100%",
              height: "100%",
              visibility: tab.id === activeTabId ? "visible" : "hidden",
              position: tab.id === activeTabId ? "relative" : "absolute",
              display: "flex",
              flexDirection: "column",
            }}
          >
            <TerminalTab
              tabId={tab.id}
              sessionNonce={tab.sessionNonce}
              isActive={tab.id === activeTabId}
              connectionHeaderPinned={tab.connectionHeaderPinned}
              connection={tab.connection ?? { type: tab.type === "terminal" ? "terminal" : "ssh" }}
              onReconnectRequest={() => handleReconnectTab(tab.id)}
              onPinConnectionHeader={() => handlePinConnectionHeader(tab.id)}
              onUnpinConnectionHeader={() => handleUnpinConnectionHeader(tab.id)}
            />
          </div>
        ))}
      </>
    )
  }

  return (
    <div className={`app ${os === "macos" ? "macos" : ""}`}>
      {/* Combined Title Bar and Tab Bar */}
      <div className="title-bar">
        <div className="title-bar-left">
          {/* Tab Bar integrated into title bar */}
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

        {/* Draggable space */}
        <div className="drag-space" data-tauri-drag-region></div>

        {/* Settings and window controls */}
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

      {/* Content Area */}
      <div className="content-area">{renderTabContent()}</div>

      {/* Connection Dialog */}
      {showConnectionDialog && (
        <ConnectionDialog
          isOpen={showConnectionDialog}
          onClose={() => {
            setShowConnectionDialog(false)
            setEditingProfile(null)
            setProfilesRefreshKey((k) => k + 1)
          }}
          onConnect={handleConnect}
          editProfile={editingProfile}
        />
      )}

      {/* Profiles Panel */}
      {showProfilesPanel && (
        <div className="modal-overlay" onClick={() => setShowProfilesPanel(false)}>
          <div className="modal-content" onClick={(e) => e.stopPropagation()}>
            <ProfilesPanel
              refreshKey={profilesRefreshKey}
              onConnect={(conn) => {
                handleConnect(conn)
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

      {/* Context Menu */}
      {contextMenu.visible && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          actions={contextMenu.actions}
          onAction={handleContextMenuAction}
          onClose={handleCloseContextMenu}
        />
      )}

      {/* Settings Dialog */}
      {showSettings && <SettingsDialog onClose={() => setShowSettings(false)} />}

      {/* Rename Dialog */}
      <RenameDialog
        isOpen={renameDialogState.isOpen}
        currentName={renameDialogState.currentName}
        onConfirm={handleRenameConfirm}
        onClose={handleRenameClose}
      />
    </div>
  )
}
