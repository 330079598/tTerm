import React, { useCallback } from "react"
import { useTranslation } from "react-i18next"
import { Maximize2, X } from "lucide-react"

import { TabAttentionDot } from "@/components/TabAttentionDot"
import { TabIcon } from "@/components/TabIcon"
import { TabLogIndicator } from "@/components/TabLogIndicator"
import { TabTitle } from "@/components/TabTitle"
import { TabTooltip } from "@/components/TabTooltip"
import { useTabPasswordPrompt } from "@/lib/tabAttention"
import { getTabCloseMenuActions } from "@/lib/tabClosing"
import {
  getTabPinMenuAction,
  getTabRenameMenuAction,
  getTabSplitMenuActions,
  getLayoutTemplatesMenuAction,
  getUngroupMenuAction,
} from "@/lib/tabMenuActions"
import { keepFocusOnMouseDown } from "@/lib/terminalFocus"
import { getTabLogMenuActions } from "@/lib/terminalLogRecording"
import { isPageTab, type Tab, type TabContextMenuAction } from "@/types/tab"

/** The panes of a tab split into several. */
export interface TabPanes {
  /** Every pane's tab, in their order. */
  tabs: Tab[]
  /** One of the panes is zoomed to fill the tab. */
  isZoomed: boolean
}

interface TabItemProps {
  /** The tab, or for a split one, the pane it had the focus in. */
  tab: Tab
  panes?: TabPanes
  index: number
  getTabContextIds: (tabId: string) => string[]
  isActive: boolean
  isDragging: boolean
  setActiveNode?: (node: HTMLDivElement | null) => void
  onTabClick: (id: string) => void
  onTabClose: (id: string) => void
  onContextMenu: (event: React.MouseEvent, tab: Tab, actions: TabContextMenuAction[]) => void
  onPointerDown: (event: React.PointerEvent<HTMLDivElement>, tabId: string) => void
}

/** One tab in the tab bar, with its context menu. */
export const TabItem = React.memo(function TabItem({
  tab,
  panes,
  index,
  getTabContextIds,
  isActive,
  isDragging,
  setActiveNode,
  onTabClick,
  onTabClose,
  onContextMenu,
  onPointerDown,
}: TabItemProps) {
  const { t } = useTranslation()
  const paneCount = panes?.tabs.length ?? 1
  const isSplit = paneCount > 1
  // The active pane's marks come first, then the others'.
  const paneIds = isSplit
    ? [tab.id, ...(panes?.tabs.filter((pane) => pane.id !== tab.id).map((pane) => pane.id) ?? [])]
    : undefined
  const waitingForPassword = useTabPasswordPrompt(paneIds ?? [tab.id])

  const setNodeRef = (node: HTMLDivElement | null) => {
    if (isActive) setActiveNode?.(node)
  }

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault()
      const closeActions = getTabCloseMenuActions(
        getTabContextIds(tab.id),
        tab.id,
        {
          closeTab: t("contextMenu.closeTab"),
          closeOtherTabs: t("contextMenu.closeOtherTabs"),
          closeTabsToLeft: t("contextMenu.closeTabsToLeft"),
          closeTabsToRight: t("contextMenu.closeTabsToRight"),
        },
        isSplit
      )
      const splitActions = isSplit ? [getUngroupMenuAction(t)] : []

      if (isPageTab(tab)) {
        onContextMenu(e, tab, [
          {
            label: t("contextMenu.newTab"),
            action: "new",
            icon: "plus",
            shortcut: "workspace.newTab",
          },
          ...splitActions,
          ...(isSplit ? [getLayoutTemplatesMenuAction(t)] : []),
          { separator: true, label: "", action: "" },
          ...closeActions,
        ])
        return
      }

      // An SSH tab without a profile (quick connect, or connected without saving) can be saved as
      // one; a serial tab can only edit the profile it came from.
      const editConnectionAction: TabContextMenuAction | null = tab.connection?.profileId
        ? tab.type === "ssh" || tab.type === "serial"
          ? { label: t("contextMenu.editConnection"), action: "edit-connection", icon: "edit" }
          : null
        : tab.type === "ssh"
          ? { label: t("contextMenu.saveConnection"), action: "edit-connection", icon: "edit" }
          : null

      const logActions = getTabLogMenuActions(tab, t)

      const actions: TabContextMenuAction[] = [
        {
          label: t("contextMenu.newTab"),
          action: "new",
          icon: "plus",
          shortcut: "workspace.newTab",
        },
        {
          label: t("contextMenu.duplicateTab"),
          action: "duplicate",
          icon: "copy",
          shortcut: "tabs.duplicateActive",
        },
        ...getTabSplitMenuActions(tab, t),
        ...splitActions,
        getLayoutTemplatesMenuAction(t),
        getTabRenameMenuAction(tab, t),
        ...(editConnectionAction ? [editConnectionAction] : []),
        ...getTabPinMenuAction(tab, t),
        ...logActions,
        { separator: true, label: "", action: "" },
        ...closeActions,
      ]
      onContextMenu(e, tab, actions)
    },
    [getTabContextIds, isSplit, tab, onContextMenu, t]
  )

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (event.key !== "Enter" && event.key !== " ") {
        return
      }

      event.preventDefault()
      onTabClick(tab.id)
    },
    [onTabClick, tab.id]
  )

  return (
    <TabTooltip tab={tab} panes={isSplit ? panes?.tabs : undefined}>
      <div
        ref={setNodeRef}
        className={`tab-item ${isActive ? "active" : ""} ${tab.isModified ? "modified" : ""} ${isDragging ? "dragging" : ""}`}
        role="tab"
        tabIndex={0}
        aria-selected={isActive}
        data-tab-id={tab.id}
        data-password-prompt={waitingForPassword || undefined}
        data-allow-context-menu
        onClick={() => onTabClick(tab.id)}
        onContextMenu={handleContextMenu}
        onKeyDown={handleKeyDown}
        onMouseDown={keepFocusOnMouseDown}
        onPointerDown={(event) => onPointerDown(event, tab.id)}
      >
        <span className="tab-number">{index + 1}</span>
        <TabIcon tab={tab} paneIds={paneIds} className="tab-icon" />
        <TabAttentionDot tabId={tab.id} paneIds={paneIds} />
        <TabLogIndicator tabId={tab.id} paneIds={paneIds} />
        <TabTitle tab={tab} className="tab-title" />
        {isSplit && (
          <span
            className="tab-pane-count"
            role="img"
            aria-label={
              panes?.isZoomed
                ? `${t("workspace.paneCount", { count: paneCount })}, ${t("workspace.paneZoomed")}`
                : t("workspace.paneCount", { count: paneCount })
            }
            title={t("workspace.paneCount", { count: paneCount })}
          >
            {panes?.isZoomed ? <Maximize2 size={9} aria-hidden="true" /> : null}
            {paneCount}
          </span>
        )}
        <button
          className="tab-close"
          aria-label={t("contextMenu.closeTab", { defaultValue: "Close tab" })}
          onClick={(e) => {
            e.stopPropagation()
            onTabClose(tab.id)
          }}
        >
          <X size={12} />
        </button>
      </div>
    </TabTooltip>
  )
})
