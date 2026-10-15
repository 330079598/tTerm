import "dockview-react/dist/styles/dockview.css"
import "@/components/TTermApp/WorkspaceDock.css"
import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from "react"
import { DockviewReact, type DockviewApi, type DockviewReadyEvent, themeDark } from "dockview-react"
import { Columns2, PanelsTopLeft, Rows2 } from "lucide-react"

import { WorkspacePanel } from "@/components/TTermApp/WorkspacePanel"
import {
  WorkspaceHeaderActions,
  WorkspaceWatermark,
} from "@/components/TTermApp/WorkspaceHeaderActions"
import type { WorkspaceDropPreview, PendingSplit } from "@/components/TTermApp/workspaceDrop"
import {
  WorkspaceContext,
  type TabPanelsProps,
  type WorkspaceContextValue,
  type WorkspaceSplitDirection,
} from "@/components/TTermApp/workspaceContext"
import { WorkspaceTab } from "@/components/TTermApp/WorkspaceTab"
import { isPageTab, type Tab } from "@/types/tab"
import { useDockInteractionClasses } from "@/components/TTermApp/useDockInteractionClasses"
import { usePaneKeymap } from "@/components/TTermApp/usePaneKeymap"
import { useWorkspaceDrop } from "@/components/TTermApp/useWorkspaceDrop"

const workspaceDockTheme = {
  ...themeDark,
  gap: 8,
}

export interface TabPanelsHandle {
  activateTab: (tabId: string) => void
  clearTabDrop: () => void
  commitTabDrop: (tabId: string, clientX: number, clientY: number) => boolean
  getActiveTabId: () => string | null
  getGroupTabIds: (tabId: string) => string[] | null
  getVisibleTerminalTabIds: () => string[]
  previewTabDrop: (tabId: string, clientX: number, clientY: number) => boolean
  splitTab: (tabId: string, direction: WorkspaceSplitDirection) => void
}

const dockComponents = { workspacePanel: WorkspacePanel }
const dockTabComponents = { workspaceTab: WorkspaceTab }

