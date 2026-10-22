// @vitest-environment jsdom

import { useEffect } from "react"
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { TabPanels } from "@/components/TTermApp/TabPanels"
import { useTabs } from "@/hooks/useTabs"
import { getPaneIds } from "@/lib/tabGroups"
import type { Tab } from "@/types/tab"

const mounts = vi.hoisted(() => ({ count: new Map<string, number>() }))

vi.mock("@/components/TerminalTab", async () => {
  const { useEffect: useMountEffect } = await import("react")
  return {
    TerminalTab: ({ tabId, isActive }: { tabId: string; isActive: boolean }) => {
      useMountEffect(() => {
        mounts.count.set(tabId, (mounts.count.get(tabId) ?? 0) + 1)
      }, [tabId])
      return <div data-testid={`terminal-${tabId}`} data-visible={isActive || undefined} />
    },
  }
})

// jsdom lays nothing out, so pane shortcuts have no positions to work with here.
vi.mock("@/contexts/KeymapContext", () => ({
  useKeymap: () => ({ registerHandler: () => () => {} }),
  useShortcutLabel: () => () => undefined,
}))

vi.mock("@/components/TabAttentionDot", () => ({ TabAttentionDot: () => null }))
vi.mock("@/components/TabLogIndicator", () => ({ TabLogIndicator: () => null }))
vi.mock("@/components/TabIcon", () => ({ TabIcon: () => null }))
vi.mock("@/components/TabTooltip", () => ({
  TabTooltip: ({ children }: { children: React.ReactNode }) => children,
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

const initialTabs: Tab[] = ["tab-1", "tab-2"].map((id, index) => ({
  id,
  title: id,
  type: "terminal",
  isActive: index === 0,
  hasConnected: true,
}))

let api: ReturnType<typeof useTabs>

function WorkspaceHarness() {
  const tabsApi = useTabs()
  const { restoreSession } = tabsApi

  useEffect(() => {
    api = tabsApi
  })

  useEffect(() => {
    restoreSession(initialTabs, "tab-1")
  }, [restoreSession])

  if (tabsApi.tabs.length === 0) {
    return null
  }

  return (
    <TabPanels
      activeTabId={tabsApi.activeTabId}
      groups={tabsApi.groups}
      broadcastSourceTabId={null}
      duplicateTab={tabsApi.duplicateTab}
      mergeGroup={tabsApi.mergeGroup}
      placePane={tabsApi.placePane}
      breakOutPane={tabsApi.breakOutPane}
      ungroupPanes={tabsApi.ungroupPanes}
      togglePaneZoom={tabsApi.togglePaneZoom}
      resizeSplit={tabsApi.resizeSplit}
      handlePinConnectionHeader={() => {}}
      handleReconnectTab={() => {}}
      handleServerMonitorVisibilityChange={() => {}}
      handleUnpinConnectionHeader={() => {}}
      liveBroadcastState="idle"
      onActiveTabChange={tabsApi.setActiveTab}
      onConnectProfile={() => {}}
      onEditProfile={() => {}}
      onOpenRemoteFile={() => {}}
      onPauseBroadcast={() => {}}
      onResumeBroadcast={() => {}}
      onStopBroadcast={() => {}}
      onTabClose={(id) => tabsApi.removeTabs([id])}
      onTabContextMenu={() => {}}
      onTerminalConnectionStateChange={() => {}}
      onTerminalInput={async () => {}}
      onTerminalSavedPasswordPromptChange={() => {}}
      onTerminalSessionUnavailable={() => {}}
      onTerminalSensitivePrompt={() => {}}
      startupConnectionsReady
      startupSessionRestoreMode="all"
      tabs={tabsApi.tabs}
      updateTab={tabsApi.updateTab}
    />
  )
}

const pane = (id: string) => screen.getByTestId(`terminal-${id}`).closest(".workspace-pane")
const isPaneVisible = (id: string) => !pane(id)?.hasAttribute("data-hidden")
const groupPanes = () => api.groups.map((group) => getPaneIds(group.root))

afterEach(() => {
  cleanup()
  mounts.count.clear()
  vi.clearAllMocks()
})

describe("TabPanels split tabs", () => {
  it("opens a split's copy as a pane of the same tab, both in sight with headers", async () => {
    render(<WorkspaceHarness />)
    await screen.findByTestId("terminal-tab-2")
    expect(document.querySelectorAll(".pane-header")).toHaveLength(0)

    act(() => {
      api.duplicateTab("tab-1", "right")
    })

    expect(groupPanes()).toEqual([["tab-1", "tab-3"], ["tab-2"]])
    expect(api.activeTabId).toBe("tab-3")
    expect(isPaneVisible("tab-1")).toBe(true)
    expect(isPaneVisible("tab-3")).toBe(true)
    expect(isPaneVisible("tab-2")).toBe(false)
    expect(document.querySelectorAll(".pane-header")).toHaveLength(2)
  })

  it("keeps every terminal mounted while switching tabs and moving panes between them", async () => {
    render(<WorkspaceHarness />)
    await screen.findByTestId("terminal-tab-2")
    act(() => {
      api.duplicateTab("tab-1", "below")
    })
    act(() => {
      api.setActiveTab("tab-2")
    })
    expect(isPaneVisible("tab-2")).toBe(true)
    expect(isPaneVisible("tab-1")).toBe(false)

    act(() => {
      api.mergeGroup("tab-2", "tab-3", "right")
    })
    expect(groupPanes()).toEqual([["tab-1", "tab-3", "tab-2"]])
    act(() => {
      api.breakOutPane("tab-3", 0)
    })
    expect(groupPanes()).toEqual([["tab-3"], ["tab-1", "tab-2"]])

    expect([...mounts.count.values()].every((count) => count === 1)).toBe(true)
  })

  it("hides the other panes behind a zoomed one, and closing a pane focuses its neighbour", async () => {
    render(<WorkspaceHarness />)
    await screen.findByTestId("terminal-tab-2")
    act(() => {
      api.duplicateTab("tab-1", "right")
    })
    act(() => {
      api.togglePaneZoom("tab-3")
    })
    expect(isPaneVisible("tab-3")).toBe(true)
    expect(isPaneVisible("tab-1")).toBe(false)

    fireEvent.click(screen.getAllByRole("button", { name: "workspace.closePane" })[0])
    expect(groupPanes()).toEqual([["tab-3"], ["tab-2"]])
    expect(api.activeTabId).toBe("tab-3")
    expect(document.querySelectorAll(".pane-header")).toHaveLength(0)
  })

  it("undoes the split from a pane's header, giving each pane a tab of its own", async () => {
    render(<WorkspaceHarness />)
    await screen.findByTestId("terminal-tab-2")
    act(() => {
      api.duplicateTab("tab-1", "right")
    })

    fireEvent.click(screen.getAllByRole("button", { name: "workspace.unsplit" })[0])
    expect(groupPanes()).toEqual([["tab-1"], ["tab-3"], ["tab-2"]])
    expect(api.activeTabId).toBe("tab-3")
    expect(document.querySelectorAll(".pane-header")).toHaveLength(0)
  })

  it("closes the last pane of a tab by moving to the tab that takes its place", async () => {
    render(<WorkspaceHarness />)
    await screen.findByTestId("terminal-tab-2")
    act(() => {
      api.removeTabs(["tab-1"])
    })
    expect(api.activeTabId).toBe("tab-2")
    expect(isPaneVisible("tab-2")).toBe(true)
  })
})
