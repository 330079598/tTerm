import React, { useState } from "react"

import { PaneHeader } from "@/components/TTermApp/PaneHeader"
import { WorkspacePanel } from "@/components/TTermApp/WorkspacePanel"
import { useWorkspace } from "@/components/TTermApp/workspaceContext"
import type { PaneBox } from "@/lib/paneLayout"
import type { Tab } from "@/types/tab"

interface WorkspacePaneProps {
  tab: Tab
  /** Where the pane sits in its tab's layout, kept while the tab is out of sight. */
  box: PaneBox | undefined
  isVisible: boolean
  isActive: boolean
  /** The pane's tab is split, so the pane shows a header of its own. */
  showHeader: boolean
  isZoomed: boolean
  paneNumber: number
  paneCount: number
}

/**
 * One pane in the workspace. Every pane of every tab stays mounted, laid out
 * where its tab puts it; panes out of sight are hidden but keep their size,
 * so switching tabs neither restarts nor refits their terminals.
 */
export const WorkspacePane = React.memo(function WorkspacePane({
  tab,
  box,
  isVisible,
  isActive,
  showHeader,
  isZoomed,
  paneNumber,
  paneCount,
}: WorkspacePaneProps) {
  const { onActiveTabChange } = useWorkspace()
  const [hasBeenVisible, setHasBeenVisible] = useState(isVisible)
  if (isVisible && !hasBeenVisible) {
    setHasBeenVisible(true)
  }

  return (
    <div
      className={`workspace-pane ${isActive ? "active" : ""} ${showHeader ? "has-header" : ""}`}
      data-hidden={!isVisible || undefined}
      data-pane-id={tab.id}
      style={box && { left: box.left, top: box.top, width: box.width, height: box.height }}
      onPointerDownCapture={() => {
        if (isVisible && !isActive) onActiveTabChange(tab.id)
      }}
    >
      {showHeader && (
        <PaneHeader
          tab={tab}
          isActive={isActive}
          isZoomed={isZoomed}
          paneNumber={paneNumber}
          paneCount={paneCount}
        />
      )}
      <div className="workspace-pane-body">
        <WorkspacePanel
          tab={tab}
          isVisible={isVisible}
          hasBeenVisible={hasBeenVisible}
          isActive={isActive}
        />
      </div>
    </div>
  )
})
