import "@/components/TTermApp/Workspace.css"
import { forwardRef, useImperativeHandle, useMemo, useRef, type CSSProperties } from "react"
import { createPortal } from "react-dom"
import { Columns2, Rows2 } from "lucide-react"

import { TabIcon } from "@/components/TabIcon"
import { WorkspacePane } from "@/components/TTermApp/WorkspacePane"
import { WorkspaceSashes } from "@/components/TTermApp/WorkspaceSashes"
import {
  WorkspaceContext,
  type TabPanelsProps,
  type WorkspaceContextValue,
} from "@/components/TTermApp/workspaceContext"
import { usePaneDrag } from "@/components/TTermApp/usePaneDrag"
import { usePaneKeymap } from "@/components/TTermApp/usePaneKeymap"
import { usePaneSlots } from "@/components/TTermApp/usePaneSlots"
import { useWorkspaceDrop } from "@/components/TTermApp/useWorkspaceDrop"
import { compareTabIds } from "@/lib/tabNavigation"

/** The gap between panes, which their dividers sit in. */
const PANE_GAP = 5
const PANE_MINIMUM = { width: 220, height: 140 }

export interface TabPanelsHandle {
  clearTabDrop: () => void
  commitTabDrop: (tabId: string, clientX: number, clientY: number) => boolean
  previewTabDrop: (tabId: string, clientX: number, clientY: number) => boolean
}

/**
 * The workspace: the panes of the tab in sight, laid out by its split, with
 * the dividers between them. The panes of the other tabs stay mounted out of
 * sight, so neither switching tabs nor moving a pane to another tab remounts
 * its terminal.
 */
