import React, { useState, useCallback, useEffect, useRef } from "react"
import { useTranslation } from "react-i18next"
import { platform } from "@tauri-apps/plugin-os"
import { Plus, Settings } from "lucide-react"
import { ContextMenu } from "@/components/ContextMenu"
import { WindowControls } from "@/components/WindowControls"
import { ConnectionDialog } from "@/components/ConnectionDialog"
import { RenameDialog } from "@/components/RenameDialog"
import { ThemeSwitcher } from "@/components/ThemeSwitcher"
import { LanguageSwitcher } from "@/components/LanguageSwitcher"
import { TabBar } from "@/components/TabBar"
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

export const TTermApp: React.FC = () => {
  const { t, i18n } = useTranslation()
  const [os] = useState<string>(() => platform())
  const [showConnectionDialog, setShowConnectionDialog] = useState(false)
  const [showThemeSwitcher, setShowThemeSwitcher] = useState(false)
  const [showLanguageSwitcher, setShowLanguageSwitcher] = useState(false)
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
  } = useTabs()

  const { saveSession, loadSession, clearSession } = useSessionPersistence()
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
        addTab({
          title: "Terminal",
          type: "terminal",
          isModified: false,
        })
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
      addTab(connection)
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
        // Handle settings menu actions
        switch (action) {
          case "change-theme":
            setShowThemeSwitcher(true)
            break
          case "change-language":
            setShowLanguageSwitcher(true)
            break
          case "clear-session":
            await clearSession()
            // Reload page to reset state
            window.location.reload()
            break
          case "about":
            alert(
              `${t("app.title")} - ${t("app.subtitle")}\n${t("app.version")}\n${t("app.builtWith")}`
            )
            break
          default:
            break
        }
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
      clearSession,
      t,
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

  // Handle settings button click
  const handleSettingsClick = useCallback(() => {
    const actions: TabContextMenuAction[] = [
      { label: t("settings.changeTheme"), action: "change-theme", icon: "palette" },
      { label: t("settings.changeLanguage"), action: "change-language", icon: "languages" },
      { separator: true, label: "", action: "" },
      { label: t("settings.clearSession"), action: "clear-session", icon: "x" },
      { separator: true, label: "", action: "" },
      { label: t("settings.about"), action: "about" },
    ]

    // Show context menu near settings button
    const rect = settingsButtonRef.current?.getBoundingClientRect()
    const x = rect ? rect.right - 200 : window.innerWidth - 200
    const y = rect ? rect.bottom + 4 : 50
    setContextMenu({
      visible: true,
      x,
      y,
      tab: null,
      actions,
    })
  }, [t])

  const renderTabContent = () => {
    if (!activeTabId) {
      return (
        <div className="terminal-placeholder">
          <h3>{t("welcome.title")}</h3>
          <p>{t("welcome.description")}</p>
          <button onClick={handleNewTab} className="btn-primary" style={{ marginTop: "16px" }}>
            {t("welcome.newConnection")}
          </button>
        </div>
      )
    }

    const activeTab = tabs.find((t) => t.id === activeTabId)
    if (!activeTab) return null

    // This is where tab content will be rendered
    // You can render different content based on activeTab.type
    // e.g., terminal emulator, SSH client, SFTP browser, etc.
    return (
      <div className="tab-content-container">
        {/* Future terminal/SSH/SFTP content will be rendered here */}
        <div className="tab-content-placeholder">
          <div className="tab-info">
            <h3>{activeTab.title}</h3>
            <p>
              {t("tabContent.type")}: {activeTab.type.toUpperCase()}
            </p>
            {activeTab.connection && (
              <p>
                {activeTab.connection.host}:{activeTab.connection.port}
                {activeTab.connection.username && ` (${activeTab.connection.username})`}
              </p>
            )}
          </div>
          <div className="tab-hint">
            <p>{t("tabContent.hint", { type: activeTab.type })}</p>
            <p>{t("tabContent.implementation")}</p>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className={`app ${os === "macos" ? "macos" : ""}`}>
      {/* Combined Title Bar and Tab Bar */}
      <div className="title-bar" data-tauri-drag-region>
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
            </div>
          </div>
        </div>

        {/* Draggable space (10% of window width) */}
        <div className="drag-space" data-tauri-drag-region></div>

        {/* Settings and window controls */}
        <div className="title-bar-right">
          <button
            ref={settingsButtonRef}
            className="tab-action settings-button"
            onClick={handleSettingsClick}
            title={t("settings.title")}
          >
            <Settings size={16} />
          </button>
          <WindowControls />
        </div>
      </div>

      {/* Content Area */}
      <div className="content-area">{renderTabContent()}</div>

      {/* Connection Dialog */}
      <ConnectionDialog
        isOpen={showConnectionDialog}
        onClose={() => setShowConnectionDialog(false)}
        onConnect={handleConnect}
      />

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

      {/* Theme Switcher */}
      {showThemeSwitcher && <ThemeSwitcher onClose={() => setShowThemeSwitcher(false)} />}

      {/* Language Switcher */}
      {showLanguageSwitcher && <LanguageSwitcher onClose={() => setShowLanguageSwitcher(false)} />}

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
