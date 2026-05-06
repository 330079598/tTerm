import React from "react"

import { TerminalTab } from "@/components/TerminalTab"
import { Tab } from "@/types/tab"

interface TabPanelsProps {
  activeTabId: string | null
  handlePinConnectionHeader: (tabId: string) => void
  handleReconnectTab: (tabId: string) => void
  handleUnpinConnectionHeader: (tabId: string) => void
  tabs: Tab[]
}

export const TabPanels: React.FC<TabPanelsProps> = ({
  activeTabId,
  handlePinConnectionHeader,
  handleReconnectTab,
  handleUnpinConnectionHeader,
  tabs,
}) => {
  return (
    <>
      {tabs.map((tab) => (
        <div
          key={`${tab.id}:${tab.sessionNonce ?? 0}`}
          style={{
            position: "absolute",
            inset: 0,
            width: "100%",
            height: "100%",
            display: "flex",
            flexDirection: "column",
            visibility: tab.id === activeTabId ? "visible" : "hidden",
            pointerEvents: tab.id === activeTabId ? "auto" : "none",
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
