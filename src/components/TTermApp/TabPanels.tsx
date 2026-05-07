import React from "react"

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
          startupSessionRestoreMode === "all" || isActive || tab.hasConnected === true

        return (
          <div
            key={shouldConnect ? `${tab.id}:${tab.sessionNonce ?? 0}` : tab.id}
            style={{
              position: "absolute",
              inset: 0,
              width: "100%",
              height: "100%",
              display: "flex",
              flexDirection: "column",
              visibility: isActive ? "visible" : "hidden",
              pointerEvents: isActive ? "auto" : "none",
            }}
          >
            {shouldConnect && (
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
            )}
          </div>
        )
      })}
    </>
  )
}