export const TabPanels = forwardRef<TabPanelsHandle, TabPanelsProps>(function TabPanels(
  {
    activeTabId,
    groups,
    paneLook,
    mergeGroup,
    placePane,
    resizeSplit,
    breakOutPane,
    broadcastSourceTabId,
    duplicateTab,
    handlePinConnectionHeader,
    handleReconnectTab,
    handleServerMonitorVisibilityChange,
    handleUnpinConnectionHeader,
    liveBroadcastState,
    onActiveTabChange,
    onConnectProfile,
    onEditProfile,
    onOpenCommandLibrary,
    onOpenRemoteFile,
    onPauseBroadcast,
    onResumeBroadcast,
    onSaveCommand,
    onStopBroadcast,
    onTabClose,
    onTabContextMenu,
    onTerminalAttention,
    onTerminalCommandExecuted,
    onTerminalConnectionProgress,
    onTerminalConnectionStateChange,
    onTerminalInput,
    onTerminalSavedPasswordPromptChange,
    onTerminalSensitivePrompt,
    onTerminalSessionUnavailable,
    profilesRefreshKey,
    startupConnectionsReady,
    startupSessionRestoreMode,
    tabs,
    togglePaneZoom,
    ungroupPanes,
    updateTab,
  },
  ref
) {
  const rootRef = useRef<HTMLDivElement | null>(null)
  const { show_headers: showPaneHeaders, dim_inactive_percent: dimPercent } = paneLook
  const { activeGroup, slots, visibleBoxes, sashes } = usePaneSlots(
    rootRef,
    groups,
    activeTabId,
    PANE_GAP
  )

  const { dropPreview, clearDrop, previewTabDrop, commitTabDrop, previewPaneDrop, commitPaneDrop } =
    useWorkspaceDrop({
      rootRef,
      groups,
      activeGroup,
      visibleBoxes,
      tabs,
      duplicateTab,
      mergeGroup,
      placePane,
    })

  const { startPaneDrag, ghostTab, ghostRef } = usePaneDrag({
    tabs,
    previewPaneDrop,
    commitPaneDrop,
    clearDrop,
    breakOutPane,
  })

  usePaneKeymap({
    activeTabId,
    activeGroup,
    visibleBoxes,
    setActiveTab: onActiveTabChange,
    ungroupPanes,
    togglePaneZoom,
    breakOutPane,
  })

  useImperativeHandle(ref, () => ({ clearTabDrop: clearDrop, commitTabDrop, previewTabDrop }), [
    clearDrop,
    commitTabDrop,
    previewTabDrop,
  ])

  const contextValue = useMemo<WorkspaceContextValue>(
    () => ({
      breakOutPane,
      broadcastSourceTabId,
      duplicateTab,
      handlePinConnectionHeader,
      handleReconnectTab,
      handleServerMonitorVisibilityChange,
      handleUnpinConnectionHeader,
      liveBroadcastState,
      onActiveTabChange,
      onConnectProfile,
      onEditProfile,
      onOpenCommandLibrary,
      onOpenRemoteFile,
      onPauseBroadcast,
      onResumeBroadcast,
      onSaveCommand,
      onStopBroadcast,
      onTabClose,
      onTabContextMenu,
      onTerminalAttention,
      onTerminalCommandExecuted,
      onTerminalConnectionProgress,
      onTerminalConnectionStateChange,
      onTerminalInput,
      onTerminalSavedPasswordPromptChange,
      onTerminalSensitivePrompt,
      onTerminalSessionUnavailable,
      profilesRefreshKey,
      startPaneDrag,
      startupConnectionsReady,
      startupSessionRestoreMode,
      tabs,
      togglePaneZoom,
      ungroupPanes,
      updateTab,
    }),
    [
      breakOutPane,
      broadcastSourceTabId,
      duplicateTab,
      handlePinConnectionHeader,
      handleReconnectTab,
      handleServerMonitorVisibilityChange,
      handleUnpinConnectionHeader,
      liveBroadcastState,
      onActiveTabChange,
      onConnectProfile,
      onEditProfile,
      onOpenCommandLibrary,
      onOpenRemoteFile,
      onPauseBroadcast,
      onResumeBroadcast,
      onSaveCommand,
      onStopBroadcast,
      onTabClose,
      onTabContextMenu,
      onTerminalAttention,
      onTerminalCommandExecuted,
      onTerminalConnectionProgress,
      onTerminalConnectionStateChange,
      onTerminalInput,
      onTerminalSavedPasswordPromptChange,
      onTerminalSensitivePrompt,
      onTerminalSessionUnavailable,
      profilesRefreshKey,
      startPaneDrag,
      startupConnectionsReady,
      startupSessionRestoreMode,
      tabs,
      togglePaneZoom,
      ungroupPanes,
      updateTab,
    ]
  )

  // Panes are listed in the order their tabs opened, which never changes, so
  // React never moves a terminal's element, which would reset its scroll.
  const mountOrder = useMemo(() => [...tabs].sort((a, b) => compareTabIds(a.id, b.id)), [tabs])
  const isSplit = activeGroup !== undefined && activeGroup.root.type === "split"
  // With more than one pane in sight, the ones without the keyboard may fade,
  // and without headers a frame marks the one with it.
  const showsSplit = isSplit && !activeGroup.zoomedTabId
  const workspaceClass = [
    "workspace",
    isSplit ? "is-split" : "",
    isSplit && showPaneHeaders ? "has-pane-headers" : "",
    showsSplit && !showPaneHeaders ? "frames-active-pane" : "",
    showsSplit && dimPercent > 0 ? "dims-inactive-panes" : "",
  ].join(" ")

  return (
    <WorkspaceContext.Provider value={contextValue}>
      <div
        ref={rootRef}
        className={workspaceClass}
        style={{ "--inactive-pane-dim": dimPercent / 100 } as CSSProperties}
      >
        {mountOrder.map((tab) => {
          const slot = slots.get(tab.id)
          if (!slot) return null
          return (
            <WorkspacePane
              key={tab.id}
              tab={tab}
              box={slot.box}
              isVisible={slot.isVisible}
              isActive={slot.isVisible && tab.id === activeTabId}
              showHeader={showPaneHeaders && slot.paneCount > 1}
              isZoomed={slot.group.zoomedTabId === tab.id}
              paneNumber={slot.paneNumber}
              paneCount={slot.paneCount}
            />
          )
        })}
        {activeGroup && sashes.length > 0 && (
          <WorkspaceSashes
            group={activeGroup}
            sashes={sashes}
            gap={PANE_GAP}
            minimum={PANE_MINIMUM}
            resizeSplit={resizeSplit}
          />
        )}
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
              {dropPreview.direction === "left" || dropPreview.direction === "right" ? (
                <Columns2 size={24} />
              ) : (
                <Rows2 size={24} />
              )}
            </div>
          </div>
        )}
      </div>
      {ghostTab &&
        createPortal(
          <div ref={ghostRef} className="tab-drag-ghost" aria-hidden="true">
            <TabIcon tab={ghostTab} className="tab-icon" />
            <span className="tab-drag-ghost-title">{ghostTab.title}</span>
          </div>,
          document.body
        )}
    </WorkspaceContext.Provider>
  )
})
