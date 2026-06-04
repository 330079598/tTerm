import React from "react"

import { ErrorBoundary } from "@/components/ErrorBoundary"
import { TerminalTab } from "@/components/TerminalTab"
import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"
import { Tab } from "@/types/tab"

const RemoteFileEditor = React.lazy(() =>
  import("@/components/RemoteFileEditor").then((module) => ({
    default: module.RemoteFileEditor,
  }))
)

const SettingsPanel = React.lazy(() =>
  import("@/components/SettingsDialog").then((module) => ({
    default: module.SettingsPanel,
  }))
)

interface TabPanelsProps {
  activeTabId: string | null
  handlePinConnectionHeader: (tabId: string) => void
  handleReconnectTab: (tabId: string) => void
  handleUnpinConnectionHeader: (tabId: string) => void
  onOpenRemoteFile: (
    entry: SftpDirectoryEntry,
    sourceTabId: string,
    connection?: Tab["connection"]
  ) => void
  startupSessionRestoreMode: "active" | "all"
  tabs: Tab[]
  updateTab: (id: string, updater: (tab: Tab) => Tab) => void
}

export const TabPanels: React.FC<TabPanelsProps> = ({
  activeTabId,
  handlePinConnectionHeader,
  handleReconnectTab,
  handleUnpinConnectionHeader,
  onOpenRemoteFile,
  startupSessionRestoreMode,
  tabs,
  updateTab,
}) => {
  return (
    <>
      {tabs.map((tab) => {
        const isActive = tab.id === activeTabId
        const shouldConnect =
          tab.type !== "settings" &&
          tab.type !== "remote-file-editor" &&
          (startupSessionRestoreMode === "all" || isActive || tab.hasConnected === true)

        return (
          <div
            key={shouldConnect ? `${tab.id}:${tab.sessionNonce ?? 0}` : tab.id}
            style={{
              position: "absolute",
              inset: 0,
              zIndex: isActive ? 1 : 0,
              width: "100%",
              height: "100%",
              display: isActive ? "flex" : "none",
              flexDirection: "column",
              overflow: "hidden",
              backgroundColor: "hsl(var(--background))",
              contain: "layout paint",
              isolation: "isolate",
              pointerEvents: isActive ? "auto" : "none",
            }}
          >
            {tab.type === "settings" ? (
              <ErrorBoundary resetKey={tab.id} scope="settings">
                <React.Suspense fallback={null}>
                  <SettingsPanel />
                </React.Suspense>
              </ErrorBoundary>
            ) : tab.type === "remote-file-editor" ? (
              <ErrorBoundary resetKey={tab.id} scope="remote-file-editor">
                <React.Suspense fallback={null}>
                  <RemoteFileEditor
                    tab={tab}
                    onTabUpdate={(updater) => updateTab(tab.id, updater)}
                  />
                </React.Suspense>
              </ErrorBoundary>
            ) : (
              shouldConnect && (
                <ErrorBoundary resetKey={`${tab.id}:${tab.sessionNonce ?? 0}`} scope="terminal-tab">
                  <TerminalTab
                    tabId={tab.id}
                    sessionNonce={tab.sessionNonce}
                    isActive={isActive}
                    connectionHeaderPinned={tab.connectionHeaderPinned}
                    connection={
                      tab.connection ?? { type: tab.type === "terminal" ? "terminal" : "ssh" }
                    }
                    onReconnectRequest={() => handleReconnectTab(tab.id)}
                    onOpenRemoteFile={onOpenRemoteFile}
                    onPinConnectionHeader={() => handlePinConnectionHeader(tab.id)}
                    onUnpinConnectionHeader={() => handleUnpinConnectionHeader(tab.id)}
                  />
                </ErrorBoundary>
              )
            )}
          </div>
        )
      })}
    </>
  )
}
