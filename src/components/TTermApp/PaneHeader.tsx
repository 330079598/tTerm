import React from "react"
import { Maximize2, Minimize2, PanelBottom, PanelRight, PanelsTopLeft, X } from "lucide-react"
import { useTranslation } from "react-i18next"

import { TabAttentionDot } from "@/components/TabAttentionDot"
import { TabIcon } from "@/components/TabIcon"
import { TabLogIndicator } from "@/components/TabLogIndicator"
import { TabTitle } from "@/components/TabTitle"
import { TabTooltip } from "@/components/TabTooltip"
import { useWorkspace } from "@/components/TTermApp/workspaceContext"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useShortcutLabel } from "@/contexts/KeymapContext"
import type { KeymapActionId } from "@/lib/keymap/actions"
import { withShortcut } from "@/lib/keymap/keymap"
import { getPaneMenuActions } from "@/lib/tabMenuActions"
import { isSessionTab, type Tab } from "@/types/tab"

interface PaneHeaderProps {
  tab: Tab
  isActive: boolean
  isZoomed: boolean
  /** The pane's place among its tab's panes, from 1, shown while it is zoomed. */
  paneNumber: number
  paneCount: number
}

/**
 * The slim bar over a pane of a split tab: what runs in it, and buttons to
 * split, zoom and close it, and to undo the tab's split. Dragging it moves
 * the pane; double-clicking it zooms the pane.
 */
export const PaneHeader = React.memo(function PaneHeader({
  tab,
  isActive,
  isZoomed,
  paneNumber,
  paneCount,
}: PaneHeaderProps) {
  const { t } = useTranslation()
  const shortcutLabel = useShortcutLabel()
  const {
    duplicateTab,
    onTabClose,
    onTabContextMenu,
    startPaneDrag,
    togglePaneZoom,
    ungroupPanes,
  } = useWorkspace()
  const canSplit = isSessionTab(tab)

  const action = (
    label: string,
    shortcut: KeymapActionId | undefined,
    icon: React.ReactNode,
    onClick: () => void
  ) => {
    const text = shortcut ? withShortcut(label, shortcutLabel(shortcut)) : label
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            className="pane-header-action"
            aria-label={label}
            onClick={(event) => {
              event.stopPropagation()
              onClick()
            }}
          >
            {icon}
          </button>
        </TooltipTrigger>
        <TooltipContent>{text}</TooltipContent>
      </Tooltip>
    )
  }

  return (
    <TabTooltip tab={tab}>
      <div
        className={`pane-header ${isActive ? "active" : ""} ${tab.isModified ? "modified" : ""}`}
        data-allow-context-menu
        data-pane-header={tab.id}
        onPointerDown={(event) => startPaneDrag(event, tab.id)}
        onDoubleClick={(event) => {
          if (event.target instanceof Element && event.target.closest("button")) return
          togglePaneZoom(tab.id)
        }}
        onContextMenu={(event) =>
          onTabContextMenu(event, tab, getPaneMenuActions(tab, { isZoomed }, t))
        }
      >
        <div className="pane-header-tab">
          <TabIcon tab={tab} />
          <TabAttentionDot tabId={tab.id} />
          <TabLogIndicator tabId={tab.id} />
          <TabTitle tab={tab} className="pane-header-title" />
          {isZoomed && (
            <span className="pane-header-zoom">
              {paneNumber}/{paneCount}
            </span>
          )}
        </div>
        <div className="pane-header-actions">
          {canSplit &&
            action(
              t("workspace.splitRight"),
              "workspace.splitRight",
              <PanelRight size={13} aria-hidden="true" />,
              () => duplicateTab(tab.id, "right")
            )}
          {canSplit &&
            action(
              t("workspace.splitDown"),
              "workspace.splitBelow",
              <PanelBottom size={13} aria-hidden="true" />,
              () => duplicateTab(tab.id, "below")
            )}
          {action(
            isZoomed ? t("workspace.unzoomPane") : t("workspace.zoomPane"),
            "workspace.togglePaneZoom",
            isZoomed ? (
              <Minimize2 size={13} aria-hidden="true" />
            ) : (
              <Maximize2 size={13} aria-hidden="true" />
            ),
            () => togglePaneZoom(tab.id)
          )}
          {action(
            t("workspace.unsplit"),
            "workspace.restoreSingleGroup",
            <PanelsTopLeft size={13} aria-hidden="true" />,
            () => ungroupPanes(tab.id)
          )}
          {action(
            t("workspace.closePane"),
            "tabs.closeActive",
            <X size={13} aria-hidden="true" />,
            () => onTabClose(tab.id)
          )}
        </div>
      </div>
    </TabTooltip>
  )
})