export const TabPanels = forwardRef<TabPanelsHandle, TabPanelsProps>(function TabPanels(
  {
    activeTabId,
    broadcastSourceTabId,
    duplicateTab,
    handlePinConnectionHeader,
    handleReconnectTab,
    handleServerMonitorVisibilityChange,
    handleUnpinConnectionHeader,
    initialLayout,
    liveBroadcastState,
    onActiveTabChange,
    onConnectProfile,
    onEditProfile,
    onLayoutChange,
    onOpenCommandLibrary,
    onOpenRemoteFile,
    onPauseBroadcast,
    onResumeBroadcast,
    onStopBroadcast,
    onTabClose,
    onTabContextMenu,
    onTerminalCommandExecuted,
    onTerminalAttention,
    onTerminalConnectionStateChange,
    onTerminalInput,
    onSaveCommand,
    onTerminalSavedPasswordPromptChange,
    onTerminalSessionUnavailable,
    onTerminalSensitivePrompt,
    onTerminalConnectionProgress,
    profilesRefreshKey,
    startupConnectionsReady,
    startupSessionRestoreMode,
    tabs,
    updateTab,
  },
  ref
) {
  const [dockApi, setDockApi] = useState<DockviewApi | null>(null)
  const [groupCount, setGroupCount] = useState(1)
  const [maximizedGroupId, setMaximizedGroupId] = useState<string | null>(null)
  const [dropPreview, setDropPreview] = useState<WorkspaceDropPreview | null>(null)
  const dockRootRef = useRef<HTMLDivElement | null>(null)
  const dropPreviewRef = useRef<WorkspaceDropPreview | null>(null)
  const pendingSplitRef = useRef<PendingSplit | null>(null)
  const tabsRef = useRef(tabs)
  const onActiveTabChangeRef = useRef(onActiveTabChange)
  const onLayoutChangeRef = useRef(onLayoutChange)
  const initialLayoutRef = useRef(initialLayout)
  const dockDisposablesRef = useRef<Array<{ dispose: () => void }>>([])

  useDockInteractionClasses(dockRootRef)

  const { splitTab, clearTabDrop, previewTabDrop, commitTabDrop } = useWorkspaceDrop({
    dockApi,
    tabsRef,
    duplicateTab,
    pendingSplitRef,
    dropPreviewRef,
    setDropPreview,
    dockRootRef,
  })

  useEffect(() => {
    tabsRef.current = tabs
    onActiveTabChangeRef.current = onActiveTabChange
    onLayoutChangeRef.current = onLayoutChange
  }, [onActiveTabChange, onLayoutChange, tabs])

  const addPanel = useCallback(
    (
      api: DockviewApi,
      tab: Tab,
      position?: { referencePanel: string; direction?: WorkspaceSplitDirection }
    ) => {
      api.addPanel({
        id: tab.id,
        component: "workspacePanel",
        tabComponent: "workspaceTab",
        title: tab.title,
        params: { tabId: tab.id },
        position,
        // Settings and other app pages are regular DOM content. Keeping them in Dockview's overlay
        // layer can leave stale pixels behind when WebView2 switches back to a WebGL-backed terminal.
        renderer: isPageTab(tab) ? "onlyWhenVisible" : "always",
        minimumWidth: 220,
        minimumHeight: 140,
      })
    },
    []
  )

  const collapseToSingleGroup = useCallback(
    (tabId: string) => {
      const targetPanel = dockApi?.getPanel(tabId)
      if (!dockApi || !targetPanel) {
        return
      }

      dockApi.exitMaximizedGroup()
      for (const panel of [...dockApi.panels]) {
        if (panel.group !== targetPanel.group) {
          panel.api.moveTo({ group: targetPanel.group, position: "center", skipSetActive: true })
        }
      }
      for (const group of [...dockApi.groups]) {
        if (group !== targetPanel.group && group.panels.length === 0) {
          dockApi.removeGroup(group)
        }
      }
      targetPanel.api.setActive()
    },
    [dockApi]
  )

  usePaneKeymap(dockApi)

  useImperativeHandle(
    ref,
    () => ({
      activateTab(tabId) {
        dockApi?.getPanel(tabId)?.api.setActive()
      },
      clearTabDrop,
      commitTabDrop,
      getActiveTabId() {
        return dockApi?.activePanel?.id ?? null
      },
      getGroupTabIds(tabId) {
        const panel = dockApi?.getPanel(tabId)
        return panel ? panel.group.panels.map((groupPanel) => groupPanel.id) : null
      },
      getVisibleTerminalTabIds() {
        if (!dockApi) return []
        return dockApi.panels
          .filter((panel) => {
            const tab = tabsRef.current.find((candidate) => candidate.id === panel.id)
            return panel.api.isVisible && (tab?.type === "terminal" || tab?.type === "ssh")
          })
          .map((panel) => panel.id)
      },
      previewTabDrop,
      splitTab,
    }),
    [clearTabDrop, commitTabDrop, dockApi, previewTabDrop, splitTab]
  )

  const handleReady = useCallback(
    ({ api }: DockviewReadyEvent) => {
      setDockApi(api)
      const validTabIds = new Set(tabsRef.current.map((tab) => tab.id))

      if (initialLayoutRef.current) {
        try {
          api.fromJSON(initialLayoutRef.current)
        } catch (error) {
          console.error("Failed to restore workspace layout:", error)
          api.clear()
        }
      }

      for (const panel of [...api.panels]) {
        if (!validTabIds.has(panel.id)) {
          api.removePanel(panel)
        }
      }

      let referencePanel = api.panels[api.panels.length - 1]?.id
      for (const tab of tabsRef.current) {
        if (!api.getPanel(tab.id)) {
          addPanel(api, tab, referencePanel ? { referencePanel } : undefined)
          referencePanel = tab.id
        }
      }
      setGroupCount(api.size)

      dockDisposablesRef.current.forEach((disposable) => disposable.dispose())
      dockDisposablesRef.current = [
        api.onDidActivePanelChange(({ panel }) => {
          if (panel) {
            onActiveTabChangeRef.current(panel.id)
          }
        }),
        api.onDidLayoutChange(() => {
          setGroupCount(api.size)
          onLayoutChangeRef.current(api.toJSON())
        }),
        api.onDidAddGroup(() => setGroupCount(api.size)),
        api.onDidRemoveGroup(() => setGroupCount(api.size)),
        api.onDidMaximizedGroupChange(({ group, isMaximized }) => {
          setMaximizedGroupId(isMaximized ? group.id : null)
        }),
      ]
      setMaximizedGroupId(api.panels.find((panel) => panel.api.isMaximized())?.group.id ?? null)

      if (activeTabId) {
        api.getPanel(activeTabId)?.api.setActive()
      }
      onLayoutChangeRef.current(api.toJSON())
    },
    [activeTabId, addPanel]
  )

  useEffect(
    () => () => {
      dockDisposablesRef.current.forEach((disposable) => disposable.dispose())
      dockDisposablesRef.current = []
    },
    []
  )

  useEffect(() => {
    if (!dockApi) {
      return
    }
    const tabIds = new Set(tabs.map((tab) => tab.id))
    for (const panel of [...dockApi.panels]) {
      if (!tabIds.has(panel.id)) {
        dockApi.removePanel(panel)
      }
    }

    let referencePanel = dockApi.activePanel?.id ?? dockApi.panels[dockApi.panels.length - 1]?.id
    for (const tab of tabs) {
      const panel = dockApi.getPanel(tab.id)
      if (panel) {
        if (panel.title !== tab.title) {
          panel.api.setTitle(tab.title)
        }
        continue
      }
      const pendingSplit = pendingSplitRef.current
      if (pendingSplit?.tabId === tab.id) {
        pendingSplitRef.current = null
        if (dockApi.getPanel(pendingSplit.sourceTabId)) {
          addPanel(dockApi, tab, {
            referencePanel: pendingSplit.sourceTabId,
            direction: pendingSplit.direction,
          })
          dockApi.getPanel(tab.id)?.api.setActive()
          referencePanel = tab.id
          continue
        }
      }
      addPanel(dockApi, tab, referencePanel ? { referencePanel } : undefined)
      referencePanel = tab.id
    }
  }, [addPanel, dockApi, tabs])

  useEffect(() => {
    if (!dockApi || !activeTabId || dockApi.activePanel?.id === activeTabId) {
      return
    }
    dockApi.getPanel(activeTabId)?.api.setActive()
  }, [activeTabId, dockApi])

  const contextValue = useMemo<WorkspaceContextValue>(
    () => ({
      broadcastSourceTabId,
      collapseToSingleGroup,
      duplicateTab,
      groupCount,
      handlePinConnectionHeader,
      handleReconnectTab,
      handleServerMonitorVisibilityChange,
      handleUnpinConnectionHeader,
      liveBroadcastState,
      maximizedGroupId,
      onConnectProfile,
      onEditProfile,
      onOpenCommandLibrary,
      onOpenRemoteFile,
      onPauseBroadcast,
      onResumeBroadcast,
      onStopBroadcast,
      onTabClose,
      onTabContextMenu,
      onTerminalCommandExecuted,
      onTerminalAttention,
      onTerminalConnectionStateChange,
      onTerminalInput,
      onSaveCommand,
      onTerminalSavedPasswordPromptChange,
      onTerminalSessionUnavailable,
      onTerminalSensitivePrompt,
      onTerminalConnectionProgress,
      profilesRefreshKey,
      splitTab,
      startupConnectionsReady,
      startupSessionRestoreMode,
      tabs,
      updateTab,
    }),
    [
      broadcastSourceTabId,
      collapseToSingleGroup,
      duplicateTab,
      groupCount,
      handlePinConnectionHeader,
      handleReconnectTab,
      handleServerMonitorVisibilityChange,
      handleUnpinConnectionHeader,
      liveBroadcastState,
      maximizedGroupId,
      onConnectProfile,
      onEditProfile,
      onOpenCommandLibrary,
      onOpenRemoteFile,
      onPauseBroadcast,
      onResumeBroadcast,
      onStopBroadcast,
      onTabClose,
      onTabContextMenu,
      onTerminalCommandExecuted,
      onTerminalAttention,
      onTerminalConnectionStateChange,
      onTerminalInput,
      onSaveCommand,
      onTerminalSavedPasswordPromptChange,
      onTerminalSessionUnavailable,
      onTerminalSensitivePrompt,
      onTerminalConnectionProgress,
      profilesRefreshKey,
      splitTab,
      startupConnectionsReady,
      startupSessionRestoreMode,
      tabs,
      updateTab,
    ]
  )

  return (
    <WorkspaceContext.Provider value={contextValue}>
      <div
        ref={dockRootRef}
        className={`workspace-dock dockview-theme-dark ${groupCount <= 1 ? "single-group" : "multi-group"} ${maximizedGroupId ? "is-maximized" : ""}`}
      >
        <DockviewReact
          components={dockComponents}
          tabComponents={dockTabComponents}
          theme={workspaceDockTheme}
          rightHeaderActionsComponent={WorkspaceHeaderActions}
          watermarkComponent={WorkspaceWatermark}
          defaultRenderer="always"
          dndStrategy="pointer"
          disableFloatingGroups
          noPanelsOverlay="watermark"
          onReady={handleReady}
        />
        {dropPreview && (
          <div
            className={`workspace-drop-preview ${dropPreview.direction}`}
            style={{
              top: dropPreview.top,
              left: dropPreview.left,
              width: dropPreview.width,
              height: dropPreview.height,
            }}
            aria-hidden="true"
          >
            <div className="workspace-drop-preview-surface">
              {dropPreview.direction === "center" ? (
                <PanelsTopLeft size={24} />
              ) : dropPreview.direction === "left" || dropPreview.direction === "right" ? (
                <Columns2 size={24} />
              ) : (
                <Rows2 size={24} />
              )}
            </div>
          </div>
        )}
      </div>
    </WorkspaceContext.Provider>
  )
})
