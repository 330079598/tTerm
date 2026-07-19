import "dockview-react/dist/styles/dockview.css"
import "@/components/TTermApp/WorkspaceDock.css"

import React, {
  createContext,
  forwardRef,
  useCallback,
  useContext,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from "react"
import {
  DockviewReact,
  type DockviewApi,
  type DockviewReadyEvent,
  type IDockviewHeaderActionsProps,
  type IDockviewPanelHeaderProps,
  type IDockviewPanelProps,
  type SerializedDockview,
} from "dockview-react"
import { Columns2, Maximize2, Minimize2, PanelsTopLeft, Rows2, Settings, X } from "lucide-react"
import { useTranslation } from "react-i18next"

import { ErrorBoundary } from "@/components/ErrorBoundary"
import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"
import { TerminalTab } from "@/components/TerminalTab"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { SavedProfile, Tab, TabContextMenuAction } from "@/types/tab"

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

type WorkspacePanelParams = {
  tabId: string
}

export type WorkspaceSplitDirection = "right" | "below"

export interface TabPanelsHandle {
  activateTab: (tabId: string) => void
  clearTabDrop: () => void
  commitTabDrop: (tabId: string, clientX: number, clientY: number) => boolean
  getGroupTabIds: (tabId: string) => string[] | null
  previewTabDrop: (tabId: string, clientX: number, clientY: number) => boolean
  splitTab: (tabId: string, direction: WorkspaceSplitDirection) => void
}

type WorkspaceDropDirection = "left" | "right" | "above" | "below" | "center"

type WorkspaceDropPreview = {
  direction: WorkspaceDropDirection
  sourceTabId: string
  targetTabId: string
  top: number
  left: number
  width: number
  height: number
}

type PendingSplit = {
  direction: WorkspaceSplitDirection
  sourceTabId: string
  tabId: string
}

interface TabPanelsProps {
  activeTabId: string | null
  duplicateTab: (id: string) => string | null
  handlePinConnectionHeader: (tabId: string) => void
  handleReconnectTab: (tabId: string) => void
  handleServerMonitorVisibilityChange: (tabId: string, visible: boolean) => void
  handleUnpinConnectionHeader: (tabId: string) => void
  initialLayout: SerializedDockview | null
  onActiveTabChange: (tabId: string) => void
  onLayoutChange: (layout: SerializedDockview) => void
  onOpenRemoteFile: (
    entry: SftpDirectoryEntry,
    sourceTabId: string,
    connection?: Tab["connection"]
  ) => void
  onConnectProfile: (connection: Omit<Tab, "id" | "isActive">) => void
  onEditProfile: (profile: SavedProfile) => void
  onTabClose: (tabId: string) => void
  onTabContextMenu: (event: React.MouseEvent, tab: Tab, actions: TabContextMenuAction[]) => void
  profilesRefreshKey?: number
  startupConnectionsReady: boolean
  startupSessionRestoreMode: "active" | "all"
  tabs: Tab[]
  updateTab: (id: string, updater: (tab: Tab) => Tab) => void
}

type WorkspaceContextValue = Omit<
  TabPanelsProps,
  "activeTabId" | "initialLayout" | "onActiveTabChange" | "onLayoutChange"
> & {
  collapseToSingleGroup: (tabId: string) => void
  groupCount: number
  maximizedGroupId: string | null
  splitTab: (tabId: string, direction: WorkspaceSplitDirection) => void
}

const WorkspaceContext = createContext<WorkspaceContextValue | null>(null)

function useWorkspace() {
  const value = useContext(WorkspaceContext)
  if (!value) {
    throw new Error("Workspace components must be rendered inside TabPanels")
  }
  return value
}

function useDockPanelState(api: IDockviewPanelProps["api"]) {
  const [state, setState] = useState(() => ({
    hasBeenVisible: api.isVisible,
    isActive: api.isActive,
    isFocused: api.isFocused,
    isVisible: api.isVisible,
  }))

  useEffect(() => {
    const update = () => {
      setState((current) => ({
        hasBeenVisible: current.hasBeenVisible || api.isVisible,
        isActive: api.isActive,
        isFocused: api.isFocused,
        isVisible: api.isVisible,
      }))
    }
    const disposables = [
      api.onDidActiveChange(update),
      api.onDidFocusChange(update),
      api.onDidVisibilityChange(update),
    ]
    update()
    return () => disposables.forEach((disposable) => disposable.dispose())
  }, [api])

  return state
}

function getTabActions(
  tab: Tab,
  t: ReturnType<typeof useTranslation>["t"]
): TabContextMenuAction[] {
  const commonStart: TabContextMenuAction[] = [
    { label: t("contextMenu.newTab"), action: "new", icon: "plus" },
  ]
  const commonEnd: TabContextMenuAction[] = [
    { separator: true, label: "", action: "" },
    { label: t("contextMenu.closeTab"), action: "close", icon: "x" },
    { label: t("contextMenu.closeOtherTabs"), action: "close-others" },
    { label: t("contextMenu.closeTabsToLeft"), action: "close-left" },
    { label: t("contextMenu.closeTabsToRight"), action: "close-right" },
  ]

  if (tab.type === "settings") {
    return [...commonStart, ...commonEnd]
  }

  const pinAction: TabContextMenuAction | null =
    tab.type === "ssh"
      ? tab.connectionHeaderPinned === false
        ? { label: t("contextMenu.pinConnectionHeader"), action: "pin-header", icon: "pin" }
        : {
            label: t("contextMenu.unpinConnectionHeader"),
            action: "unpin-header",
            icon: "pin-off",
          }
      : null
  const editConnectionAction: TabContextMenuAction | null =
    tab.type === "ssh" && tab.connection?.profileId
      ? { label: t("contextMenu.editConnection"), action: "edit-connection", icon: "edit" }
      : null
  const canSplit = tab.type === "terminal" || tab.type === "ssh"

  return [
    ...commonStart,
    { label: t("contextMenu.duplicateTab"), action: "duplicate", icon: "copy" },
    ...(canSplit
      ? [
          {
            label: t("contextMenu.splitRight", { defaultValue: "Split Right" }),
            action: "split-right",
            icon: "split-right",
          },
          {
            label: t("contextMenu.splitDown", { defaultValue: "Split Down" }),
            action: "split-down",
            icon: "split-down",
          },
        ]
      : []),
    { label: t("contextMenu.renameTab"), action: "rename", icon: "edit" },
    ...(editConnectionAction ? [editConnectionAction] : []),
    ...(pinAction ? [pinAction] : []),
    ...commonEnd,
  ]
}

const WorkspaceTab: React.FC<IDockviewPanelHeaderProps<WorkspacePanelParams>> = ({
  api,
  params,
}) => {
  const { t } = useTranslation()
  const { onTabClose, onTabContextMenu, tabs } = useWorkspace()
  const { isVisible } = useDockPanelState(api)
  const tab = tabs.find((candidate) => candidate.id === params.tabId)

  if (!tab) {
    return null
  }

  return (
    <div
      className={`workspace-tab ${isVisible ? "active" : ""} ${tab.isModified ? "modified" : ""}`}
      role="tab"
      aria-selected={isVisible}
      data-allow-context-menu
      onClick={() => api.setActive()}
      onContextMenu={(event) => onTabContextMenu(event, tab, getTabActions(tab, t))}
    >
      {tab.type === "settings" && <Settings size={13} aria-hidden="true" />}
      <span className="workspace-tab-title">{tab.title}</span>
      <button
        type="button"
        className="workspace-tab-close"
        aria-label={t("contextMenu.closeTab", { defaultValue: "Close tab" })}
        onClick={(event) => {
          event.stopPropagation()
          onTabClose(tab.id)
        }}
      >
        <X size={12} aria-hidden="true" />
      </button>
    </div>
  )
}

const WorkspacePanel: React.FC<IDockviewPanelProps<WorkspacePanelParams>> = ({ api, params }) => {
  const workspace = useWorkspace()
  const { hasBeenVisible, isVisible } = useDockPanelState(api)
  const tab = workspace.tabs.find((candidate) => candidate.id === params.tabId)

  if (!tab) {
    return null
  }

  const isSshConnection = tab.connection?.type === "ssh" || tab.type === "ssh"
  const shouldConnect =
    tab.type !== "settings" &&
    tab.type !== "remote-file-editor" &&
    (workspace.startupConnectionsReady || !isSshConnection) &&
    (workspace.startupSessionRestoreMode === "all" ||
      isVisible ||
      hasBeenVisible ||
      tab.hasConnected === true)

  return (
    <div
      className="workspace-panel"
      data-focused={api.isFocused || undefined}
      data-workspace-panel-id={tab.id}
    >
      {tab.type === "settings" ? (
        <ErrorBoundary resetKey={tab.id} scope="settings">
          <React.Suspense fallback={null}>
            <SettingsPanel
              onConnectProfile={workspace.onConnectProfile}
              onEditProfile={workspace.onEditProfile}
              profilesRefreshKey={workspace.profilesRefreshKey}
            />
          </React.Suspense>
        </ErrorBoundary>
      ) : tab.type === "remote-file-editor" ? (
        <ErrorBoundary resetKey={tab.id} scope="remote-file-editor">
          <React.Suspense fallback={null}>
            <RemoteFileEditor
              tab={tab}
              onTabUpdate={(updater) => workspace.updateTab(tab.id, updater)}
            />
          </React.Suspense>
        </ErrorBoundary>
      ) : (
        shouldConnect && (
          <ErrorBoundary resetKey={`${tab.id}:${tab.sessionNonce ?? 0}`} scope="terminal-tab">
            <TerminalTab
              tabId={tab.id}
              sessionNonce={tab.sessionNonce}
              isActive={isVisible}
              connectionHeaderPinned={tab.connectionHeaderPinned}
              connection={tab.connection ?? { type: tab.type === "terminal" ? "terminal" : "ssh" }}
              onReconnectRequest={() => workspace.handleReconnectTab(tab.id)}
              onOpenRemoteFile={workspace.onOpenRemoteFile}
              onPinConnectionHeader={() => workspace.handlePinConnectionHeader(tab.id)}
              onServerMonitorVisibilityChange={(visible) =>
                workspace.handleServerMonitorVisibilityChange(tab.id, visible)
              }
              onUnpinConnectionHeader={() => workspace.handleUnpinConnectionHeader(tab.id)}
            />
          </ErrorBoundary>
        )
      )}
    </div>
  )
}

const WorkspaceHeaderActions: React.FC<IDockviewHeaderActionsProps> = ({
  activePanel,
  isGroupActive,
}) => {
  const { t } = useTranslation()
  const { collapseToSingleGroup, groupCount, maximizedGroupId, splitTab, tabs } = useWorkspace()
  const tab = activePanel ? tabs.find((candidate) => candidate.id === activePanel.id) : undefined
  const canSplit = tab?.type === "terminal" || tab?.type === "ssh"
  const isMaximized = activePanel?.group.id === maximizedGroupId

  const action = (label: string, icon: React.ReactNode, onClick: () => void, disabled = false) => (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          className="workspace-header-action"
          aria-label={label}
          disabled={disabled}
          onClick={onClick}
        >
          {icon}
        </button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  )

  return (
    <div className={`workspace-header-actions ${isGroupActive ? "active" : ""}`}>
      {action(
        t("workspace.splitRight", { defaultValue: "Split Right" }),
        <Columns2 size={14} aria-hidden="true" />,
        () => activePanel && splitTab(activePanel.id, "right"),
        !canSplit
      )}
      {action(
        t("workspace.splitDown", { defaultValue: "Split Down" }),
        <Rows2 size={14} aria-hidden="true" />,
        () => activePanel && splitTab(activePanel.id, "below"),
        !canSplit
      )}
      {activePanel &&
        action(
          isMaximized
            ? t("workspace.restoreGroup", { defaultValue: "Restore Group" })
            : t("workspace.maximizeGroup", { defaultValue: "Maximize Group" }),
          isMaximized ? (
            <Minimize2 size={14} aria-hidden="true" />
          ) : (
            <Maximize2 size={14} aria-hidden="true" />
          ),
          () => (isMaximized ? activePanel.api.exitMaximized() : activePanel.api.maximize())
        )}
      {activePanel &&
        groupCount > 1 &&
        action(
          t("workspace.restoreSingleGroup", { defaultValue: "Restore Single Group" }),
          <PanelsTopLeft size={14} aria-hidden="true" />,
          () => collapseToSingleGroup(activePanel.id)
        )}
    </div>
  )
}

const WorkspaceWatermark: React.FC = () => {
  const { t } = useTranslation()
  return (
    <div className="workspace-watermark">
      <Columns2 size={24} aria-hidden="true" />
      <span>{t("workspace.emptyGroup", { defaultValue: "Drag a tab here" })}</span>
    </div>
  )
}

const dockComponents = { workspacePanel: WorkspacePanel }
const dockTabComponents = { workspaceTab: WorkspaceTab }

export const TabPanels = forwardRef<TabPanelsHandle, TabPanelsProps>(function TabPanels(
  {
    activeTabId,
    duplicateTab,
    initialLayout,
    onActiveTabChange,
    onLayoutChange,
    tabs,
    ...workspaceProps
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

  useEffect(() => {
    const root = dockRootRef.current
    if (!root) {
      return
    }

    const stopResizing = () => {
      document.body.classList.remove(
        "workspace-sash-resizing",
        "workspace-sash-resizing-horizontal",
        "workspace-sash-resizing-vertical"
      )
    }

    const stopGroupDrag = () => {
      document.body.classList.remove("workspace-group-dragging")
    }

    const startResizing = (event: PointerEvent) => {
      if (event.button !== 0 || !(event.target instanceof Element)) {
        return
      }

      const sash = event.target.closest(".dv-sash")
      if (!sash || !root.contains(sash)) {
        return
      }

      const splitView = sash.closest(".dv-split-view-container")
      document.body.classList.add("workspace-sash-resizing")
      document.body.classList.toggle(
        "workspace-sash-resizing-horizontal",
        splitView?.classList.contains("dv-horizontal") ?? false
      )
      document.body.classList.toggle(
        "workspace-sash-resizing-vertical",
        splitView?.classList.contains("dv-vertical") ?? false
      )
      window.getSelection()?.removeAllRanges()
      event.preventDefault()
    }

    const startGroupDrag = (target: EventTarget | null) => {
      if (!(target instanceof Element)) {
        return
      }

      const dragHandle = target.closest(".dv-tab, .dv-draggable")
      if (!dragHandle || !root.contains(dragHandle)) {
        return
      }

      document.body.classList.add("workspace-group-dragging")
      window.getSelection()?.removeAllRanges()
    }

    const handleGroupPointerDown = (event: PointerEvent) => {
      if (event.button === 0) {
        startGroupDrag(event.target)
      }
    }

    const handleNativeDragStart = (event: DragEvent) => {
      startGroupDrag(event.target)
    }

    root.addEventListener("pointerdown", startResizing, true)
    root.addEventListener("pointerdown", handleGroupPointerDown, true)
    root.addEventListener("dragstart", handleNativeDragStart, true)
    document.addEventListener("pointerup", stopResizing)
    document.addEventListener("pointercancel", stopResizing)
    document.addEventListener("pointerup", stopGroupDrag)
    document.addEventListener("pointercancel", stopGroupDrag)
    document.addEventListener("dragend", stopGroupDrag)
    document.addEventListener("drop", stopGroupDrag)
    window.addEventListener("blur", stopResizing)
    window.addEventListener("blur", stopGroupDrag)

    return () => {
      root.removeEventListener("pointerdown", startResizing, true)
      root.removeEventListener("pointerdown", handleGroupPointerDown, true)
      root.removeEventListener("dragstart", handleNativeDragStart, true)
      document.removeEventListener("pointerup", stopResizing)
      document.removeEventListener("pointercancel", stopResizing)
      document.removeEventListener("pointerup", stopGroupDrag)
      document.removeEventListener("pointercancel", stopGroupDrag)
      document.removeEventListener("dragend", stopGroupDrag)
      document.removeEventListener("drop", stopGroupDrag)
      window.removeEventListener("blur", stopResizing)
      window.removeEventListener("blur", stopGroupDrag)
      stopResizing()
      stopGroupDrag()
    }
  }, [])

  const clearTabDrop = useCallback(() => {
    dropPreviewRef.current = null
    setDropPreview(null)
  }, [])

  const previewTabDrop = useCallback(
    (tabId: string, clientX: number, clientY: number) => {
      const root = dockRootRef.current
      const api = dockApi
      const sourcePanel = api?.getPanel(tabId)
      if (!root || !api || !sourcePanel) {
        clearTabDrop()
        return false
      }

      const targetGroupElement = Array.from(
        root.querySelectorAll<HTMLElement>(".dv-groupview")
      ).find((element) => {
        const rect = element.getBoundingClientRect()
        return (
          rect.width > 0 &&
          rect.height > 0 &&
          clientX >= rect.left &&
          clientX <= rect.right &&
          clientY >= rect.top &&
          clientY <= rect.bottom
        )
      })
      const targetElement = targetGroupElement
        ? Array.from(
            targetGroupElement.querySelectorAll<HTMLElement>("[data-workspace-panel-id]")
          ).find((element) => {
            const rect = element.getBoundingClientRect()
            return rect.width > 0 && rect.height > 0
          })
        : undefined
      const targetTabId = targetElement?.dataset.workspacePanelId
      const targetPanel = targetTabId ? api.getPanel(targetTabId) : undefined
      if (!targetGroupElement || !targetElement || !targetTabId || !targetPanel) {
        clearTabDrop()
        return false
      }

      const rootRect = root.getBoundingClientRect()
      const targetRect = targetGroupElement.getBoundingClientRect()
      const normalizedX = (clientX - targetRect.left) / targetRect.width - 0.5
      const normalizedY = (clientY - targetRect.top) / targetRect.height - 0.5
      const direction: WorkspaceDropDirection =
        Math.abs(normalizedX) <= 0.25 && Math.abs(normalizedY) <= 0.25
          ? "center"
          : Math.abs(normalizedX) > Math.abs(normalizedY)
            ? normalizedX < 0
              ? "left"
              : "right"
            : normalizedY < 0
              ? "above"
              : "below"

      if (
        sourcePanel.group === targetPanel.group &&
        (direction === "center" || sourcePanel.group.panels.length <= 1)
      ) {
        clearTabDrop()
        return false
      }
      const nextPreview: WorkspaceDropPreview = {
        direction,
        sourceTabId: tabId,
        targetTabId,
        left: targetRect.left - rootRect.left,
        top: targetRect.top - rootRect.top,
        width: targetRect.width,
        height: targetRect.height,
      }

      dropPreviewRef.current = nextPreview
      setDropPreview((current) =>
        current?.direction === nextPreview.direction &&
        current.targetTabId === nextPreview.targetTabId &&
        current.sourceTabId === nextPreview.sourceTabId &&
        current.left === nextPreview.left &&
        current.top === nextPreview.top &&
        current.width === nextPreview.width &&
        current.height === nextPreview.height
          ? current
          : nextPreview
      )
      return true
    },
    [clearTabDrop, dockApi]
  )

  const commitTabDrop = useCallback(
    (tabId: string, clientX: number, clientY: number) => {
      if (!previewTabDrop(tabId, clientX, clientY)) {
        return false
      }

      const preview = dropPreviewRef.current
      const sourcePanel = dockApi?.getPanel(tabId)
      const targetPanel = preview ? dockApi?.getPanel(preview.targetTabId) : undefined
      if (!preview || !sourcePanel || !targetPanel) {
        clearTabDrop()
        return false
      }

      sourcePanel.api.moveTo({
        group: targetPanel.group,
        position:
          preview.direction === "above"
            ? "top"
            : preview.direction === "below"
              ? "bottom"
              : preview.direction,
      })
      sourcePanel.api.setActive()
      clearTabDrop()
      return true
    },
    [clearTabDrop, dockApi, previewTabDrop]
  )

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
        renderer: "always",
        minimumWidth: 220,
        minimumHeight: 140,
      })
    },
    []
  )

  const splitTab = useCallback(
    (tabId: string, direction: WorkspaceSplitDirection) => {
      const sourcePanel = dockApi?.getPanel(tabId)
      if (!dockApi || !sourcePanel) {
        return
      }
      const sourceTab = tabsRef.current.find((tab) => tab.id === tabId)
      if (!sourceTab || (sourceTab.type !== "terminal" && sourceTab.type !== "ssh")) {
        return
      }

      if (sourcePanel.group.panels.length === 1) {
        const duplicatedTabId = duplicateTab(tabId)
        if (duplicatedTabId) {
          pendingSplitRef.current = {
            direction,
            sourceTabId: tabId,
            tabId: duplicatedTabId,
          }
        }
        return
      }

      const targetGroup = dockApi.addGroup({ referencePanel: sourcePanel, direction })
      sourcePanel.api.moveTo({ group: targetGroup, position: "center" })
      sourcePanel.api.setActive()
    },
    [dockApi, duplicateTab]
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

  useImperativeHandle(
    ref,
    () => ({
      activateTab(tabId) {
        dockApi?.getPanel(tabId)?.api.setActive()
      },
      clearTabDrop,
      commitTabDrop,
      getGroupTabIds(tabId) {
        const panel = dockApi?.getPanel(tabId)
        return panel ? panel.group.panels.map((groupPanel) => groupPanel.id) : null
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
      ...workspaceProps,
      collapseToSingleGroup,
      duplicateTab,
      groupCount,
      maximizedGroupId,
      splitTab,
      tabs,
    }),
    [
      collapseToSingleGroup,
      duplicateTab,
      groupCount,
      maximizedGroupId,
      splitTab,
      tabs,
      workspaceProps,
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
          rightHeaderActionsComponent={WorkspaceHeaderActions}
          watermarkComponent={WorkspaceWatermark}
          defaultRenderer="always"
          dndStrategy="pointer"
          keyboardNavigation
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
