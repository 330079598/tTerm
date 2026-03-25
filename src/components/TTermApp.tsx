import "@/components/TTermApp.css"
import React, { useState, useCallback, useEffect, useRef } from "react"
import { TerminalTab } from "@/components/TerminalTab"
import { useTranslation } from "react-i18next"
import { platform } from "@tauri-apps/plugin-os"
import { Plus, Settings } from "lucide-react"
import { ContextMenu } from "@/components/ContextMenu"
import { WindowControls } from "@/components/WindowControls"
import { ConnectionDialog } from "@/components/ConnectionDialog"
import { RenameDialog } from "@/components/RenameDialog"
import { ThemeSwitcher } from "@/components/ThemeSwitcher"
import { LanguageSwitcher } from "@/components/LanguageSwitcher"
import { FontSettings } from "@/components/FontSettings"
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
  const [showFontSettings, setShowFontSettings] = useState(false)
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
          case "font-settings":
            setShowFontSettings(true)
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
      { label: t("settings.fontSettings"), action: "font-settings", icon: "type" },
      { separator: true, label: "", action: "" },
      { label: t("settings.clearSession"), action: "clear-session", icon: "x" },
      { separator: true, label: "", action: "" },
      { label: t("settings.about"), action: "about" },
    ]

    // Show context menu near settings button (above, anchored to right of sidebar)
    const rect = settingsButtonRef.current?.getBoundingClientRect()
    const x = rect ? rect.left : 8
    const y = rect ? rect.top - 4 : window.innerHeight - 200
    setContextMenu({
      visible: true,
      x,
      y,
      tab: null,
      actions,
    })
  }, [t])

  const renderTabContent = () => {
    if (tabs.length === 0) {
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

    return (
      <>
        {tabs.map((tab) => (
          <div
            key={tab.id}
            style={{
              width: "100%",
              height: "100%",
              visibility: tab.id === activeTabId ? "visible" : "hidden",
              position: tab.id === activeTabId ? "relative" : "absolute",
              display: "flex",
              flexDirection: "column",
            }}
          >
            {tab.type === "terminal" || tab.type === "ssh" ? (
              <TerminalTab
                tabId={tab.id}
                isActive={tab.id === activeTabId}
                connection={tab.connection ?? { type: tab.type }}
              />
            ) : (
              <div className="tab-content-placeholder">
                <div className="tab-info">
                  <h3>{tab.title}</h3>
                  <p>
                    {t("tabContent.type")}: {tab.type.toUpperCase()}
                  </p>
                  {tab.connection && (
                    <p>
                      {tab.connection.host}:{tab.connection.port}
                      {tab.connection.username && ` (${tab.connection.username})`}
                    </p>
                  )}
                </div>
                <div className="tab-hint">
                  <p>{t("tabContent.hint", { type: tab.type })}</p>
                </div>
              </div>
            )}
          </div>
        ))}
      </>
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

        {/* Draggable space */}
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

      {/* Font Settings */}
      {showFontSettings && <FontSettings onClose={() => setShowFontSettings(false)} />}

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
