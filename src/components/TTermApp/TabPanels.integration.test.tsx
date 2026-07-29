// @vitest-environment jsdom

import { useEffect, useRef, useState } from "react"
import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { TabPanels, type TabPanelsHandle } from "@/components/TTermApp/TabPanels"
import { useTabs } from "@/hooks/useTabs"
import { getTabIdsForCloseAction } from "@/lib/tabClosing"
import type { Tab } from "@/types/tab"

vi.mock("@/components/TerminalTab", () => ({
  TerminalTab: ({ tabId }: { tabId: string }) => <div data-testid={`terminal-${tabId}`} />,
}))

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
  }),
}))

class TestResizeObserver implements ResizeObserver {
  disconnect() {}
  observe() {}
  unobserve() {}
}

vi.stubGlobal("ResizeObserver", TestResizeObserver)
vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
  callback(0)
  return 0
})
vi.stubGlobal("cancelAnimationFrame", () => {})

const initialTabs: Tab[] = ["left-active", "left-other", "context", "outside"].map((id, index) => ({
  id,
  title: id,
  type: "terminal",
  isActive: index === 0,
  hasConnected: true,
}))

function WorkspaceHarness() {
  const workspaceRef = useRef<TabPanelsHandle>(null)
  const { tabs, activeTabId, duplicateTab, removeTabs, restoreSession, setActiveTab, updateTab } =
    useTabs()
  const [contextGroupIds, setContextGroupIds] = useState<string[]>([])
  const [outsideGroupIds, setOutsideGroupIds] = useState<string[]>([])
  const [dockActiveTabId, setDockActiveTabId] = useState<string | null>(null)

  useEffect(() => {
    restoreSession(initialTabs, "left-active")
  }, [restoreSession])

  if (tabs.length === 0) {
    return null
  }

  const inspectGroups = () => {
    setContextGroupIds(workspaceRef.current?.getGroupTabIds("context") ?? [])
    setOutsideGroupIds(workspaceRef.current?.getGroupTabIds("outside") ?? [])
    setDockActiveTabId(workspaceRef.current?.getActiveTabId() ?? null)
  }

  const closeContextTabsToLeft = () => {
    const groupTabIds = workspaceRef.current?.getGroupTabIds("context") ?? []
    const targetIds = getTabIdsForCloseAction(groupTabIds, "context", "close-left")
    if (activeTabId && targetIds.includes(activeTabId)) {
      workspaceRef.current?.activateTab("context")
    }
    removeTabs(targetIds, { preferredActiveTabId: "context" })
  }

  return (
    <div style={{ width: 800, height: 600 }}>
      <button type="button" onClick={() => workspaceRef.current?.splitTab("outside", "right")}>
        Split outside
      </button>
      <button
        type="button"
        onClick={() => {
          workspaceRef.current?.activateTab("left-active")
          setActiveTab("left-active")
        }}
      >
        Activate left
      </button>
      <button type="button" onClick={closeContextTabsToLeft}>
        Close context left
      </button>
      <button type="button" onClick={inspectGroups}>
        Inspect groups
      </button>
      <output data-testid="active-tab">{activeTabId}</output>
      <output data-testid="dock-active-tab">{dockActiveTabId}</output>
      <output data-testid="tab-ids">{tabs.map((tab) => tab.id).join(",")}</output>
      <output data-testid="context-group">{contextGroupIds.join(",")}</output>
      <output data-testid="outside-group">{outsideGroupIds.join(",")}</output>
      <TabPanels
        ref={workspaceRef}
        activeTabId={activeTabId}
        broadcastSourceTabId={null}
        duplicateTab={duplicateTab}
        handlePinConnectionHeader={() => {}}
        handleReconnectTab={() => {}}
        handleServerMonitorVisibilityChange={() => {}}
        handleUnpinConnectionHeader={() => {}}
        initialLayout={null}
        liveBroadcastState="idle"
        onActiveTabChange={setActiveTab}
        onConnectProfile={() => {}}
        onEditProfile={() => {}}
        onLayoutChange={() => {}}
        onOpenRemoteFile={() => {}}
        onPauseBroadcast={() => {}}
        onResumeBroadcast={() => {}}
        onStopBroadcast={() => {}}
        onTabClose={() => {}}
        onTabContextMenu={() => {}}
        onTerminalConnectionStateChange={() => {}}
        onTerminalInput={async () => {}}
        onTerminalSavedPasswordPromptChange={() => {}}
        onTerminalSessionUnavailable={() => {}}
        onTerminalSensitivePrompt={() => {}}
        startupConnectionsReady
        startupSessionRestoreMode="all"
        tabs={tabs}
        updateTab={updateTab}
      />
    </div>
  )
}

afterEach(() => {
  vi.clearAllMocks()
})

describe("TabPanels grouped closing", () => {
  it("closes the active tabs to the left in one group without affecting another group", async () => {
    render(<WorkspaceHarness />)

    await screen.findByTestId("terminal-outside")
    fireEvent.click(screen.getByRole("button", { name: "Split outside" }))
    fireEvent.click(screen.getByRole("button", { name: "Inspect groups" }))

    await waitFor(() => {
      expect(screen.getByTestId("context-group").textContent).toBe("left-active,left-other,context")
      expect(screen.getByTestId("outside-group").textContent).toBe("outside")
    })

    fireEvent.click(screen.getByRole("button", { name: "Activate left" }))
    fireEvent.click(screen.getByRole("button", { name: "Inspect groups" }))
    expect(screen.getByTestId("active-tab").textContent).toBe("left-active")
    expect(screen.getByTestId("dock-active-tab").textContent).toBe("left-active")

    fireEvent.click(screen.getByRole("button", { name: "Close context left" }))

    await waitFor(() => {
      expect(screen.getByTestId("active-tab").textContent).toBe("context")
      expect(screen.getByTestId("tab-ids").textContent).toBe("context,outside")
    })

    fireEvent.click(screen.getByRole("button", { name: "Inspect groups" }))
    await waitFor(() => {
      expect(screen.getByTestId("context-group").textContent).toBe("context")
      expect(screen.getByTestId("outside-group").textContent).toBe("outside")
      expect(screen.getByTestId("dock-active-tab").textContent).toBe("context")
    })
  })
})
