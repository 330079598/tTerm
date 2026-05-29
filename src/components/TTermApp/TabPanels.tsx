import React from "react"

import { ErrorBoundary } from "@/components/ErrorBoundary"
import { SettingsPanel } from "@/components/SettingsDialog"
import { TerminalTab } from "@/components/TerminalTab"
import { Tab } from "@/types/tab"

interface TabPanelsProps {
  activeTabId: string | null
  handlePinConnectionHeader: (tabId: string) => void
  handleReconnectTab: (tabId: string) => void
  handleUnpinConnectionHeader: (tabId: string) => void
  startupSessionRestoreMode: "active" | "all"
  tabs: Tab[]
}

export const TabPanels: React.FC<TabPanelsProps> = ({
  activeTabId,
  handlePinConnectionHeader,
  handleReconnectTab,
  handleUnpinConnectionHeader,
  startupSessionRestoreMode,
  tabs,
}) => {
  return (
    <>
      {tabs.map((tab) => {
        const isActive = tab.id === activeTabId
        const shouldConnect =
          tab.type !== "settings" &&
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
                <SettingsPanel />
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
